# MD Previewer

**[English](README.md) · 简体中文**

[![CI](https://github.com/ArnoldRedman/md-preview/actions/workflows/ci.yml/badge.svg)](https://github.com/ArnoldRedman/md-preview/actions/workflows/ci.yml)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)

MD Previewer 是一个体积小、本地优先的 Markdown 阅读器和快速编辑器，桌面端使用 Rust 与系统 WebView，不内置 Chromium，也不依赖 Electron。

![MD Previewer 图标](docs/icon.png)

## 当前功能

- 从命令行、文件选择器、拖放或系统文件关联打开 Markdown 与文本文件
- 多文档标签与会话恢复；在 Windows 资源管理器中再次打开 Markdown 会复用现有窗口
- 外部编辑器写盘后自动刷新
- 表格、任务列表、代码高亮、GitHub Alerts、KaTeX、Mermaid、本地图片和本地文档链接
- 预览搜索、正文缩放、打印、源码编辑和可靠自动保存
- iOS 与 Android 原生只读预览外壳

渲染资源全部离线内置。**自动更新当前已禁用**，等本 fork 建立独立且可信的签名发布通道后再启用。

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
