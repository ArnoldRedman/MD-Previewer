# Changelog

This file tracks MD Previewer releases. The upstream MD Preview history remains available at <https://github.com/vorojar/md-preview/releases>.

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
