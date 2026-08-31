#!/usr/bin/env bash
set -euo pipefail

APP_NAME="MD Previewer"
BUNDLE_ID="io.github.arnoldredman.mdpreviewer"
APP_DIR="target/${APP_NAME}.app"

if [ ! -d "$APP_DIR" ]; then
  echo "App bundle not found. Run ./bundle.sh first." >&2
  exit 1
fi

cp -R "$APP_DIR" /Applications/
/System/Library/Frameworks/CoreServices.framework/Versions/A/Frameworks/LaunchServices.framework/Versions/A/Support/lsregister \
  -f "/Applications/${APP_NAME}.app"

swift - "$BUNDLE_ID" <<'SWIFT'
import CoreServices
import Foundation

let bundleID = CommandLine.arguments[1] as NSString
let markdown = "net.daringfireball.markdown" as NSString
let result = LSSetDefaultRoleHandlerForContentType(markdown, .viewer, bundleID)
print(result == noErr ? "Default Markdown viewer set." : "Could not set default viewer (error \(result)).")
SWIFT

echo "Installed /Applications/${APP_NAME}.app"
