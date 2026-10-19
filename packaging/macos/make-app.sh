#!/usr/bin/env bash
# macOS: builds "God of War III.app", which asks for the render resolution and starts the game
# without Terminal (picker-app.applescript). It runs this checkout as built by build.sh, like the
# launcher's PLAY button; run build.sh again after pulling changes.
# Usage: bash packaging/macos/make-app.sh [output folder, default ~/Desktop]
# The icon comes from the game dump (GOW3_GAME_DIR, default ../CUSA01623), else the launcher's.
set -euo pipefail
cd -- "$(dirname -- "$0")/../.."
port=$PWD
dest=${1:-$HOME/Desktop}
app=$dest/God\ of\ War\ III.app
game=${GOW3_GAME_DIR:-$port/../CUSA01623}
icon=$game/sce_sys/icon0.png
[[ -f $icon ]] || icon=$port/launcher/gow3.png

work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT
# The checkout's path as an AppleScript string (backslashes and quotes escaped), inserted as is.
escaped=$(printf '%s' "$port" | sed -e 's/[\\"]/\\&/g')
perl -pe 'BEGIN { $path = shift } s/\@PORT_DIR\@/$path/' "$escaped" \
    packaging/macos/picker-app.applescript > "$work/picker.applescript"
osacompile -o "$work/God of War III.app" "$work/picker.applescript"

# The game's icon instead of the script icon (Assets.car would take precedence over it).
mkdir "$work/icon.iconset"
for size in 16 32 128 256 512; do
    sips -z "$size" "$size" "$icon" --out "$work/icon.iconset/icon_${size}x${size}.png" >/dev/null
    sips -z $((size * 2)) $((size * 2)) "$icon" --out "$work/icon.iconset/icon_${size}x${size}@2x.png" >/dev/null
done
iconutil -c icns "$work/icon.iconset" -o "$work/God of War III.app/Contents/Resources/applet.icns"
rm -f "$work/God of War III.app/Contents/Resources/Assets.car"
plist=$work/God\ of\ War\ III.app/Contents/Info.plist
/usr/libexec/PlistBuddy -c 'Delete :CFBundleIconName' "$plist" 2>/dev/null || true
/usr/libexec/PlistBuddy -c 'Add :CFBundleIdentifier string io.github.gow3-pc.picker' "$plist" 2>/dev/null ||
    /usr/libexec/PlistBuddy -c 'Set :CFBundleIdentifier io.github.gow3-pc.picker' "$plist"
codesign --force --deep -s - "$work/God of War III.app" 2>/dev/null

mkdir -p "$dest"
rm -rf "$app"
cp -R "$work/God of War III.app" "$app"
touch "$app"
echo "Created $app"
