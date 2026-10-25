#!/usr/bin/env bash
# Build the AppImage (x86_64) from the release binaries with appimagetool.
#
# Usage: packaging/linux/build-appimage.sh <dir with the release binaries> <output .AppImage>
#                                          <appimagetool> <type2 runtime>
# Run from the repository root, on the oldest supported Ubuntu LTS (the binaries
# use the glibc of the build machine), with patchelf and the libraries below
# installed. The release workflow downloads pinned, checksum-verified copies of
# appimagetool and of the runtime.
set -euo pipefail

bin_dir=$1
out=$2
appimagetool=$3
runtime=$4

work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT
appdir="$work/ConstructCAD.AppDir"
packaging/linux/install-tree.sh "$bin_dir" "$appdir"

# winit loads the keyboard and X11 libraries with dlopen and fails without them;
# minimal systems lack some. Bundle them with the libraries they need (and their
# copyright files); RUNPATH lets the binaries find them, and the libraries each
# other. libX11, libX11-xcb, libxcb and libwayland-client stay with the host
# (AppImage excludelist).
bundled_libs="libxkbcommon.so.0 libxkbcommon-x11.so.0 libxcb-xkb.so.1
    libXcursor.so.1 libXrender.so.1 libXfixes.so.3 libXi.so.6 libXext.so.6"
mkdir -p "$appdir/usr/lib"
for lib in $bundled_libs; do
    path=$(ldconfig -p | awk -v l="$lib" '$1 == l && /x86-64/ { print $NF; exit }')
    if [ -z "$path" ]; then
        echo "error: $lib not found on the build machine" >&2
        exit 1
    fi
    cp -L "$path" "$appdir/usr/lib/$lib"
    patchelf --set-rpath '$ORIGIN' "$appdir/usr/lib/$lib"
    pkg=$(dpkg -S "$(readlink -f "$path")" | head -n1 | cut -d: -f1)
    install -Dm644 "/usr/share/doc/$pkg/copyright" "$appdir/usr/share/doc/$pkg/copyright"
done
for exe in constructcad ccad; do
    patchelf --set-rpath '$ORIGIN/../lib' "$appdir/usr/bin/$exe"
done

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
