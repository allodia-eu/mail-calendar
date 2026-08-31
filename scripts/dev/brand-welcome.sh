#!/usr/bin/env bash
# Cut every client's welcome illustration from the one brand source.
#
#   scripts/dev/brand-welcome.sh [path/to/source.png]
#
# The twin of brand-icons.sh, and the same rule decides the source: `branding/allodia-welcome.png`
# if the checkout has it, `branding/default-welcome.png` otherwise (docs/branding.md). A separate
# slot from the icon deliberately; the neutral welcome art is a copy of the neutral icon today, and
# giving it its own file is what lets art drawn for the screen replace it later without the launcher
# icon following along.
#
# The Linux client draws no illustration, so there are three clients here rather than four. Outputs
# are committed, because no client generates art at build time.
#
# Why the encodings differ, which is the part that is not obvious:
#   * Android; **lossy** webp, q90. The art is flat shapes over smooth gradients, the worst case for
#     lossless: the five density buckets cost ~360 KB lossless against ~108 KB lossy, for no visible
#     difference. Alpha stays lossless so the outline keeps its edge.
#   * Apple; PNG, quantized to 256 colours with **dithering off**. An asset catalog is the only
#     reliable @1x/@2x/@3x path and actool wants PNG; dithering flat art adds noise that costs bytes
#     and buys nothing.
#   * Windows; one PNG at 3x; WinUI scales it down for lower-DPI displays.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
# shellcheck source=scripts/dev/lib.sh
. "$ROOT/scripts/dev/lib.sh"   # brand_welcome_source + imagemagick_bin
cd "$ROOT"

SRC="${1:-$(brand_welcome_source)}"
[ -f "$SRC" ] || { echo "brand-welcome: source not found: $SRC" >&2; exit 1; }

IM="$(imagemagick_bin || true)"
[ -n "$IM" ] || { echo "brand-welcome: need ImageMagick (brew install imagemagick)" >&2; exit 1; }
im() { "$IM" "$@"; }

echo "==> Source art: $SRC"

# The resource is named for the slot, not for what is currently in it: an unbranded build draws
# something that is not a robot, and a resource called `allodia_robot` would then be a lie in three
# clients at once.
NAME=welcome_art

# Android: one hero image per density bucket, sized for a 160dp box (1x/1.5x/2x/3x/4x).
for pair in mdpi:160 hdpi:240 xhdpi:320 xxhdpi:480 xxxhdpi:640; do
  out="clients/android/app/src/main/res/drawable-${pair%%:*}"
  mkdir -p "$out"
  im "$SRC" -resize "${pair##*:}x${pair##*:}" -strip \
    -quality 90 -define webp:method=6 -define webp:alpha-quality=100 \
    "$out/$NAME.webp"
done

# Apple: an asset catalog inside MailcalUI, loaded via `Image("WelcomeArt", bundle: .module)`.
imgset="clients/apple/Packages/MailcalKit/Sources/MailcalUI/Assets.xcassets/WelcomeArt.imageset"
mkdir -p "$imgset"
im "$SRC" -resize 160x160 -strip -dither None -colors 256 "$imgset/welcome-art.png"
im "$SRC" -resize 320x320 -strip -dither None -colors 256 "$imgset/welcome-art@2x.png"
im "$SRC" -resize 480x480 -strip -dither None -colors 256 "$imgset/welcome-art@3x.png"
cat >"$imgset/Contents.json" <<'JSON'
{
  "images" : [
    { "idiom" : "universal", "scale" : "1x", "filename" : "welcome-art.png" },
    { "idiom" : "universal", "scale" : "2x", "filename" : "welcome-art@2x.png" },
    { "idiom" : "universal", "scale" : "3x", "filename" : "welcome-art@3x.png" }
  ],
  "info" : { "author" : "xcode", "version" : 1 }
}
JSON

# Windows: copied next to the exe by Mailcal.csproj, loaded as ms-appx:///Images/welcome-art.png.
im "$SRC" -resize 480x480 -strip -dither None -colors 256 \
  "clients/windows/Mailcal/Images/welcome-art.png"

echo "==> Wrote the welcome illustration for Android, Apple and Windows"
