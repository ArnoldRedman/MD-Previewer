# Changelog

This file tracks MD Previewer releases. The upstream MD Preview history remains available at <https://github.com/vorojar/md-preview/releases>.

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
