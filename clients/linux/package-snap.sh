#!/usr/bin/env bash
# Build the Linux client as a snap, the package the Snap Store and Ubuntu's App Center install.
# The sibling of package.sh, which builds the Flatpak; both generate the desktop entry and the
# AppStream file from the same listing.
#
#   clients/linux/package-snap.sh            # build target/snap/<name>_<version>_<arch>.snap
#   clients/linux/package-snap.sh --install  # …and install it on this machine (asks for sudo)
#   clients/linux/package-snap.sh --destructive-mode  # build on this host, without LXD
#
# snapcraft builds inside an LXD container, set up once per machine:
#
#   sudo snap install snapcraft --classic --channel=latest/candidate && sudo snap install lxd
#   sudo lxd init --auto && sudo usermod -aG lxd "$USER"
#
# The container is kept between builds, with cargo's caches in it, so only the first build is cold.
# `--destructive-mode` builds on the host instead and installs the build packages into it, so it
# is for a throwaway host running the base's own release (Ubuntu 26.04 for core26), as CI's is.
# It runs snapcraft through sudo, because installing those packages needs root.
set -euo pipefail

HERE="$(cd "$(dirname "$0")" && pwd)"          # clients/linux
ROOT="$(cd "$HERE/../.." && pwd)"              # repo root (worktree)
# shellcheck source=scripts/dev/brand.sh
. "$ROOT/scripts/dev/brand.sh"
brand_load
# shellcheck source=scripts/dev/lib.sh
. "$ROOT/scripts/dev/lib.sh"

OUT="$ROOT/target/snap"
PROJECT="$OUT/project"

INSTALL=0
SNAPCRAFT_MODE=()
# What snapcraft, and the removal of what it writes, runs as. Only those: git refuses to read a
# checkout another user owns, so the script itself stays the invoking user's.
AS_BUILDER=()
while [[ $# -gt 0 ]]; do
  case "$1" in
    --install) INSTALL=1 ;;
    --destructive-mode) SNAPCRAFT_MODE=(--destructive-mode); AS_BUILDER=(sudo) ;;
    -h | --help) sed -n '2,18p' "$0"; exit 0 ;;
    *) echo "package-snap.sh: unknown option '$1'" >&2; exit 2 ;;
  esac
  shift
done

command -v snapcraft >/dev/null ||
  { echo "package-snap.sh: need snapcraft (sudo snap install snapcraft --classic)" >&2; exit 1; }
# core26's GNOME extension arrived in snapcraft 9.2; an older one fails with "Extension 'gnome'
# does not support base: 'core26'", which names neither the version nor the fix.
SNAPCRAFT_VERSION="$(snapcraft --version | awk '{print $2}')"
if ! printf '9.2\n%s\n' "$SNAPCRAFT_VERSION" | sort -VC; then
  echo "package-snap.sh: need snapcraft 9.2 or later, have $SNAPCRAFT_VERSION" \
    "(sudo snap refresh snapcraft --channel=latest/candidate)" >&2
  exit 1
fi

# The project is a copy of the tree git does not ignore, never the checkout itself: snapcraft
# hands its project directory to the build container whole, and `target/` runs to tens of GB.
# Untracked files are included, so a new source file is built before it is committed. tar keeps
# modification times, so cargo inside the container rebuilds only what changed.
echo "==> copying the source tree to $PROJECT"
"${AS_BUILDER[@]}" rm -rf "$PROJECT"
mkdir -p "$PROJECT"
git -C "$ROOT" ls-files -z --cached --others --exclude-standard |
  tar -C "$ROOT" --null --files-from=- --ignore-failed-read -cf - |
  tar -C "$PROJECT" -xf -
# The brand's own files are ignored (they belong to whoever publishes the app, docs/branding.md),
# and without them the build compiles and describes itself as the unbranded default.
cp -a "$ROOT/branding/." "$PROJECT/branding/"

