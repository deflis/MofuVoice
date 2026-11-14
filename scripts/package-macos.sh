#!/usr/bin/env bash
set -euo pipefail

# Package the Cargo-built GUI; no WebView or Node toolchain is involved.
root="$(cd "$(dirname "$0")/.." && pwd)"
cd "$root"
app="$root/target/release/bundle/macos/MofuVoice.app"
rm -rf "$app"
mkdir -p "$app/Contents/MacOS" "$app/Contents/Resources" dist
cp target/release/MofuVoice "$app/Contents/MacOS/MofuVoice"
version="$(cargo pkgid --offline | sed 's/.*#\([^@]*@\)\{0,1\}//')"
case "$version" in *[!0-9.]*|'') echo "Cannot determine package version: $version" >&2; exit 1;; esac

iconset="$(mktemp -d)/MofuVoice.iconset"
mkdir -p "$iconset"
for pixels in 16 32 128 256 512; do
  sips -z "$pixels" "$pixels" icons/icon.png --out "$iconset/icon_${pixels}x${pixels}.png" >/dev/null
  double=$((pixels * 2))
  sips -z "$double" "$double" icons/icon.png --out "$iconset/icon_${pixels}x${pixels}@2x.png" >/dev/null
done
iconutil -c icns "$iconset" -o "$app/Contents/Resources/MofuVoice.icns"
cp Info.plist "$app/Contents/Info.plist"
/usr/libexec/PlistBuddy -c "Set :CFBundleShortVersionString $version" "$app/Contents/Info.plist"
/usr/libexec/PlistBuddy -c "Set :CFBundleVersion $version" "$app/Contents/Info.plist"
codesign --force --deep --sign - "$app"
codesign --verify --deep --strict "$app"

stage="$(mktemp -d)"
mount="$(mktemp -d)"
cleanup() {
  hdiutil detach "$mount" -force >/dev/null 2>&1 || true
  rm -rf "$stage" "$mount" "$(dirname "$iconset")"
}
trap cleanup EXIT
ditto "$app" "$stage/MofuVoice.app"
ln -s /Applications "$stage/Applications"
arch="$(uname -m)"
dmg="dist/MofuVoice_${version}_${arch}.dmg"
hdiutil create -volname MofuVoice -srcfolder "$stage" -ov -format UDZO "$dmg"
hdiutil attach "$dmg" -readonly -nobrowse -mountpoint "$mount"
test -x "$mount/MofuVoice.app/Contents/MacOS/MofuVoice"
test -L "$mount/Applications"
codesign --verify --deep --strict "$mount/MofuVoice.app"
hdiutil detach "$mount"
tar -czf dist/MofuVoice.app.tar.gz -C "$(dirname "$app")" MofuVoice.app
cp LICENSE dist/
