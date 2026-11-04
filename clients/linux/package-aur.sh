#!/usr/bin/env bash
# Write the Linux client's Arch User Repository package: the PKGBUILD and .SRCINFO a publisher
# pushes to the AUR, filled from the brand and /VERSION. The sibling of package.sh, which builds
# the Flatpak, and package-snap.sh; all three generate the desktop entry and the AppStream file
# from the same listing.
#
#   clients/linux/package-aur.sh                  # target/aur/<name>/ for the tag v<VERSION>
#   clients/linux/package-aur.sh --local          # the same recipe, over this working tree
#   clients/linux/package-aur.sh --build          # ...and build it with makepkg
#   clients/linux/package-aur.sh --local --build -- --syncdeps   # the rest goes to makepkg
#
# The release recipe names the tag's source archive on the repository `origin` points at, and
# downloads it once to record its checksum, so the tag must have been pushed. `--source-url` names
# another repository. `--local` packs the tree git does not ignore instead, untracked files
# included, under the archive's own name, so it builds what a tag of this tree would.
#
# The AUR builds on the user's machine, where no OAuth registration exists, so the package never
# carries one: this script reads no `.env` and sets no MAILCAL_REQUIRE_INJECTED_CONFIG.
set -euo pipefail

HERE="$(cd "$(dirname "$0")" && pwd)"          # clients/linux
ROOT="$(cd "$HERE/../.." && pwd)"              # repo root (worktree)
# shellcheck source=scripts/dev/lib.sh
. "$ROOT/scripts/dev/lib.sh"                   # brand_load, die, info

LOCAL=0
BUILD=0
SOURCE_URL=""
MAKEPKG_ARGS=()
while [[ $# -gt 0 ]]; do
  case "$1" in
    --local) LOCAL=1 ;;
    --build) BUILD=1 ;;
    --source-url) SOURCE_URL="${2:?--source-url needs a repository URL}"; shift ;;
    --) shift; MAKEPKG_ARGS=("$@"); break ;;
    -h | --help) sed -n '2,18p' "$0"; exit 0 ;;
    *) die "package-aur.sh: unknown option '$1'" ;;
  esac
  shift
done

# The repository's https address, from `origin` unless named: the archive GitHub serves for a tag
# is `<repository>/archive/refs/tags/<tag>.tar.gz`, unpacking into `<name>-<version>/`.
if [[ -z "$SOURCE_URL" ]]; then
  SOURCE_URL="$(git -C "$ROOT" remote get-url origin)"
