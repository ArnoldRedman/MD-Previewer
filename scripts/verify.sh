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
    # src 已按职责拆成多个模块，身份标记只要落在 src 里就算数
    'src': ['MD Previewer', 'md-previewer', 'MD_PREVIEWER_CONFIG_DIR', 'io.github.arnoldredman.mdpreviewer'],
    'build.rs': ['ArnoldRedman', 'MD Previewer'],
    'bundle.sh': ['MD Previewer', 'io.github.arnoldredman.mdpreviewer', 'mdpreviewer'],
    'mobile/ios/project.yml': ['MDPreviewerMobile', 'io.github.arnoldredman.mdpreviewer.mobile'],
    'mobile/android/app/build.gradle': ['io.github.arnoldredman.mdpreviewer.mobile'],
}
for filename, markers in required.items():
    path = Path(filename)
    if path.is_dir():
        text = '\n'.join(p.read_text(encoding='utf-8') for p in sorted(path.rglob('*.rs')))
    else:
        text = path.read_text(encoding='utf-8')
    for marker in markers:
        if marker not in text:
            raise SystemExit(f'{filename} is missing identity marker: {marker}')

scan = [Path('src'), Path('frontend'), Path('macos'), Path('mobile'), Path('.github')]
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
bash -n bundle.sh install.sh scripts/build-linux.sh mobile/scripts/*.sh

echo "[verify] Rust format/check/test"
cargo fmt --check
cargo check
cargo test

if [ -x node_modules/.bin/eslint ]; then
  echo "[verify] frontend lint"
  node_modules/.bin/eslint .
else
  echo "[verify] frontend lint skipped: run npm ci first"
fi

if command -v node >/dev/null 2>&1 && node -e "import('playwright')" >/dev/null 2>&1; then
  echo "[verify] browser checks"
  node scripts/verify-anchor-navigation.mjs
  node scripts/verify-desktop-search.mjs
  node scripts/verify-desktop-reading-tools.mjs
  node scripts/verify-desktop-tables.mjs
  node scripts/verify-desktop-edit-topbar.mjs
  node scripts/verify-ux-improvements.mjs
  node scripts/verify-auto-update.mjs
  node scripts/verify-encoding-convert.mjs
  node scripts/verify-landing-page.mjs
  node scripts/verify-shortcuts-settings-logs.mjs
  node mobile/scripts/verify-mobile-renderer.mjs
else
  echo "[verify] browser checks skipped: Playwright unavailable"
fi

# Android 统一走仓库自带的 wrapper（固定 Gradle 8.14.3）：
# AGP 8.13 与 Gradle 9.6+ 不兼容，依赖系统 gradle 会随机器而变
if [ -x mobile/android/gradlew ]; then
  if [ -z "${ANDROID_HOME:-${ANDROID_SDK_ROOT:-}}" ] && [ ! -f mobile/android/local.properties ]; then
    echo "[verify] Android build skipped: Android SDK location is not configured"
  else
    echo "[verify] Android debug build"
    (cd mobile/android && ./gradlew --no-daemon :app:assembleDebug)
  fi
else
  echo "[verify] Android build skipped: Gradle wrapper missing"
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
