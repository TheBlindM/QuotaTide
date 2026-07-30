#!/bin/sh
set -eu

app_path="${1:?Pass the QuotaTide.app path}"
output_path="${2:?Pass the output DMG path}"
volume_icon="${3:?Pass the volume icon path}"

test -d "$app_path"
test "$(basename "$app_path")" = "QuotaTide.app"
test -f "$volume_icon"
output_directory="$(dirname "$output_path")"
mkdir -p "$output_directory"
staging="$(mktemp -d "$output_directory/quotatide-dmg.XXXXXX")"
cleanup() {
  rm -rf "$staging"
}
trap cleanup EXIT HUP INT TERM

ditto "$app_path" "$staging/QuotaTide.app"
ln -s /Applications "$staging/Applications"
cp "$volume_icon" "$staging/.VolumeIcon.icns"
if command -v SetFile >/dev/null 2>&1; then
  SetFile -a C "$staging"
fi

hdiutil create \
  -ov \
  -srcfolder "$staging" \
  -volname QuotaTide \
  -fs HFS+ \
  -format UDZO \
  -size 128m \
  "$output_path"
hdiutil verify "$output_path"
echo "Created verified 128 MiB-source universal DMG at $output_path"