fi
SOURCE_URL="$(sed -E -e 's#^git@([^:]+):#https://\1/#' -e 's#\.git$##' -e 's#/$##' <<< "$SOURCE_URL")"
[[ "$SOURCE_URL" == https://* ]] || die "the source repository is not an https URL: $SOURCE_URL"
REPO_NAME="${SOURCE_URL##*/}"

PKGNAME="$MAILCAL_AUR_NAME"
[[ "$PKGNAME" =~ ^[a-z0-9@_+][a-z0-9@._+-]*$ ]] ||
  die "MAILCAL_AUR_NAME '$PKGNAME' is not a package name the AUR accepts"
VERSION="$(tr -d '[:space:]' < "$ROOT/VERSION")"
OUT="$ROOT/target/aur/$PKGNAME"
ARCHIVE="$PKGNAME-$VERSION.tar.gz"

metadata() { # <python expression over flatpak_metadata as m and brand>
  (cd "$ROOT" && python3 -c "
import sys
sys.path[:0] = ['scripts/dev', 'scripts/ci']
import brand
import flatpak_metadata as m
print($1)
")
}
# English only: pacman has no per-locale description.
PKGDESC="$(metadata 'm.summaries(brand.listing_source().read_text(encoding="utf-8"))["en"]')"
# Never the licensed core: that takes the Allodia registration, which no AUR build has.
LICENSE="$(metadata 'm.project_license(False)')"
[[ "$LICENSE" != *LicenseRef-* ]] || die "an AUR build cannot carry $LICENSE"

rm -rf "$OUT"
mkdir -p "$OUT"

# The brand's own files: whatever git ignores under branding/ (docs/branding.md).
BRAND_FILES=()
while IFS= read -r -d '' file; do
  cp "$ROOT/$file" "$OUT/"
  BRAND_FILES+=("${file##*/}")
done < <(git -C "$ROOT" ls-files -z --others --ignored --exclude-standard -- branding/)

if [[ "$LOCAL" == 1 ]]; then
  info "packing this working tree as $ARCHIVE"
  git -C "$ROOT" ls-files -z --cached --others --exclude-standard |
    tar -C "$ROOT" --null --files-from=- --ignore-failed-read \
      --transform "s#^#$REPO_NAME-$VERSION/#" -czf "$OUT/$ARCHIVE"
  SOURCES=("\"$ARCHIVE\"")
else
  url="$SOURCE_URL/archive/refs/tags/v$VERSION.tar.gz"
  info "downloading $url to record its checksum"
  curl -fsSL --retry 3 -o "$OUT/$ARCHIVE" "$url" ||
    die "could not download $url: has v$VERSION been pushed to $SOURCE_URL?"
  SOURCES=("\"\$pkgname-\$pkgver.tar.gz::$SOURCE_URL/archive/refs/tags/v\$pkgver.tar.gz\"")
fi
SUMS=("'$(sha256sum "$OUT/$ARCHIVE" | cut -d' ' -f1)'")
for file in "${BRAND_FILES[@]}"; do
  SOURCES+=("\"$file\"")
  SUMS+=("'$(sha256sum "$OUT/$file" | cut -d' ' -f1)'")
done

# One quoted word per line, as a PKGBUILD lists them.
list() { local first=1 item; for item in "$@"; do
  if [[ "$first" == 1 ]]; then printf '%s' "$item"; first=0; else printf '\n  %s' "$item"; fi
done; }
quoted_brand_files=""
for file in "${BRAND_FILES[@]}"; do quoted_brand_files+=" \"$file\""; done

# scripts/dev/aur_pkgbuild.py takes every value from the environment and quotes the free text.
AUR_PUBLISHER_NAME="$MAILCAL_PUBLISHER_NAME" \
AUR_REPO_URL="$SOURCE_URL" \
AUR_PKGNAME="$PKGNAME" \
AUR_VERSION="$VERSION" \
AUR_PKGDESC="$PKGDESC" \
AUR_URL="$MAILCAL_WEBSITE_URL" \
AUR_LICENSE="$LICENSE" \
AUR_SOURCES="$(list "${SOURCES[@]}")" \
AUR_SHA256SUMS="$(list "${SUMS[@]}")" \
AUR_SOURCE_DIR="\"$REPO_NAME-\$pkgver\"" \
AUR_BRAND_FILES="${quoted_brand_files# }" \
AUR_APP_ID="$MAILCAL_APP_ID" \
AUR_MCP_RELAY_NAME="$MAILCAL_MCP_RELAY_NAME" \
  python3 "$ROOT/scripts/dev/aur_pkgbuild.py" "$HERE/aur/PKGBUILD.in" "$OUT/PKGBUILD" ||
  die "the PKGBUILD could not be written"
bash -n "$OUT/PKGBUILD" || die "the PKGBUILD written to $OUT is not valid bash"
info "wrote $OUT/PKGBUILD ($PKGNAME $VERSION, $LICENSE)"

if command -v makepkg >/dev/null; then
  (cd "$OUT" && makepkg --printsrcinfo > .SRCINFO)
  info "wrote $OUT/.SRCINFO"
else
  warn "no makepkg on this host, so no .SRCINFO: the AUR refuses a push without one"
fi

if [[ "$BUILD" == 1 ]]; then
  command -v makepkg >/dev/null || die "--build needs makepkg, which is Arch's"
  (cd "$OUT" && makepkg --force --cleanbuild "${MAKEPKG_ARGS[@]}")
  if command -v namcap >/dev/null; then
    info "namcap"
    (cd "$OUT" && namcap PKGBUILD ./*.pkg.tar.* || true)
  fi
fi
