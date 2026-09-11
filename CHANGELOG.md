# Changelog

This file tracks MD Previewer releases. The upstream MD Preview history remains available at <https://github.com/vorojar/md-preview/releases>.

## 1.5.0

- In-app update download with progress: "Update Now" now downloads the release asset inside the app and shows a progress bar (bytes and percentage) in the update dialog, with a retryable error state. Only the final swap-and-relaunch step still runs a helper script, and it is started without any console window, so no PowerShell window appears.
- Appearance setting: choose System / Light / Dark in the settings panel. The choice is stored in `settings.json` (the old macOS `theme.txt` is migrated on first launch) and applied to both the window and the WebView, so Windows users can force dark mode regardless of the system theme.

## 1.4.1

- Enhanced recent files management in sidebar:
  - Hovering over a recent file now displays a tooltip with the complete physical directory path.
  - Right-click context menu added for recent entries: "从历史中删除" (Remove from History), "打开所在目录" (Reveal in File Manager), and "复制路径" (Copy Path).
  - Added "清空历史" (Clear History) one-click action button at the bottom of the recent list with confirmation.
- Security and raw HTML sanitization hardening:
  - Comprehensive filtering of embedded raw HTML within Markdown: strips `<script>` tags, inline event attributes (`onclick`, `onload`, etc.), `<style>`, `<svg>`, and custom `data-*` attributes.
  - Hardened link and navigation interceptors against `javascript:`, `data:`, `blob:`, and obfuscated entity URL schemes.
- State synchronization & edge-case crash prevention:
  - Single-tab mode dirty document protection: active unsaved documents can no longer be silently superseded by opening a new file.
  - Window close protection now consults session-level dirty tracking; returns `save-skipped` when clean to eliminate deadlock.
  - Encoding switches automatically save dirty content before transcoding; relocating missing files resets detected encoding.
  - Unified modal and popover overlay hierarchy: pressing `Escape` closes one layer at a time.
  - Fixed startup blank screen issue by allowing initial in-memory document data URI navigation while continuing to intercept subsequent script-bearing protocols.
  - Architectural consolidation: introduced `App` structure to unify event loop state and deduplicate file persistence, repaint, and directory watching logic.

## 1.4.0

- Support opening any file extension as plain text:
  - Any non-binary text file (source code, JSON, YAML, configs, logs, arbitrary extensions) can now be opened directly.
  - Automatic 8 KB binary sniffing prevents accidental opening of binary files (PE executables, libraries, archives, media).
  - Open File dialog filters updated with "Supported Documents", "Markdown", "Text", and "All Files (*)".
- In-place document encoding conversion and Save As:
  - Encoding popover now offers two distinct groups: "重新以指定编码打开" (Reopen With) and "转换为指定编码" (Convert To).
  - Supports converting between UTF-8, UTF-8 BOM, GBK / ANSI, UTF-16 LE, and UTF-16 BE in-place.
  - Added character loss safety warning dialog when converting text with characters unrepresentable in GBK.
  - Added "另存为" (Save As) via `Ctrl+Shift+S` / `Cmd+Shift+S` and encoding popover entry, preserving tab state and resolving conflicting open tabs.
  - Full UTF-8 BOM detection and preservation on save.
- In-app update notes Markdown rendering:
  - Update release notes are now rendered via the native Markdown renderer with full styling (headings, lists, code blocks, blockquotes, links).
  - Heading IDs are automatically sanitized to prevent anchor conflicts with the main document table of contents.

## 1.3.1

- Release build for automated update channel deployment:
  - Validates and delivers in-app auto-update from v1.3.0 and prior versions
  - Features the new brand application icon assets
  - Includes download failure auto-recovery to prevent process dropping

## 1.3.0

- Updated brand application icon across all platforms:
  - Replaced the legacy icon with a freshly designed, crisp document glyph (`< • >`)
  - Full multi-resolution support for Windows ICO, macOS ICNS, iOS AppIcon, and Android mipmap assets
  - Added build.rs dependency tracking to ensure icon assets trigger automatic resource recompilation
