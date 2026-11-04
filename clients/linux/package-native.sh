#!/usr/bin/env bash
# Build the Linux client for a native install: the release binaries and every file a distribution
# package installs beside them, staged as a /usr tree and packed into one tarball. The sibling of
# package.sh, which builds the Flatpak, and package-snap.sh; all three generate the desktop entry
# and the AppStream file from the same listing.
#
#   clients/linux/package-native.sh     # target/native/<name>-<version>-<arch>.tar.xz (+ .sha256)
#
# It links against this host's GTK, libadwaita and WebKitGTK, so the host decides where the binary
# runs: built on Ubuntu 26.04, as CI is, it runs there and on any distribution whose glibc and
# libraries are as new or newer, Arch among them, because each keeps its soname. Built on a newer
# distribution it would not run on an older one.
#
# A `.deb` or an AUR package of the tarball is the publisher's to make, in their own repository and
# under their own keys. The tarball names nothing about where it is published.
#
# The registrations reach the compile as they reach any host build, from the environment or the
# repository's `.env`, which the core's build script reads itself; MAILCAL_REQUIRE_INJECTED_CONFIG=1
# makes a build without them fail by name. The Allodia sign-in is compiled in when its registration
# is present (`core_cargo_features`), and the licence files follow it.
set -euo pipefail

HERE="$(cd "$(dirname "$0")" && pwd)"          # clients/linux
ROOT="$(cd "$HERE/../.." && pwd)"              # repo root (worktree)
# shellcheck source=scripts/dev/brand.sh
. "$ROOT/scripts/dev/brand.sh"
brand_load
# shellcheck source=scripts/dev/lib.sh
. "$ROOT/scripts/dev/lib.sh"                   # core_cargo_features, die, info

while [[ $# -gt 0 ]]; do
  case "$1" in
    -h | --help) sed -n '2,20p' "$0"; exit 0 ;;
    *) die "package-native.sh: unknown option '$1'" ;;
  esac
done

for tool in cargo python3 desktop-file-validate appstreamcli; do
  command -v "$tool" >/dev/null ||
    die "package-native.sh: need $tool (see the prerequisites in clients/linux/README.md)"
done

NAME="$MAILCAL_LINUX_PACKAGE_NAME"
# What apt and pacman both accept: Debian's rule, which is the narrower of the two.
[[ "$NAME" =~ ^[a-z0-9][a-z0-9+.-]+$ ]] ||
  die "MAILCAL_LINUX_PACKAGE_NAME '$NAME' is not a name both apt and pacman accept"
VERSION="$(tr -d '[:space:]' < "$ROOT/VERSION")"
ARCH="$(uname -m)"
OUT="$ROOT/target/native"
STAGE="$OUT/root"
TARBALL="$OUT/$NAME-$VERSION-$ARCH.tar.xz"
# Every earlier tarball too: one written under another name or version would be uploaded beside it.
rm -rf "$STAGE" "$OUT/metadata" "$OUT"/*.tar.xz "$OUT"/*.tar.xz.sha256
mkdir -p "$OUT"

# A build directory of its own. The shared one hands a checkout whatever another checkout built
# last when its sources are older (.cargo/config.toml), and a release artefact must be this tree's.
export CARGO_BUILD_BUILD_DIR="$OUT/build"

FEATURES="$(core_cargo_features)"
info "building $NAME $VERSION for $ARCH${FEATURES:+ with --features $FEATURES}"
cargo build --locked --release -p mailcal-linux ${FEATURES:+--features "$FEATURES"}
cargo build --locked --release -p mailcal-mcp-shim --bin mailcal-mcp
if [[ -n "$FEATURES" ]]; then
  info "credentials: this build carries the Allodia sign-in (--features $FEATURES)"
fi

python3 "$ROOT/scripts/dev/flatpak_metadata.py" --package native --out-dir "$OUT/metadata" >/dev/null
desktop-file-validate "$OUT/metadata/$MAILCAL_APP_ID.desktop"
appstreamcli validate --no-net --explain "$OUT/metadata/$MAILCAL_APP_ID.metainfo.xml"

# The metainfo derives its licence from the same registration, so the two cannot disagree unless
# one of them stopped reading it; a package would then claim a licence its binary is not under.
licence="$(sed -n 's:.*<project_license>\(.*\)</project_license>.*:\1:p' "$OUT/metadata/$MAILCAL_APP_ID.metainfo.xml")"
case "$FEATURES:$licence" in
  :*LicenseRef-*) die "the metainfo claims '$licence' for a build without the Allodia sign-in" ;;
  ?*:GPL-3.0-only) die "the metainfo claims only GPL-3.0-only for a build with the Allodia sign-in" ;;
esac

install -Dm755 "$ROOT/target/release/mailcal-linux" "$STAGE/usr/bin/$NAME"
install -Dm755 "$ROOT/target/release/mailcal-mcp" "$STAGE/usr/bin/$MAILCAL_MCP_RELAY_NAME"
install -Dm644 "$OUT/metadata/$MAILCAL_APP_ID.desktop" -t "$STAGE/usr/share/applications"
install -Dm644 "$OUT/metadata/$MAILCAL_APP_ID.metainfo.xml" -t "$STAGE/usr/share/metainfo"
install -Dm644 "$OUT/metadata/$MAILCAL_APP_ID.service" -t "$STAGE/usr/share/dbus-1/services"
for size in 128 256 512; do
  install -Dm644 "$HERE/flatpak/icons/$size.png" \
    "$STAGE/usr/share/icons/hicolor/${size}x${size}/apps/$MAILCAL_APP_ID.png"
done
install -Dm644 "$ROOT/LICENSE" "$STAGE/usr/share/licenses/$NAME/GPL-3.0-only.txt"
if [[ -n "$FEATURES" ]]; then
  install -Dm644 "$ROOT/LICENSES/LicenseRef-Allodia-1.0.txt" -t "$STAGE/usr/share/licenses/$NAME"
fi

missing="$(ldd "$STAGE/usr/bin/$NAME" | grep 'not found' || true)"
[[ -z "$missing" ]] || die "the binary needs libraries this host does not have:"$'\n'"$missing"

# No owner, no timestamps and a fixed order, so the packing adds nothing of its own to the bytes.
# The checksum still moves with every build: the binary records when it was built.
tar -C "$STAGE" --sort=name --owner=0 --group=0 --numeric-owner --mtime='@0' -cJf "$TARBALL" usr
(cd "$OUT" && sha256sum "${TARBALL##*/}" > "${TARBALL##*/}.sha256")
info "wrote $TARBALL ($licence)"
info "$(cat "$TARBALL.sha256")"
