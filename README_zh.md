# MD Previewer

**[English](README.md) · 简体中文**

[![CI](https://github.com/ArnoldRedman/md-preview/actions/workflows/ci.yml/badge.svg)](https://github.com/ArnoldRedman/md-preview/actions/workflows/ci.yml)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)

MD Previewer 是一个体积小、本地优先的 Markdown 阅读器和快速编辑器，桌面端使用 Rust 与系统 WebView，不内置 Chromium，也不依赖 Electron。

![MD Previewer 图标](docs/icon.png)

## 效果展示

### 预览模式
![MD Previewer 预览模式](docs/preview.png)

### 编辑模式与设置面板
![MD Previewer 编辑模式与设置面板](docs/editor.png)

## 当前功能

- 从命令行、文件选择器、拖放或系统文件关联打开 Markdown 与任意文本文件（`.txt`、`.json`、`.toml`、`.yaml`、`.log`、源码、无扩展名文件等），二进制文件会被拒绝
- 非 Markdown 文件按纯文本排版：等宽字体，完整保留换行与缩进，安全字符转义，不发生 Markdown 语法冲突
- 文档编码识别、转码与另存为：右上角实时显示编码，支持按 UTF-8、UTF-8 BOM、GBK/ANSI、UTF-16 LE、UTF-16 BE 重新打开以纠正乱码，也能把文件原地转换为任一编码（会丢字符时先确认），Ctrl/Cmd+Shift+S 另存为
- 双栏实时预览：左侧源码编辑、右侧实时渲染，配备 Rider 风格竖向清晰分界线与精准居中切换按钮
- 多文档标签与会话恢复；在 Windows 资源管理器中再次打开文件会复用现有窗口
- 标签右键快捷菜单：关闭其他标签、一键复制文件绝对路径、在系统文件管理器中快速定位
- 可折叠的左侧栏：包含所在目录文件列表、最近打开历史，以及全新大纲目录（TOC Outline）与锚点跳转
- 代码块一键复制按钮（桌面与移动端均已原生适配）
- 图片点击灯箱（Lightbox）放大查看，支持缩放控制与鼠标拖拽平移
- 工具栏齿轮设置面板：支持窗口复用偏好、标签累计模式、代码块/正文软换行（Word Wrap）开关以及手动检查更新
- 自动版本检测与一键更新：应用启动后延迟静默检测、后台每隔 4 小时轻量轮询 GitHub Releases，检测到新版后顶栏高亮徽标提醒，支持查看更新日志及 Windows 原生一键下载替换重启
- 作者模式：在章节标题旁和正文上方加复制按钮，分别复制整行标题、去掉「第 N 章」序号的标题，以及全部正文
- 外部编辑器写盘后自动刷新
- 表格、任务列表、代码高亮、GitHub Alerts、KaTeX、Mermaid、本地图片和本地文档链接
- 预览搜索、正文缩放、打印、源码编辑和可靠自动保存
- iOS 与 Android 原生只读预览外壳

渲染资源全部离线内置。自动更新检查通过 GitHub Releases API 安全校验与更新。

目前发布的安装包**仅有 Windows**。macOS 与移动端外壳可以从源码构建，但因为本 fork 还没有签名身份，暂不发包。

## Windows 一键构建

在 Windows 上直接双击仓库根目录的：

```text
build-windows.cmd
```

脚本会依次运行 Rust 测试、Release 构建、安装包生成，以及隔离目录中的安装/卸载自测。产物位于 `dist`：

```text
dist\MD-Previewer-windows-x64.exe   单文件便携版
dist\MD-Previewer-Setup.exe         当前用户安装包
dist\SHA256SUMS.txt                 SHA-256 校验值
```

安装包不需要管理员权限，默认安装到 `%LOCALAPPDATA%\Programs\MD Previewer`，并创建开始菜单入口、卸载入口和 Markdown“打开方式”。它依赖系统 WebView2，不会把 WebView2 打进安装包。命令行或 CI 可用 `build-windows.cmd --no-pause` 跳过结束暂停。

## 构建

```bash
cargo test
cargo build --release
./target/release/md-previewer README.md
```

Windows 产物：`target/release/md-previewer.exe`。

macOS 通用应用：

```bash
./bundle.sh
./install.sh
```

移动端：

```bash
cd mobile/ios && xcodegen generate
cd mobile/android && gradle :app:assembleDebug
```

仓库统一验证入口：

```bash
./scripts/verify.sh
```

## 隐私与安全

MD Previewer 没有账号、遥测、分析统计或后台更新请求。网页链接只在用户主动点击后交给系统打开；Markdown、图表、公式和代码高亮都在本机渲染。文档仍可能引用远程图片或其他网络资源，系统 WebView 在渲染时可能请求这些资源。

当前 fork 基线仍允许 Markdown 原始 HTML。在 HTML 清理功能完成前，请只打开可信文档。

## Fork 身份

- 产品名：**MD Previewer**
- 可执行文件/包名：`md-previewer`
- 桌面 App ID：`io.github.arnoldredman.mdpreviewer`
- 移动端 App ID：`io.github.arnoldredman.mdpreviewer.mobile`
- 配置目录：`md-previewer`

本项目派生自 [`vorojar/md-preview`](https://github.com/vorojar/md-preview)。归属信息见 [NOTICE](NOTICE) 与 [LICENSE](LICENSE)。

## 许可证

MIT。上游项目与当前 fork 的版权声明均保留在 [LICENSE](LICENSE) 中。
