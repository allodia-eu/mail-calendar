#!/usr/bin/env bash
# Derive the Linux client's hicolor icons from the one brand source.
#
# The twin of clients/apple/Scripts/generate-appicon.sh, clients/android/generate-icons.sh and
# clients/windows/Mailcal/Images/generate-assets.ps1; same source icon, and
# scripts/dev/brand-icons.sh runs whichever of the four this host can. The outputs are committed,
# because no client generates art at build time and the GNOME SDK carries no image tooling to do it
# with.
#
#   clients/linux/flatpak/generate-icons.sh [path/to/source.png]
#
# The sizes are the hicolor buckets a GTK shell actually reads: 128 is the floor a software centre
# needs, 256 is what GNOME Shell draws in the app grid at 2x, and 512 is what a software centre uses
# for the page header. Below 128 the shell downscales the 128 itself, so smaller buckets buy
# nothing but bytes.
set -euo pipefail

HERE="$(cd "$(dirname "$0")" && pwd)"          # clients/linux/flatpak
ROOT="$(cd "$HERE/../../.." && pwd)"           # repo root (worktree)
# shellcheck source=scripts/dev/lib.sh
. "$ROOT/scripts/dev/lib.sh"   # brand_icon_source + imagemagick_bin
SRC="${1:-$(brand_icon_source)}"
OUT="$HERE/icons"

IM="$(imagemagick_bin || true)"
if [[ -z "$IM" ]]; then
  echo "generate-icons: need ImageMagick (sudo apt install imagemagick)" >&2
  exit 1
fi
im() { "$IM" "$@"; }

if [[ ! -f "$SRC" ]]; then
  echo "generate-icons: source not found: $SRC" >&2
  exit 1
fi

mkdir -p "$OUT"
for px in 128 256 512; do
  # `-strip` drops the colour profile and EXIF the source carries; a launcher icon needs neither,
  # and they are a third of the file at 128px. Flat art with smooth gradients, so no dithering.
  im "$SRC" -resize "${px}x${px}" -strip -dither None -colors 256 "$OUT/${px}.png"
done

echo "==> Wrote hicolor icons (128/256/512) to $OUT"
