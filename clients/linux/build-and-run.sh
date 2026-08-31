#!/usr/bin/env bash
# Build and launch the GTK4/libadwaita client.
#
#   clients/linux/build-and-run.sh            # against the GNOME runtime the Flatpak ships on
#   clients/linux/build-and-run.sh --host     # against this distribution's GTK (faster loop)
#
# The default is the runtime, because that is what a user gets. The development baseline tracks the
# same GNOME generation, so the two are close; but they are separate builds on separate schedules
# and only the runtime's version is pinned, so it is the one worth validating against.
#
# `--host` is the fast inner loop: it reuses target/ and the incremental cache instead of the
# separate one an SDK build needs; and it is the right choice while iterating on logic that has
# nothing to do with the toolkit. Neither is a substitute for the other; each prints which it used.
#
# The Rust build script generates the typed localisation catalog into OUT_DIR, so the source tree
# remains free of generated files.
set -euo pipefail

HERE="$(cd "$(dirname "$0")" && pwd)"
ROOT="$(cd "$HERE/../.." && pwd)"
source "$ROOT/scripts/dev/lib.sh"
source "$ROOT/scripts/dev/sdk.sh"

TARGET=sdk
while [[ $# -gt 0 ]]; do
  case "$1" in
    --host) TARGET=host; shift ;;
    --sdk) TARGET=sdk; shift ;;
    -h|--help) sed -n '2,17p' "$0"; exit 0 ;;
    *) die "unknown argument '$1' (--host|--sdk)" ;;
  esac
done

require_cmd cargo

# The client compiles clients/composer/dist/editor.html straight into the binary with include_str!,
# and that bundle is committed rather than generated per build; so rebuild it from its TypeScript
# sources before cargo reads it.
bash "$ROOT/scripts/dev/composer-bundle.sh"

log="${XDG_DATA_HOME:-$HOME/.local/share}/mailcal/mailcal.log"

# The Allodia sign-in, when this build was given the registration that turns it on -- derived from
# that registration rather than asked for separately, so the two halves cannot disagree
# (scripts/dev/lib.sh, BUILDING.md). It resolves to nothing in a build from source, and the
# Settings card is then absent. The Flatpak build deliberately stays on default features; that gap
# is recorded in the entitlement contract that ships beside the Allodia Licence.
# Each name is package-qualified because the build asks cargo for two packages, and a bare feature
# name is ambiguous across them.
FEATURES="mailcal-linux/dev-harness"
ALLODIA_FEATURE="$(core_cargo_features)"
if [[ -n "$ALLODIA_FEATURE" ]]; then
  FEATURES="$FEATURES,mailcal-linux/$ALLODIA_FEATURE"
fi

if [[ "$TARGET" == sdk ]]; then
  if ! sdk_available; then
    die "the GNOME $(sdk_runtime_version) runtime is not installed. Either install it once,
       $(sdk_install_hint)
    : or build against this distribution's GTK with --host, knowing that is not what ships."
  fi
  info "building inside the GNOME $(sdk_runtime_version) SDK: $(sdk_versions)"
  sdk_cargo build -p mailcal-linux -p mailcal-mcp-shim --features "$FEATURES"
  info "Launching Allodia Mail & Calendar (GNOME $(sdk_runtime_version) runtime)"
  info "Logs: $log (rotates .1-.3, ~4 MB cap): read them: scripts/dev/logs.sh linux --dump"
  # The binary links the runtime's libraries, including a newer libc than the host's, so it starts
  # only inside the sandbox.
  exec_env=()
  for name in MAILCAL_DEV_ACCOUNT MAILCAL_EXTRA_CA MAILCAL_CALENDAR MAILCAL_CALENDAR_VIEW \
    MAILCAL_OPEN_SUBJECT MAILCAL_OPEN_FIRST MAILCAL_SHOWCASE MAILCAL_SHOWCASE_SCREEN \
    XDG_DATA_HOME XDG_CONFIG_HOME XDG_CACHE_HOME WAYLAND_DISPLAY DISPLAY; do
    [[ -n "${!name:-}" ]] && exec_env+=("--env=$name=${!name}")
  done
  # A shell function, so no `exec`: the launcher waits on flatpak instead of replacing itself.
  sdk_exec ${exec_env[@]+"${exec_env[@]}"} "$(sdk_target_dir)/debug/mailcal-linux"
  exit $?
fi

require_cmd pkg-config
# The crate's feature gates, which are the compile-time floor; deliberately older than both the
# baseline and the runtime, because they are what the code may call. Moving the baseline does not
# move these.
pkg-config --atleast-version=4.14 gtk4 ||
  die "GTK 4.14 or newer is required (pkg-config package: gtk4)"
pkg-config --atleast-version=1.5 libadwaita-1 ||
  die "libadwaita 1.5 or newer is required (pkg-config package: libadwaita-1)"
pkg-config --exists webkitgtk-6.0 ||
  die "WebKitGTK 6.0 is required (pkg-config package: webkitgtk-6.0)"

warn "building against this distribution's GTK $(pkg-config --modversion gtk4) / libadwaita $(pkg-config --modversion libadwaita-1),
     a different build from the GNOME $(sdk_runtime_version) runtime the Flatpak links, however close the versions read.
     Drop --host to run against the runtime."
info "Building the Linux client"
(cd "$ROOT" && cargo build -p mailcal-linux -p mailcal-mcp-shim --features "$FEATURES")
info "Launching Allodia Mail & Calendar (distribution GTK)"
info "Logs: $log (rotates .1-.3, ~4 MB cap): read them: scripts/dev/logs.sh linux --dump"
exec "$ROOT/target/debug/mailcal-linux"