# The registrations reach the build the way they reach the Flatpak's: as a `.env` in the copied
# tree, because snapcraft forwards no host environment into the container (package.sh explains why
# the file and not the environment). The copy is under target/ and removed on the way out, with
# the copies the part pulled from it; in LXD those are inside the container, owner-only, and gone
# at the next build's clean.
trap '"${AS_BUILDER[@]}" rm -f "$PROJECT/.env" "$PROJECT/parts/mailcal/src/.env" "$PROJECT/parts/mailcal/build/.env"' EXIT
if [[ -f "$ROOT/.env" ]]; then
  echo "==> credentials: using $ROOT/.env"
  # In LXD the container sees the file and no environment, so a variable exported for this one
  # build cannot win there. Say so rather than letting it be ignored.
  if (( ${#SNAPCRAFT_MODE[@]} == 0 )); then
    for var in "${PACKAGE_CREDENTIAL_VARS[@]}"; do
      if [[ -n "${!var:-}" ]]; then
        echo "    note: $var set in the environment is IGNORED: the container reads only the file." >&2
      fi
    done
  fi
  cp "$ROOT/.env" "$PROJECT/.env"
else
  ( umask 077; : > "$PROJECT/.env" )
  for var in "${PACKAGE_CREDENTIAL_VARS[@]}"; do
    if [[ -n "${!var:-}" ]]; then
      printf '%s=%s\n' "$var" "${!var}" >> "$PROJECT/.env"
    fi
  done
  [[ -s "$PROJECT/.env" ]] ||
    echo "==> credentials: none given: this snap will not offer Google or Microsoft sign-in"
fi

# The Allodia sign-in is a cargo feature, and the licence the snap declares follows from it
# (`project_license` in flatpak_metadata.py), so both are decided here from the one registration.
FEATURES="$(core_cargo_features)"
LICENSE="$(cd "$ROOT" && python3 -c '
import sys
sys.path[:0] = ["scripts/dev", "scripts/ci"]
import flatpak_metadata
print(flatpak_metadata.project_license())
')"
# snapd validates the field against its own SPDX list, which admits no `LicenseRef-` id, and names
# every licence outside that list `Proprietary`.
LICENSE="$(sed 's/LicenseRef-[A-Za-z0-9.-]*/Proprietary/g' <<< "$LICENSE")"

metainfo_url() { # <type>
  sed -n "s|.*<url type=\"$1\">\(.*\)</url>.*|\1|p" "$HERE/flatpak/metainfo.xml.in"
}

mkdir -p "$PROJECT/snap"
sed -e "s|@SNAP_NAME@|$MAILCAL_SNAP_NAME|g" \
    -e "s|@URL_HOMEPAGE@|$(metainfo_url homepage)|g" \
    -e "s|@URL_CONTACT@|$(metainfo_url contact)|g" \
    -e "s|@URL_BUGTRACKER@|$(metainfo_url bugtracker)|g" \
    -e "s|@APP_ID@|$MAILCAL_APP_ID|g" \
    -e "s|@LICENSE@|$LICENSE|g" \
    -e "s|@CARGO_FEATURES@|${FEATURES:+ --features $FEATURES}|g" \
    -e "s|@REQUIRE_INJECTED_CONFIG@|${MAILCAL_REQUIRE_INJECTED_CONFIG:-}|g" \
    "$HERE/snap/snapcraft.yaml.in" > "$PROJECT/snap/snapcraft.yaml"
if grep -n '@[A-Z_]*@' "$PROJECT/snap/snapcraft.yaml"; then
  echo "package-snap.sh: the fields above were left unfilled in snapcraft.yaml" >&2
  exit 1
fi
echo "==> building $MAILCAL_SNAP_NAME ($MAILCAL_APP_ID)${FEATURES:+ with --features $FEATURES}"

# snapcraft re-pulls a local source incrementally: it copies only files newer than its last pull
# and never removes one, so a file that arrives with an old timestamp (as the brand files above
# do) or a deleted one leaves the container's copy stale. Cleaning the part makes it pull the
# tree whole. Cargo's intermediates are not in the part: `.cargo/config.toml` puts them under
# CARGO_HOME, which survives the clean, so only what changed is compiled again.
# snapcraft writes only inside the project, so the snap is moved out of it afterwards.
(cd "$PROJECT" && "${AS_BUILDER[@]}" snapcraft clean "${SNAPCRAFT_MODE[@]}" mailcal &&
  "${AS_BUILDER[@]}" snapcraft pack "${SNAPCRAFT_MODE[@]}")

for built in "$PROJECT"/"$MAILCAL_SNAP_NAME"_*.snap; do
  mv -f "$built" "$OUT/"
  echo "==> wrote $OUT/$(basename "$built")"
done
SNAP_FILE="$(ls -t "$OUT"/"$MAILCAL_SNAP_NAME"_*.snap | head -n 1)"

if [[ "$INSTALL" == "1" ]]; then
  # `--dangerous`: a local build carries no store signature.
  sudo snap install --dangerous "$SNAP_FILE"
fi
