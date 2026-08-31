# MD Previewer

**English · [简体中文](README_zh.md)**

[![CI](https://github.com/ArnoldRedman/md-preview/actions/workflows/ci.yml/badge.svg)](https://github.com/ArnoldRedman/md-preview/actions/workflows/ci.yml)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)

A small, local-first Markdown reader and quick editor built with Rust and the system WebView. It does not bundle Chromium or Electron.

![MD Previewer icon](docs/icon.png)

## What it does

- Opens Markdown and text files from the command line, file picker, drag and drop, or OS file associations.
- Keeps multiple documents in tabs and restores the previous session; opening another Markdown file from Windows Explorer reuses the running window.
- Reloads previews when another editor changes the file.
- Supports tables, task lists, syntax highlighting, GitHub alerts, KaTeX, Mermaid, local images, and local document links.
- Includes preview search, content zoom, print, source editing, and reliable autosave.
- Uses native iOS and Android shells for read-only mobile previews.

All rendering assets are bundled locally. **Automatic updates are disabled** until this fork has its own signed release channel.

## Windows one-click build

On Windows, double-click the repository-root file:

```text
build-windows.cmd
```

It runs Rust tests, creates the release executable, builds the installer, and verifies installation/uninstallation in an isolated directory. Outputs are written to `dist`:

```text
dist\MD-Previewer-windows-x64.exe   portable single-file build
dist\MD-Previewer-Setup.exe         per-user installer
dist\SHA256SUMS.txt                 SHA-256 checksums
```

The installer does not require administrator rights. It installs to `%LOCALAPPDATA%\Programs\MD Previewer`, creates Start Menu and uninstall entries, and registers Markdown in the “Open with” list. It uses the system WebView2 runtime and does not bundle WebView2. Use `build-windows.cmd --no-pause` from a terminal or CI.

## Build

```bash
cargo test
cargo build --release
./target/release/md-previewer README.md
```

Windows output: `target/release/md-previewer.exe`.

macOS universal app:

```bash
./bundle.sh
./install.sh
```

Mobile builds:

```bash
cd mobile/ios && xcodegen generate
cd mobile/android && gradle :app:assembleDebug
```

Run the repository verification entry point with:

```bash
./scripts/verify.sh
```

## Privacy and security

MD Previewer has no accounts, telemetry, analytics, or background update requests. Web links open only after user interaction; Markdown, diagrams, formulas, and code highlighting render locally. Documents can still reference remote images or other web resources, which the system WebView may request when rendering them.

The current fork baseline still permits raw HTML from Markdown. Until HTML sanitizing is implemented, open only documents you trust.

## Fork identity

- Product: **MD Previewer**
- Executable/package: `md-previewer`
- Desktop app ID: `io.github.arnoldredman.mdpreviewer`
- Mobile app ID: `io.github.arnoldredman.mdpreviewer.mobile`
- Configuration directory: `md-previewer`

This project is derived from [`vorojar/md-preview`](https://github.com/vorojar/md-preview). See [NOTICE](NOTICE) and [LICENSE](LICENSE) for attribution.

## License

MIT. Copyright notices for the upstream project and this fork are retained in [LICENSE](LICENSE).
