#!/usr/bin/env bash
# Re-cut every client's launcher icon from the one brand source.
#
#   scripts/dev/brand-icons.sh [path/to/source.png]
#
# Which source, when none is given, is `brand_icon_source`: Allodia's if `branding/allodia-icon.png`
# is beside this checkout, the neutral one otherwise. Swapping the art is therefore removing (or
# adding) that one file and running this; the art half of what `branding/allodia.env` does for the
# name (docs/branding.md).
#
# Outputs are committed, so this is run by hand and the diff is reviewed. It runs every generator
# THIS host can and names the ones it could not, because a rebrand that silently skipped a platform
# ships that platform's old art; which is the one failure this whole mechanism exists to prevent.
#
# No host can run all four: Apple's needs `sips` and Windows' needs System.Drawing, so a full
# re-cut is a Mac pass plus a Windows pass, and neither alone leaves the tree consistent. Each
# generator is therefore gated on the tool it needs rather than attempted; an ungated one aborts
# the script at the first host that lacks it, before the report below can name anything.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
# shellcheck source=scripts/dev/lib.sh
. "$ROOT/scripts/dev/lib.sh"

SRC="${1:-$(brand_icon_source)}"
[ -f "$SRC" ] || { echo "brand-icons: source not found: $SRC" >&2; exit 1; }

echo "==> Source icon: $SRC"
skipped=()

run() { # <label> <script> [args…]
  echo
  echo "--- $1"
  bash "${@:2}" "$SRC"
}

if command -v sips >/dev/null 2>&1; then
  run "Apple (macOS + iOS app icon set)" "$ROOT/clients/apple/Scripts/generate-appicon.sh"
else
  skipped+=("Apple: generate-appicon.sh cuts with sips, which is macOS-only")
fi

if imagemagick_bin >/dev/null; then
  run "Android (adaptive icon, mipmaps, Play listing)" "$ROOT/clients/android/generate-icons.sh"
  run "Linux (hicolor 128/256/512)" "$ROOT/clients/linux/flatpak/generate-icons.sh"
else
  skipped+=("Android: generate-icons.sh needs ImageMagick")
  skipped+=("Linux: generate-icons.sh needs ImageMagick")
fi

# System.Drawing, so the Windows tiles and app.ico can only be cut on Windows.
if is_windows; then
  pwsh="$(pwsh_bin)"
  if [ -n "$pwsh" ]; then
    echo
    echo "--- Windows (MSIX tiles + app.ico)"
    # Both paths through `to_win_path`: PowerShell reads `/d/repos/…` as a path from the current
    # drive's root, so an MSYS path reaches it as a file that does not exist.
    "$pwsh" -NoProfile -File \
      "$(to_win_path "$ROOT/clients/windows/Mailcal/Images/generate-assets.ps1")" \
      -Source "$(to_win_path "$SRC")"
  else
    skipped+=("Windows: no pwsh or powershell on PATH")
  fi
else
    skipped+=("Windows: generate-assets.ps1 needs System.Drawing, which is Windows-only")
fi

echo
if [ ${#skipped[@]} -gt 0 ]; then
  echo "==> NOT regenerated on this host, and still carrying whatever art was committed:"
  printf '      %s\n' "${skipped[@]}"
  exit 2
fi
echo "==> Every client's icon re-cut from $SRC"
