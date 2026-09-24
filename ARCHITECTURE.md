# 项目内情与维护笔记

这份文档写给**接手的人或 AI**：讲清这个项目为什么长成现在这样、哪些约束不能破、踩过哪些平台坑。
命令和目录约定见 `AGENTS.md`，面向用户的功能介绍见 `README.md`，版本历史见 `CHANGELOG.md`，这里不重复。

改动这个项目前建议按顺序读：`AGENTS.md` → 本文档 → 相关模块源码。

---

## 1. 三层结构

```
┌──────────────────────── 桌面端（Rust，src/ 共 25 个模块）────────────────────────┐
│  main.rs        启动接线：解析参数、建窗口/WebView、进事件循环（约 390 行）        │
│  app.rs         App 状态机：IPC 分发、标签/会话、文件监听、更新流程（约 990 行）  │
│  document.rs    编码探测与转换、保存、外部改动保护                               │
│  markdown.rs    Markdown → HTML、标题锚点、增强特性探测（KaTeX/Mermaid 是否需要）│
│  sanitize.rs    HTML 白名单消毒（安全边界）                                      │
│  escape.rs      HTML/JS 转义                                                     │
│  page.rs        把模板 + 文案 + 正文拼成一份完整 HTML                            │
│  assets.rs      include_str! 内嵌前端资源与模板                                  │
│  其余            paths / sidebar / ipc / window / theme / i18n / platform /      │
│                  finder / macos_menu / updater / watch / webview                 │
│  tests.rs       1286 行单元测试，93 个用例                                       │
└──────────────────────────────────┬──────────────────────────────────────────────┘
                                   │ include_str! + 渲染
┌──────────────────────────────────▼──────────────────────────────────────────────┐
│  frontend/   应用窗口里真正的界面（不再内嵌在 Rust 字符串里）                     │
│    page.html  骨架，含 {{占位符}}                                                │
│    page.css   样式（侧栏宽度等由 Rust 注入 CSS 变量）                            │
│    page.js    界面逻辑（标签、搜索、灯箱、更新弹窗），配置从 window.__mdPreviewerConfig 读 │
│    preview-enhance.js  KaTeX / Mermaid 离线渲染 + 标题锚点跟随                          │
└──────────────────────────────────┬──────────────────────────────────────────────┘
                                   │ 同一套渲染规则
┌──────────────────────────────────▼──────────────────────────────────────────────┐
│  mobile/     iOS（UIKit + WKWebView）、Android（WebView）、共享层 mobile/shared/  │
└─────────────────────────────────────────────────────────────────────────────────┘
```

**关键分工**：Rust 只负责"拿到什么内容、用什么文案、什么时候重载"；排版和交互全在 `frontend/`。
改界面样式/交互只动 `frontend/`，不用碰 Rust。改渲染规则（哪些 Markdown 语法支持什么）才动 `markdown.rs`/`sanitize.rs`。

**数据流（一次打开文档）**：Rust 读文件 → 探测编码 → `markdown.rs` 转 HTML → `sanitize.rs` 白名单过滤 →
`page.rs` 把 `{{占位符}}` 替换成真实值 → `webview.load_html()` → `page.js` 接管交互，
后续变更走 `window.ipc.postMessage()` 回到 `app.rs`。

---

## 2. 不能破坏的约束

每条都有"谁是守门人"——**改之前先看守门人的实现，别绕过它**。

