#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

echo "[verify] MD Previewer identity"
if python3 --version >/dev/null 2>&1; then
  PYTHON=python3
elif python --version >/dev/null 2>&1; then
  PYTHON=python
else
  echo "[verify] Python is required for identity checks" >&2
  exit 1
fi
"$PYTHON" - <<'PY'
from pathlib import Path
import re
import tomllib

root = Path('.')
package = tomllib.loads(Path('Cargo.toml').read_text(encoding='utf-8'))['package']
expected = {
    'name': 'md-previewer',
    'license': 'MIT',
    'repository': 'https://github.com/ArnoldRedman/md-preview',
}
for key, value in expected.items():
    if package.get(key) != value:
        raise SystemExit(f'Cargo.toml {key} must be {value!r}')

version = package['version']
version_markers = {
    'mobile/ios/project.yml': f'MARKETING_VERSION: "{version}"',
    'mobile/android/app/build.gradle': f'versionName = "{version}"',
    'macos/finder-extension/project.yml': f'MARKETING_VERSION: "{version}"',
}
for filename, marker in version_markers.items():
    if marker not in Path(filename).read_text(encoding='utf-8'):
        raise SystemExit(f'{filename} version must match Cargo.toml: {version}')

required = {
    'src/main.rs': ['MD Previewer', 'md-previewer', 'MD_PREVIEWER_CONFIG_DIR', 'io.github.arnoldredman.mdpreviewer'],
    'build.rs': ['ArnoldRedman', 'MD Previewer'],
    'bundle.sh': ['MD Previewer', 'io.github.arnoldredman.mdpreviewer', 'mdpreviewer'],
    'mobile/ios/project.yml': ['MDPreviewerMobile', 'io.github.arnoldredman.mdpreviewer.mobile'],
    'mobile/android/app/build.gradle': ['io.github.arnoldredman.mdpreviewer.mobile'],
}
for filename, markers in required.items():
    text = Path(filename).read_text(encoding='utf-8')
    for marker in markers:
        if marker not in text:
            raise SystemExit(f'{filename} is missing identity marker: {marker}')

scan = [Path('src'), Path('assets/enhance'), Path('macos'), Path('mobile'), Path('.github')]
files = [Path('build.rs'), Path('bundle.sh'), Path('install.sh')]
for base in scan:
    files.extend(path for path in base.rglob('*') if path.is_file())

patterns = {
    'upstream owner in product files': re.compile(r'vorojar', re.I),
    'upstream bundle id': re.compile(r'(?:com|app)\.mdpreview', re.I),
    'upstream signing team': re.compile(r'BUR55497B4'),
    'upstream App Store id': re.compile(r'6779451523'),
    'old display name': re.compile(r'MD Preview(?!er)'),
    'old native symbol': re.compile(r'MDPreview(?!er)|mdPreview(?!er)'),
    'automatic update implementation': re.compile(r'update-check|CheckUpdates|Sparkle|appcast|Get-FileHash', re.I),
}
for path in files:
    try:
        text = path.read_text(encoding='utf-8')
    except UnicodeDecodeError:
        continue
    for label, pattern in patterns.items():
        match = pattern.search(text)
        if match:
            raise SystemExit(f'{label}: {path}:{text.count(chr(10), 0, match.start()) + 1}')

removed = [
    '.github/workflows/release.yml',
    'assets/enhance/update-check.js',
    'release-sign.sh',
    'scripts/fetch-sparkle.sh',
    'scripts/generate-appcast.sh',
    'scripts/verify-sparkle-update.sh',
    'scripts/verify-windows-self-update.sh',
]
for filename in removed:
    if Path(filename).exists():
        raise SystemExit(f'upstream release/update file must stay removed: {filename}')
PY

echo "[verify] shell syntax"
bash -n bundle.sh install.sh mobile/scripts/*.sh

echo "[verify] Rust format/check/test"
cargo fmt --check
cargo check
cargo test

if command -v node >/dev/null 2>&1 && node -e "import('playwright')" >/dev/null 2>&1; then
  echo "[verify] browser checks"
  node scripts/verify-anchor-navigation.mjs
  node scripts/verify-desktop-search.mjs
  node scripts/verify-desktop-reading-tools.mjs
  node scripts/verify-desktop-edit-topbar.mjs
  node scripts/verify-ux-improvements.mjs
  node mobile/scripts/verify-mobile-renderer.mjs
else
  echo "[verify] browser checks skipped: Playwright unavailable"
fi

if command -v gradle >/dev/null 2>&1; then
  GRADLE_VERSION="$(gradle --version | awk '/^Gradle / { print $2; exit }')"
  if [ -z "${ANDROID_HOME:-${ANDROID_SDK_ROOT:-}}" ] && [ ! -f mobile/android/local.properties ]; then
    echo "[verify] Android build skipped: Android SDK location is not configured"
  elif [[ "$GRADLE_VERSION" =~ ^9\.([6-9]|[1-9][0-9]) ]] || [[ "$GRADLE_VERSION" =~ ^[1-9][0-9]\. ]]; then
    echo "[verify] Android build skipped: Gradle $GRADLE_VERSION is newer than Android Gradle Plugin 8.x supports"
  else
    echo "[verify] Android debug build"
    (cd mobile/android && gradle :app:assembleDebug)
  fi
else
  echo "[verify] Android build skipped: Gradle unavailable"
fi

if command -v xcodegen >/dev/null 2>&1 && command -v xcodebuild >/dev/null 2>&1; then
  echo "[verify] iOS project and unsigned build"
  (
    cd mobile/ios
    xcodegen generate
    xcodebuild -project MDPreviewerMobile.xcodeproj -scheme MDPreviewerMobile \
      -destination 'generic/platform=iOS' CODE_SIGNING_ALLOWED=NO build
  )
else
  echo "[verify] iOS build skipped: XcodeGen/Xcode unavailable"
fi

echo "[verify] OK"