- Added automatic update checking and Windows in-app one-click self-upgrade:
  - Startup silent check (deferred by 2.5s) and periodic 4-hour background polling via GitHub Releases API
  - Non-intrusive pulsating rocket badge (`🚀 v1.3.x`) in the top-right toolbar when an update is available
  - Release notes modal dialog displaying version badge, release name, scrollable changelog, and direct actions
  - "立即更新" (Update Now) one-click in-place update for portable users: automatically resolves current EXE path, downloads new standalone executable in the background, safely overwrites the old EXE in-place without manual file searching or moving, and restarts with session tabs preserved
  - Double-layer cleanup architecture: temporary download files and update scripts are deleted immediately on completion, plus startup sweep for historical leftovers
  - "检查更新" (Check for Updates) button integrated into the Settings popover with clean layout separator and adaptive update status
  - Fixed CSS selector specificity in the Settings popover to eliminate button clipping and text overflow
  - Retained "前往 GitHub 下载" (View on GitHub) option for users who prefer manual downloads or viewing release assets

## 1.2.1

- Added document encoding display and switcher:
  - Displays the active encoding format in the top-right tab bar next to document statistics
  - Clicking the encoding label opens a popover to quickly switch between UTF-8, GBK / ANSI, UTF-16 LE, and UTF-16 BE
  - Automatically decodes and re-renders the document upon switching, eliminating mojibake without external conversion tools
  - Respects the selected encoding when saving changes back to disk
- Optimized split-view editing mode:
  - Repositioned the source editor to the left and the live rendered preview to the right
  - Added a distinct 2px vertical divider line and subtle pane contrast matching IDE/Rider preview ergonomics
  - Fixed the split-view toolbar button icon to be precisely centered in its bounding box
- Enhanced reading and editing UX:
  - Added one-click copy buttons to code blocks on both desktop and mobile previews
  - Added Table of Contents (TOC Outline) navigation in the collapsible sidebar with smooth anchor jumping
  - Added an image lightbox viewer with click-to-zoom, mouse drag panning, and zoom controls
  - Added tab context menu (close other tabs, copy path, reveal in Windows Explorer / macOS Finder)
  - Added a word wrap toggle in the settings popover for code blocks and editor source

## 1.2.0

- Added native plain text (`.txt`) document reading and preview support:
  - Preserves original line breaks, whitespace, and tabs without Markdown formatting collisions
  - Safely escapes HTML special characters while avoiding false header, italic, or table rendering
  - Automatically disables LaTeX math and Mermaid diagrams for `.txt` files to avoid parsing errors
  - Full support for content zoom (Cmd/Ctrl +/-/0), in-page search (Cmd/Ctrl+F), and source editing (Cmd/Ctrl+E)
  - Extended Android and mobile preview shells to accept `.txt` and `text/plain` files
- Improved editing mode layout:
  - Docked the floating toolbar into a dedicated sticky top bar when entering edit mode, preventing toolbar controls from obscuring the top lines of source text
- Fixed author mode copying:
  - Ensured copied text extracts clean plain text without retaining raw Markdown syntax markers
- Added product screenshots:
  - Added preview mode and edit/settings mode screenshots to `docs/` and updated `README.md` / `README_zh.md`

## 1.1.1

- Fixed author mode copying Markdown source instead of the rendered text: **Copy body** now takes the visible article text from the rendered document, with blank lines between paragraphs and no `#`, `**`, `` ` ``, `>` or `-` markers
- Fixed the two heading buttons for the same reason: inline Markdown in a chapter heading is no longer copied along with the text

## 1.1.0

Windows-only release: the macOS and mobile shells are unchanged but are not published as packages yet.

- Added a settings panel behind the toolbar gear, stored in `settings.json` next to the other per-user files
- Added an "opening a Markdown file" choice between reusing the running window as a new tab and giving every file its own window
- Added a tab bar choice between accumulating tabs across launches and keeping only the current document
- Added a collapsible left sidebar that lists the current document's folder and recently opened files, so switching between chapters no longer needs the file picker; opening it widens the window instead of squeezing the text column
- Added an author mode that puts copy buttons beside the chapter heading and above the body, for copying the heading line, the title without its chapter number, and the whole body in one click
- Reused the running Windows window when another Markdown file is opened from Explorer, adding or activating a tab instead of creating a second window

## 0.1.0

- Established the independent **MD Previewer** product identity
- Changed executable, configuration, Windows ProgID, macOS bundle ID, Finder scheme, iOS bundle ID, and Android application ID so this fork can coexist with upstream
- Replaced the upstream icon and removed upstream marketing screenshots, App Store identity, signing team, and release evidence
- Disabled and removed automatic update checks, Sparkle integration, and Windows self-update until this fork has its own signed release channel
- Retained upstream copyright and MIT attribution in `LICENSE` and `NOTICE`