| 约束 | 为什么 | 守门人 |
|---|---|---|
| 模板里每个 `{{name}}` 都必须被替换成真实值 | 漏替换会让页面上出现字面量 `{{...}}`，用户直接看到 | `tests::page_template_fills_every_placeholder` |
| `page.js` 里读的每个 `CFG.xxx`，Rust 注入的 JSON 必须有同名键 | 键名不一致前端只会静默拿到 `undefined`，功能静默失效 | `tests::page_config_covers_every_frontend_key`（会扫描 `page.js` 反查） |
| 原始 HTML 必须过白名单消毒 | `sanitize.rs` 是安全边界；文档可能来自任何地方 | `sanitize.rs` + `tests.rs` 里的 XSS 用例 |
| 表格必须自适应可视宽度，放不下才让表格自己横向滚 | 表格顶出可视范围会逼用户拖窗口，手机上还会把整页顶宽 | `scripts/verify-desktop-tables.mjs` + `mobile/scripts/verify-mobile-renderer.mjs` 的表格断言 || 打开文档不发任何网络请求；安卓包不声明网络权限 | 对外的隐私承诺，写在下载页和 README 里 | `mobile/scripts/verify-release-readiness.sh`（断言 APK 无 INTERNET 权限） |
| 外部改动不能覆盖未保存内容；自己的写入不能触发自我重载 | 会丢用户数据 / 无限重载循环 | `document.rs` 的 `SelfWriteRecord` + `self_write_still_matches_disk` + `should_protect_external_change` |
| 发布资产名不带版本号（`MD-Previewer-*.exe/.apk/.deb/.tar.gz`） | 下载页用 `releases/latest/download/<名>` 直链，带版本号就失效 | `scripts/verify-landing-page.mjs` 与各构建脚本交叉校验 |
| 应用是单实例；第二次打开同一文件要转发给已有窗口 | Windows 上双击文档的常见路径 | `single_instance.rs` + `scripts/verify-windows-single-instance.ps1` |
| 首屏 HTML 不带文档正文，正文一律等页面就绪后走 `__setContent` 推入 | WebView2 的 `NavigateToString` 上限 2MiB，超了连 webview 都建不出来，双击大文档直接闪退 | `tests::startup_page_stays_small_even_for_a_huge_document` + `scripts/verify-windows-large-document-startup.ps1` |

---

## 3. 验证体系

**唯一入口是 `./scripts/verify.sh`**，别自己在 CI 或文档里另写一套命令。它按顺序跑：

| 步骤 | 守什么 | 缺依赖时 |
|---|---|---|
| 身份标记 | 品牌隔离：名称/包名/配置目录标识齐全，且上游更新通道的残留文件不存在 | 必跑，缺了直接失败 |
| `bash -n` | shell 语法 | 必跑 |
| `cargo fmt/check/test` | Rust 格式、编译、98 个单测 | 必跑 |
| ESLint | `frontend/`、`mobile/shared/`、验证脚本 | 没有 `node_modules/.bin/eslint` 时跳过 |
| 9 个 Playwright 检查 | 前端行为：目录跳转、搜索、阅读工具、编辑顶栏、UX、自动更新、编码转换、落地页、手机渲染层 | Playwright 不可用时跳过 |
| Android 构建 | `:app:assembleDebug`（走 wrapper） | 没有 SDK 位置时跳过 |
| iOS 构建 | xcodegen + xcodebuild | 非 macOS 跳过 |

**加检查的规矩**：能用脚本断言的就别靠人自觉。前端行为加 `scripts/verify-*.mjs`（用 `scripts/desktop-page.mjs`
读资源和源码，别各自解析 `src/main.rs`），Rust 不变量加 `src/tests.rs` 用例，然后挂进 `verify.sh`。

**改渲染输出的安全做法**：改 `page.rs`/`frontend/` 前先存一份渲染结果当基准，改完逐字节对比，
确认差异只有你预期的那几处。这套做法在模板外置重构时用过，抓出过两个真问题。

---

## 4. 平台坑速查（都是实际踩过的）

