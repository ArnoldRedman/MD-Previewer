# MD Previewer Mobile

手机端 MVP 是只读、离线、快速打开 Markdown 文件：

- iOS：UIKit + `WKWebView`
- Android：Java + `WebView`
- 共享渲染层：`mobile/shared`

## Build

```bash
cd mobile/ios
xcodegen generate
xcodebuild -project MDPreviewerMobile.xcodeproj -scheme MDPreviewerMobile -destination 'generic/platform=iOS' CODE_SIGNING_ALLOWED=NO build

cd ../android
gradle :app:assembleDebug
```

## Release identity

- iOS bundle ID: `io.github.arnoldredman.mdpreviewer.mobile`
- Android application ID: `io.github.arnoldredman.mdpreviewer.mobile`
- iOS signing team is intentionally unset; configure your own team when archiving
- Android signing material belongs only in `.env.mobile-release` and `mobile/android/signing/`

Run `mobile/scripts/verify-release-readiness.sh` before a mobile release. Real-device checks remain in [RELEASE_CHECKLIST.md](RELEASE_CHECKLIST.md).
