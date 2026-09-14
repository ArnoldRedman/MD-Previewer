#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$ROOT"

if [ -f "$ROOT/.env.mobile-release" ]; then
  set -a
  # shellcheck disable=SC1091
  . "$ROOT/.env.mobile-release"
  set +a
fi

fail() {
  echo "[release-readiness] FAIL: $*" >&2
  exit 1
}

echo "[release-readiness] root: $ROOT"

# Windows 上只有 python，没有 python3（那个是应用执行别名占位符）
if python3 --version >/dev/null 2>&1; then
  PYTHON=python3
elif python --version >/dev/null 2>&1; then
  PYTHON=python
else
  fail "python missing"
fi

test -f mobile/ios/MDPreviewerMobile/PrivacyInfo.xcprivacy || fail "missing iOS privacy manifest"
# plutil / xcodegen / xcrun 只有 macOS 才有，其他平台上只校验文件本身
if command -v plutil >/dev/null 2>&1; then
  plutil -lint mobile/ios/MDPreviewerMobile/Info.plist mobile/ios/MDPreviewerMobile/PrivacyInfo.xcprivacy >/dev/null
else
  echo "[release-readiness] iOS plist lint skipped: plutil unavailable (not macOS)"
fi
"$PYTHON" -m json.tool mobile/ios/MDPreviewerMobile/Assets.xcassets/Contents.json >/dev/null
"$PYTHON" -m json.tool mobile/ios/MDPreviewerMobile/Assets.xcassets/AppIcon.appiconset/Contents.json >/dev/null

ANDROID_ACTIVITY="mobile/android/app/src/main/java/io/github/arnoldredman/mdpreviewer/MainActivity.java"
grep -F 'intent.setType("text/*")' "$ANDROID_ACTIVITY" >/dev/null || fail "Android Open File picker must request text MIME"
if grep -F 'intent.setType("*/*")' "$ANDROID_ACTIVITY" >/dev/null; then
  fail "Android Open File picker must not request */*"
fi
if sed -n '/private void openDocumentPicker()/,/startActivityForResult/p' "$ANDROID_ACTIVITY" | grep -F 'application/octet-stream' >/dev/null; then
  fail "Android Open File picker must not use application/octet-stream"
fi
grep -F 'isSupportedDocument(uri)' "$ANDROID_ACTIVITY" >/dev/null || fail "Android Open File must validate selected documents"

echo "[release-readiness] Android release build"
(
  cd mobile/android
  # 必须用仓库自带的 wrapper：AGP 8.13 与系统 Gradle 9.6+ 不兼容
  ./gradlew --no-daemon :app:assembleRelease :app:bundleRelease
)

APK="mobile/android/app/build/outputs/apk/release/app-release-unsigned.apk"
if [ -f mobile/android/app/build/outputs/apk/release/app-release.apk ]; then
  APK="mobile/android/app/build/outputs/apk/release/app-release.apk"
fi
test -f "$APK" || fail "missing Android release APK"
test -f mobile/android/app/build/outputs/bundle/release/app-release.aab || fail "missing Android release AAB"

# SDK 位置：环境变量优先，其次 local.properties，最后才猜 macOS 默认路径
SDK_DIR="${ANDROID_HOME:-${ANDROID_SDK_ROOT:-}}"
if [ -z "$SDK_DIR" ] && [ -f mobile/android/local.properties ]; then
  SDK_DIR="$(sed -n 's/^sdk.dir=//p' mobile/android/local.properties | tr -d '\r')"
fi
if [ -z "$SDK_DIR" ] && [ -d "$HOME/Library/Android/sdk" ]; then
  SDK_DIR="$HOME/Library/Android/sdk"
fi
find_tool() {
  [ -n "$SDK_DIR" ] || return 0
  # Windows 上是 aapt.exe / apksigner.bat，Linux/macOS 上是无后缀的可执行文件；
  # 不能直接用前缀匹配，否则 aapt 会选到 CLI 不同的 aapt2
  local base="$1" candidate
  for candidate in "$base" "$base.exe" "$base.bat"; do
    local found
    found="$(find "$SDK_DIR/build-tools" -name "$candidate" -type f 2>/dev/null | sort | tail -1)"
    if [ -n "$found" ]; then
      echo "$found"
      return 0
    fi
  done
}

AAPT="$(find_tool aapt || true)"
if [ -n "$AAPT" ]; then
  "$AAPT" dump permissions "$APK" | grep -q "INTERNET" && fail "Android release requests INTERNET permission"
  "$AAPT" dump xmltree "$APK" AndroidManifest.xml | grep -q "android.intent.action.VIEW" || fail "Android VIEW intent missing"
  "$AAPT" dump xmltree "$APK" AndroidManifest.xml | grep -q "text/markdown" || fail "Android markdown MIME missing"
else
  echo "[release-readiness] Android manifest checks skipped: aapt not found"
fi

APKSIGNER="$(find_tool apksigner || true)"
if [ -n "${MD_PREVIEWER_ANDROID_KEYSTORE:-}" ]; then
  if [ -n "$APKSIGNER" ]; then
    "$APKSIGNER" verify --verbose "$APK" >/dev/null || fail "Android release APK is not signed"
  fi
  jarsigner -verify mobile/android/app/build/outputs/bundle/release/app-release.aab >/dev/null 2>&1 || fail "Android release AAB is not signed"
else
  echo "[release-readiness] Android signing env not set; release artifacts are buildable but not store-uploadable"
fi

# iOS 只能在本机是 macOS 时校验；其他平台不阻断，但要说清楚跳过了什么
if [ "$(uname -s)" != "Darwin" ]; then
  echo "[release-readiness] iOS checks skipped: not running on macOS"
elif command -v xcodegen >/dev/null 2>&1; then
  echo "[release-readiness] iOS project generation"
  (cd mobile/ios && xcodegen generate)
  if command -v xcrun >/dev/null 2>&1; then
    echo "[release-readiness] iOS Swift parse"
    xcrun --sdk iphoneos swiftc -parse \
      mobile/ios/MDPreviewerMobile/AppDelegate.swift \
      mobile/ios/MDPreviewerMobile/PreviewViewController.swift
  fi
else
  fail "xcodegen missing"
fi

echo "[release-readiness] OK"
