#!/usr/bin/env bash
# 构建 Linux 发布包：tar.gz（解压即用）与 .deb（Debian/Ubuntu）
# 只能在 Linux 上跑：.deb 需要 dpkg-deb，二进制也需要同架构的运行库
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

fail() {
  echo "[linux-package] FAIL: $*" >&2
  exit 1
}

[ "$(uname -s)" = "Linux" ] || fail "Linux packages must be built on Linux"
command -v dpkg-deb >/dev/null 2>&1 || fail "dpkg-deb missing (apt-get install -y dpkg-dev)"

APP_NAME="MD Previewer"
BIN_NAME="md-previewer"
ICON_NAME="md-previewer"
VERSION="$(awk -F\" '/^version = / { print $2; exit }' Cargo.toml)"
ARCH="amd64"
[ "$(uname -m)" = "aarch64" ] && ARCH="arm64"

BINARY="target/release/$BIN_NAME"
[ -f "$BINARY" ] || fail "missing $BINARY (run cargo build --release first)"

DIST="$ROOT/dist"
STAGE="$DIST/linux-stage"
rm -rf "$STAGE"
mkdir -p "$DIST"

# webkit2gtk 是运行期硬依赖：应用主窗口就是系统 WebView，没有它起不来
WEBKIT_DEP="libwebkit2gtk-4.1-0"
sed -e "s/@APP_NAME@/$APP_NAME/g" \
    -e "s/@BIN_NAME@/$BIN_NAME/g" \
    -e "s/@ICON_NAME@/$ICON_NAME/g" \
    -e "s/@VERSION@/$VERSION/g" \
    -e "s/@ARCH@/$ARCH/g" \
    scripts/linux/md-previewer.desktop.in > "$DIST/$ICON_NAME.desktop"

# ---------- tar.gz：二进制 + .desktop + 图标 + 许可证，解压即用 ----------
TAR_STAGE="$STAGE/tar/$BIN_NAME-$VERSION-linux-x64"
mkdir -p "$TAR_STAGE"
install -m 0755 "$BINARY" "$TAR_STAGE/$BIN_NAME"
install -m 0644 "$DIST/$ICON_NAME.desktop" "$TAR_STAGE/$ICON_NAME.desktop"
install -m 0644 assets/linux/md-previewer-256.png "$TAR_STAGE/$ICON_NAME.png"
install -m 0644 LICENSE "$TAR_STAGE/LICENSE"
cat > "$TAR_STAGE/README.md" <<EOF
# $APP_NAME $VERSION (Linux x64)

无需安装：把 $BIN_NAME 放到任意目录直接运行。

    ./$BIN_NAME 你的文档.md

首次运行前请确认系统已装 WebKitGTK 4.1 运行库（多数桌面发行版自带）：

    Debian/Ubuntu  sudo apt install libwebkit2gtk-4.1-0
    Fedora         sudo dnf install webkit2gtk4.1

想让 $APP_NAME 出现在应用菜单里，把 $ICON_NAME.desktop 复制到
~/.local/share/applications/，图标复制到 ~/.local/share/icons/hicolor/256x256/apps/。
桌面文件里的 Exec 是 $BIN_NAME，所以请把它放进 PATH 里的目录（例如 ~/.local/bin）。
EOF

TARBALL="$DIST/$BIN_NAME-$VERSION-linux-x64.tar.gz"
tar -C "$STAGE/tar" -czf "$TARBALL" "$(basename "$TAR_STAGE")"

# ---------- .deb ----------
DEB_ROOT="$STAGE/deb"
mkdir -p "$DEB_ROOT/DEBIAN" \
         "$DEB_ROOT/usr/bin" \
         "$DEB_ROOT/usr/share/applications" \
         "$DEB_ROOT/usr/share/icons/hicolor/256x256/apps" \
         "$DEB_ROOT/usr/share/icons/hicolor/512x512/apps" \
         "$DEB_ROOT/usr/share/doc/$BIN_NAME"

install -m 0755 "$BINARY" "$DEB_ROOT/usr/bin/$BIN_NAME"
install -m 0644 "$DIST/$ICON_NAME.desktop" "$DEB_ROOT/usr/share/applications/$ICON_NAME.desktop"
install -m 0644 assets/linux/md-previewer-256.png "$DEB_ROOT/usr/share/icons/hicolor/256x256/apps/$ICON_NAME.png"
install -m 0644 assets/linux/md-previewer-512.png "$DEB_ROOT/usr/share/icons/hicolor/512x512/apps/$ICON_NAME.png"
install -m 0644 LICENSE "$DEB_ROOT/usr/share/doc/$BIN_NAME/copyright"
gzip -9 -n -c CHANGELOG.md > "$DEB_ROOT/usr/share/doc/$BIN_NAME/changelog.gz"

INSTALLED_SIZE="$(du -k -s "$DEB_ROOT/usr" | cut -f1)"
cat > "$DEB_ROOT/DEBIAN/control" <<EOF
Package: $BIN_NAME
Version: $VERSION
Section: utils
Priority: optional
Architecture: $ARCH
Depends: $WEBKIT_DEP, libgtk-3-0
Recommends: xdg-utils
Installed-Size: $INSTALLED_SIZE
Maintainer: ArnoldRedman <https://github.com/ArnoldRedman/MD-Previewer>
Homepage: https://arnoldredman.github.io/MD-Previewer/
Description: Lightweight local-first Markdown reader
 MD Previewer renders Markdown with the WebKitGTK engine that ships with your
 desktop, so it starts fast and stays small. Tabs, document outline, full-text
 search, encoding detection, KaTeX math and Mermaid diagrams all work offline.
 .
 The application makes no network requests while reading documents.
EOF

DEB="$DIST/${BIN_NAME}_${VERSION}_${ARCH}.deb"
dpkg-deb --root-owner-group --build "$DEB_ROOT" "$DEB" >/dev/null

rm -rf "$STAGE"

echo "[linux-package] version $VERSION ($ARCH)"
(cd "$DIST" && sha256sum "$(basename "$TARBALL")" "$(basename "$DEB")")
