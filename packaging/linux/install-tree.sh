#!/usr/bin/env bash
# Install ConstructCAD into a file tree under <root>/usr, the layout shared by
# the .deb package and the AppImage:
#   usr/bin/constructcad, usr/bin/ccad
#   usr/share/applications/constructcad.desktop
#   usr/share/icons/hicolor/{256x256,512x512}/apps/constructcad.png
#   usr/share/mime/packages/constructcad.xml
#   usr/share/doc/constructcad/ (README, CHANGELOG, license, sample project)
#
# Usage: packaging/linux/install-tree.sh <dir with the release binaries> <root>
# Run from the repository root.
set -euo pipefail

bin_dir=$1
root=$2
here=packaging/linux

install -Dm755 "$bin_dir/constructcad" "$root/usr/bin/constructcad"
install -Dm755 "$bin_dir/ccad" "$root/usr/bin/ccad"
install -Dm644 "$here/constructcad.desktop" "$root/usr/share/applications/constructcad.desktop"
install -Dm644 assets/icons/app-icon-256.png "$root/usr/share/icons/hicolor/256x256/apps/constructcad.png"
install -Dm644 assets/icons/app-icon.png "$root/usr/share/icons/hicolor/512x512/apps/constructcad.png"
install -Dm644 "$here/constructcad.xml" "$root/usr/share/mime/packages/constructcad.xml"
doc="$root/usr/share/doc/constructcad"
install -Dm644 README.md "$doc/README.md"
install -Dm644 README.pt-BR.md "$doc/README.pt-BR.md"
install -Dm644 CHANGELOG.md "$doc/CHANGELOG.md"
install -Dm644 LICENSE "$doc/LICENSE"
install -Dm644 examples/casa_simples.ccad "$doc/examples/casa_simples.ccad"