| 症状 | 原因 | 做法 |
|---|---|---|
| PowerShell 脚本报 `unexpected token`、缺 catch 块之类莫名错误 | 仓库里 `.ps1` 都是**纯 ASCII 无 BOM**；加了中文注释，Windows PowerShell 5.1 按 ANSI 解码直接破坏语法解析 | `.ps1` 里只写英文注释；要加中文就先把文件存成 UTF-8 **带 BOM**，但会和其他 4 个文件不一致，不推荐 |
| 新加的可执行脚本在 CI 上 `Permission denied` | Windows 上 `core.fileMode=false`，本地 `chmod +x` 不会被 git 记录 | `git update-index --chmod=+x <file>`，再用 `git ls-files -s` 确认是 `100755` |
| Android 构建报 AGP 依赖 Gradle 内部 API | AGP 8.x 与 Gradle 9.6+ 不兼容（9.6 移除了那些 API） | Android 一律走仓库自带 wrapper（固定 8.14.3），**不要**用系统 gradle |
| 生成签名密钥时 keytool 莫名失败 | git-bash 下 `openssl rand -base64` 输出 CRLF，`tr -d '\n'` 会留一个 `\r` 在密码尾 | 已改成 `tr -d '\r\n'` |
| ESLint 报几千条 `'document' is not defined` | 安卓打包会把手机渲染层拷进 `mobile/android/**/build/`，被一起扫了 | 构建产物目录已在 `eslint.config.mjs` 的 `ignores` 里 |
| Linux 上窗口白屏/黑块 | WebKitGTK 的 GPU 合成问题 | 代码里 `apply_linux_webkit_compat_env()` 会对 NVIDIA 自动设 `WEBKIT_DISABLE_DMABUF_RENDERER`；排查时试 `WEBKIT_DISABLE_COMPOSITING_MODE=1` |
| 想构建 Linux 包但手上没有 Linux | `wry`/`tao` 依赖 webkit2gtk，Windows 无法交叉编译 | 走 CI 的 ubuntu runner，或本机装 WSL2（WSLg 能显示窗口，可真正验证 GUI） |
| 找不到 Android SDK | 机器上可能只有 Unity 自带的那份 | 复用 `<UnityEditor>/Editor/Data/PlaybackEngines/AndroidPlayer/SDK`：里面已有 cmdline-tools（含 sdkmanager）和 platform-tools，补装缺的组件即可；但 Unity 自带 OpenJDK 是 11，AGP 要 17，JDK 得另配 |

---

## 5. 发布链路

三种产物，构建位置和签名方式都不同：

| 产物 | 在哪构建 | 签名 | 备注 |
|---|---|---|---|
| Windows 安装包 / 免安装版 | 本机 `scripts/build-windows.ps1` | 未做代码签名 | 装完关联 `.md`；应用内更新只支持 Windows |
| Android APK | 本机 `mobile/scripts/build-release.sh`（先 `source .env.mobile-release`） | 自己的 upload keystore | 密钥在 `mobile/android/signing/`，密码在 `.env.mobile-release`，两者都在 `.gitignore` 里，**丢了就无法给已装用户送更新** |
| Linux tar.gz / .deb | CI 的 `build-linux` job（`scripts/build-linux.sh`） | 不涉及 | 作为 workflow artifact 产出，发布时下载回来改名上传 |

**发布到 release 时的注意点**：

- 新产物挂在**当前版本对应的那个 release** 上，不要为了挂个包去建新 tag：桌面端更新检查取"最新 release"，
  如果新 tag 里没有 `MD-Previewer-Setup.exe`，所有 Windows 用户会被提示一个更新却找不到安装包。
- `SHA256SUMS.txt` **只追加行**，不要用本地重新构建的 exe 哈希去重算：PE 每次构建字节都不同，
  用本地哈希覆盖会让发布页上的校验值和用户下载到的文件对不上。
- 发布说明里的校验块、下载页表格、`SHA256SUMS.txt` 三处要保持一致。

---

## 6. 已知取舍与待办

诚实清单，别当成 bug 反复排查：

- **iOS 没有发布二进制**：需要 macOS + Xcode + Apple 签名身份。
- **Linux 包未在真机验证 GUI**：只验证过编译、打包、包结构、元数据、二进制可执行性；窗格渲染没实测过。
- **Linux 没有应用内更新**：更新链路只有 Windows 那条。
- **没有 tag 自动发版**：目前 release 资产是手工上传的。
- **截图是手工拍的**：拍摄脚本（含窗口摆位、hover 显示工具栏、点击打开设置面板）留在仓库外，
  下次改 UI 要重拍得重新摸一遍；README/下载页用的是 `docs/*.png`，同一份文件两处引用。
- **`README.md` 里"尚未实现 HTML 清理"的说法已过时**（消毒器早已实现），没顺手改是为了不扩大 diff。
- **URL 大小写不一致**：`Cargo.toml` 与新下载页部分位置写 `md-preview`，更新接口与发布页写 `MD-Previewer`；
  GitHub 不区分大小写所以能用，但别在新代码里扩大这个不一致。
- `verify.sh` 的身份检查会扫描整个 `src/`（模块拆分前只扫 `main.rs`），新增模块不用改脚本。
