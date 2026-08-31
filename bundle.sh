#!/usr/bin/env bash
set -euo pipefail

APP_NAME="MD Previewer"
BUNDLE_ID="io.github.arnoldredman.mdpreviewer"
BIN="md-previewer"
APP_DIR="target/${APP_NAME}.app"
APP_VERSION="$(awk -F\" '/^version = / { print $2; exit }' Cargo.toml)"

echo "Building universal release..."
cargo build --release --target aarch64-apple-darwin
cargo build --release --target x86_64-apple-darwin
lipo -create \
  "target/aarch64-apple-darwin/release/$BIN" \
  "target/x86_64-apple-darwin/release/$BIN" \
  -output "target/release/$BIN-universal"

echo "Creating app bundle..."
rm -rf "$APP_DIR"
mkdir -p "$APP_DIR/Contents/MacOS" "$APP_DIR/Contents/Resources" "$APP_DIR/Contents/PlugIns"
cp "target/release/$BIN-universal" "$APP_DIR/Contents/MacOS/$BIN"
cp assets/icon.icns "$APP_DIR/Contents/Resources/AppIcon.icns"

echo "Building Finder Sync extension..."
FINDER_PROJECT="macos/finder-extension/MDPreviewerFinderExtension.xcodeproj"
FINDER_DERIVED="target/finder-extension-derived"
xcodebuild \
  -project "$FINDER_PROJECT" \
  -scheme MDPreviewerFinderExtension \
  -configuration Release \
  -derivedDataPath "$FINDER_DERIVED" \
  ARCHS="arm64 x86_64" \
  ONLY_ACTIVE_ARCH=NO \
  CODE_SIGNING_ALLOWED=NO \
  MARKETING_VERSION="$APP_VERSION" \
  CURRENT_PROJECT_VERSION="$APP_VERSION" \
  build >/dev/null
ditto \
  "$FINDER_DERIVED/Build/Products/Release/MDPreviewerFinderExtension.appex" \
  "$APP_DIR/Contents/PlugIns/MDPreviewerFinderExtension.appex"

cat > "$APP_DIR/Contents/Info.plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>CFBundleName</key><string>${APP_NAME}</string>
  <key>CFBundleDisplayName</key><string>${APP_NAME}</string>
  <key>CFBundleIdentifier</key><string>${BUNDLE_ID}</string>
  <key>CFBundleVersion</key><string>${APP_VERSION}</string>
  <key>CFBundleShortVersionString</key><string>${APP_VERSION}</string>
  <key>CFBundleExecutable</key><string>${BIN}</string>
  <key>CFBundleIconFile</key><string>AppIcon</string>
  <key>CFBundlePackageType</key><string>APPL</string>
  <key>NSHighResolutionCapable</key><true/>
  <key>LSMinimumSystemVersion</key><string>10.15</string>
  <key>CFBundleURLTypes</key>
  <array><dict>
    <key>CFBundleURLName</key><string>MD Previewer Finder Actions</string>
    <key>CFBundleURLSchemes</key><array><string>mdpreviewer</string></array>
  </dict></array>
  <key>CFBundleDocumentTypes</key>
  <array><dict>
    <key>CFBundleTypeName</key><string>Markdown Document</string>
    <key>CFBundleTypeRole</key><string>Viewer</string>
    <key>LSHandlerRank</key><string>Owner</string>
    <key>LSItemContentTypes</key><array><string>net.daringfireball.markdown</string></array>
    <key>CFBundleTypeExtensions</key>
    <array><string>md</string><string>markdown</string><string>mdown</string><string>mkd</string></array>
    <key>CFBundleTypeIconFile</key><string>AppIcon</string>
  </dict></array>
  <key>UTImportedTypeDeclarations</key>
  <array><dict>
    <key>UTTypeIdentifier</key><string>net.daringfireball.markdown</string>
    <key>UTTypeDescription</key><string>Markdown Document</string>
    <key>UTTypeConformsTo</key><array><string>public.plain-text</string></array>
    <key>UTTypeTagSpecification</key><dict>
      <key>public.filename-extension</key>
      <array><string>md</string><string>markdown</string><string>mdown</string><string>mkd</string></array>
    </dict>
  </dict></array>
</dict>
</plist>
PLIST

codesign --force --sign - \
  --entitlements macos/finder-extension/Extension.entitlements \
  "$APP_DIR/Contents/PlugIns/MDPreviewerFinderExtension.appex"
codesign --force --sign - "$APP_DIR"

echo "Done: $APP_DIR"
