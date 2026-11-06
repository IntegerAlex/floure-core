#!/usr/bin/env bash
# Build a portable tarball: binary + desktop entry, icon, metainfo, licences.
# Run from the repository root after `cargo build --release -p floure-app`.
set -euo pipefail

version=$(cargo metadata --no-deps --format-version 1 \
  | python3 -c 'import json,sys; print(json.load(sys.stdin)["packages"][0]["version"])')
arch=$(uname -m)
name="floure-${version}-${arch}"
stage="target/portable/${name}"

rm -rf "$stage"
mkdir -p "$stage"/{bin,share/applications,share/metainfo,share/icons/hicolor/512x512/apps,share/licenses/floure}

install -Dm755 target/release/floure "$stage/bin/floure"
install -Dm644 packaging/floure.desktop "$stage/share/applications/floure.desktop"
install -Dm644 packaging/floure.metainfo.xml "$stage/share/metainfo/floure.metainfo.xml"
install -Dm644 packaging/icons/512x512.png \
  "$stage/share/icons/hicolor/512x512/apps/floure.png"
install -Dm644 LICENSE "$stage/share/licenses/floure/LICENSE"
install -Dm644 crates/floure-app/assets/fonts/OFL.txt \
  "$stage/share/licenses/floure/OFL-Inter.txt"

cat > "$stage/INSTALL.md" <<'EOF'
# Floure portable build

Copy `bin/floure` to a directory on your PATH, for example:

    install -Dm755 bin/floure ~/.local/bin/floure

Then install the desktop entry and icon (optional):

    install -Dm644 share/applications/floure.desktop ~/.local/share/applications/floure.desktop
    install -Dm644 share/icons/hicolor/512x512/apps/floure.png \
      ~/.local/share/icons/hicolor/512x512/apps/floure.png

The binary needs Vulkan or OpenGL drivers. Models download on first run.
On Wayland, bind `floure --toggle` in your compositor; on X11/Windows a
global shortcut is registered when available.
EOF

tar -C target/portable -czf "target/portable/${name}.tar.gz" "$name"
echo "target/portable/${name}.tar.gz"
