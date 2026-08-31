#!/usr/bin/env bash
# Regenerate the AppIcon set from the shared brand source, using only native `sips`.
# The twin of the Windows Images/generate-assets.ps1, same source icon, one command to rebrand.
#
# Source: whichever image `brand_icon_source` resolves, Allodia's when `branding/allodia-icon.png`
# is present, the neutral one otherwise (docs/branding.md). Output:
# App/Assets.xcassets/AppIcon.appiconset/ (committed PNGs + Contents.json), which project.yml wires
# in via ASSETCATALOG_COMPILER_APPICON_NAME.
#
# The one set carries BOTH platforms: the per-size `mac` idiom images (macOS) and the single-size
# 1024 `universal`/`ios` image (iPhone + iPad, Xcode derives every runtime iOS size from it). The
# source has no alpha, so the iOS icon is App-Store-valid (Apple rejects an iOS icon with an alpha
# channel). project.yml scopes ASSETCATALOG_COMPILER_APPICON_NAME per SDK so each platform picks its
# own images from this shared set.
#
#   Scripts/generate-appicon.sh [path/to/source.png]
#
# Note: this is a straight downscale, the macOS "squircle" shape/padding treatment is a design
# follow-up (see clients/apple/README.md). A full-bleed icon is functional and Store-valid.
set -euo pipefail

HERE="$(cd "$(dirname "$0")/.." && pwd)"   # clients/apple
ROOT="$(cd "$HERE/../.." && pwd)"          # repo root (worktree)
. "$ROOT/scripts/dev/brand.sh"
SRC="${1:-$(brand_icon_source)}"
OUT="$HERE/App/Assets.xcassets/AppIcon.appiconset"

if [[ ! -f "$SRC" ]]; then
  echo "generate-appicon: source not found: $SRC" >&2
  exit 1
fi

mkdir -p "$OUT"

# One line per emitted file: "<filename> <pixel size>". macOS wants 16/32/128/256/512 pt at @1x and
# @2x (ten files across seven pixel sizes); iOS wants one single-size 1024 image (icon_1024.png),
# from which Xcode's asset compiler derives every runtime iPhone/iPad size at build time.
while read -r name px; do
  [[ -z "$name" ]] && continue
  sips -z "$px" "$px" "$SRC" --out "$OUT/$name" >/dev/null
done <<'ICONS'
icon_16x16.png 16
icon_16x16@2x.png 32
icon_32x32.png 32
icon_32x32@2x.png 64
icon_128x128.png 128
icon_128x128@2x.png 256
icon_256x256.png 256
icon_256x256@2x.png 512
icon_512x512.png 512
icon_512x512@2x.png 1024
icon_1024.png 1024
ICONS

cat >"$OUT/Contents.json" <<'JSON'
{
  "images" : [
    { "idiom" : "universal", "platform" : "ios", "size" : "1024x1024", "filename" : "icon_1024.png" },
    { "idiom" : "mac", "scale" : "1x", "size" : "16x16", "filename" : "icon_16x16.png" },
    { "idiom" : "mac", "scale" : "2x", "size" : "16x16", "filename" : "icon_16x16@2x.png" },
    { "idiom" : "mac", "scale" : "1x", "size" : "32x32", "filename" : "icon_32x32.png" },
    { "idiom" : "mac", "scale" : "2x", "size" : "32x32", "filename" : "icon_32x32@2x.png" },
    { "idiom" : "mac", "scale" : "1x", "size" : "128x128", "filename" : "icon_128x128.png" },
    { "idiom" : "mac", "scale" : "2x", "size" : "128x128", "filename" : "icon_128x128@2x.png" },
    { "idiom" : "mac", "scale" : "1x", "size" : "256x256", "filename" : "icon_256x256.png" },
    { "idiom" : "mac", "scale" : "2x", "size" : "256x256", "filename" : "icon_256x256@2x.png" },
    { "idiom" : "mac", "scale" : "1x", "size" : "512x512", "filename" : "icon_512x512.png" },
    { "idiom" : "mac", "scale" : "2x", "size" : "512x512", "filename" : "icon_512x512@2x.png" }
  ],
  "info" : { "author" : "xcode", "version" : 1 }
}
JSON

echo "==> Wrote AppIcon set (macOS 16-1024 + iOS 1024) to $OUT"
