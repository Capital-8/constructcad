#!/usr/bin/env bash
# Build the AppImage (x86_64) from the release binaries with appimagetool.
#
# Usage: packaging/linux/build-appimage.sh <dir with the release binaries> <output .AppImage>
#                                          <appimagetool> <type2 runtime>
# Run from the repository root. The release workflow downloads pinned,
# checksum-verified copies of appimagetool and of the runtime.
set -euo pipefail

bin_dir=$1
out=$2
appimagetool=$3
runtime=$4

work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT
appdir="$work/ConstructCAD.AppDir"
packaging/linux/install-tree.sh "$bin_dir" "$appdir"

# The AppDir root holds the desktop file, the icon and the entry point.
cp packaging/linux/constructcad.desktop "$appdir/constructcad.desktop"
cp assets/icons/app-icon-256.png "$appdir/constructcad.png"
ln -s constructcad.png "$appdir/.DirIcon"
cat > "$appdir/AppRun" <<'EOF'
#!/bin/sh
# Start ConstructCAD from wherever the AppImage is mounted.
here="$(dirname "$(readlink -f "$0")")"
exec "$here/usr/bin/constructcad" "$@"
EOF
chmod 755 "$appdir/AppRun"

mkdir -p "$(dirname "$out")"
# The runtime option goes first; it unpacks appimagetool instead of mounting
# it, so the build machine needs no FUSE.
ARCH=x86_64 "$appimagetool" --appimage-extract-and-run --no-appstream --runtime-file "$runtime" "$appdir" "$out"
