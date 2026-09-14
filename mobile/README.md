# MD Previewer Mobile

手机端 MVP 是只读、离线、快速打开 Markdown 文件：

- iOS：UIKit + `WKWebView`
- Android：Java + `WebView`
- 共享渲染层：`mobile/shared`

## Build

前置条件：

- JDK 17（AGP 8.13 要求，JDK 11 会在配置阶段直接报错）
- Android SDK：`platforms;android-36` + `build-tools;36.0.0` + `platform-tools`（compileSdk / targetSdk 都是 36）
- SDK 位置通过 `ANDROID_HOME` 或 `mobile/android/local.properties` 里的 `sdk.dir` 指定；`local.properties` 是本机文件，不入库
- Gradle 用仓库自带的 wrapper（固定 8.14.3）；AGP 8.13 与 Gradle 9.6+ 不兼容，不要用系统 gradle 直接构建
- 不想单独装整套 SDK 时，可以直接复用 Unity 自带的 Android SDK：`<UnityEditor>/Editor/Data/PlaybackEngines/AndroidPlayer/SDK` 里已有 `cmdline-tools`（含 `sdkmanager`）和 `platform-tools`，把它复制到标准位置或用 `ANDROID_HOME` 指过去，再补装缺的组件即可；Unity 自带的 OpenJDK 是 11，不能用来跑 AGP

Android：

```bash
cd mobile/android
./gradlew :app:assembleDebug        # 可安装的调试包
./gradlew :app:assembleRelease      # 未签名，需签名后才能安装或上架
```

iOS（需要 macOS + Xcode）：

```bash
cd mobile/ios
xcodegen generate
xcodebuild -project MDPreviewerMobile.xcodeproj -scheme MDPreviewerMobile -destination 'generic/platform=iOS' CODE_SIGNING_ALLOWED=NO build
```

## Release identity

- iOS bundle ID: `io.github.arnoldredman.mdpreviewer.mobile`
- Android application ID: `io.github.arnoldredman.mdpreviewer.mobile`
- iOS signing team is intentionally unset; configure your own team when archiving
- Android signing material belongs only in `.env.mobile-release` and `mobile/android/signing/`

Run `mobile/scripts/verify-release-readiness.sh` before a mobile release. Real-device checks remain in [RELEASE_CHECKLIST.md](RELEASE_CHECKLIST.md).
