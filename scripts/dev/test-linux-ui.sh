#!/usr/bin/env bash
# Run the Linux client's acceptance suite on a private Wayland + D-Bus session, against the local
# Stalwart harness first and then the showcase for the states no fixture can stage. Each leg is a
# function in scripts/dev/linux-ui/ (`--list-legs` names them); `--leg` runs one with the legs it
# needs, and the whole run takes them in order. Every screenshot, tree and log is kept under
# target/ui-test-artifacts/linux, with each leg's time in leg-times.tsv.
#
# A leg finds, presses, types and waits through the app's test agent (docs/test-agent.md), over one
# connection for the whole run. It asks GTK's AT-SPI tree only where the assertion is what a screen
# reader gets: an accessible name, a role or a description. Each of those calls says so beside it.
# One click goes through a real pointer, the suite's only guard on pointer input, and a few keys
# are real keystrokes where the key itself is what is tested.
#
# It expects the deterministic seed, the one `harness.sh up` lays down. Each run leaves its own mail
# in Alice's Inbox (about ten messages: notifications, invitations, a bounce), so the list grows
# from one run to the next and a fixture's place in it is not fixed. The run reaches a seeded
# message by searching for it, and judges a list by the mail it delivered itself, which is always
# at the top; a check that waits for a fixture row by position would pass only on a fresh seed.
# Against a `--bulk` mailbox an AT-SPI walk of the whole remote tree takes about a second and a
# half over D-Bus, which the few AT-SPI assertions left still pay; reseed with `harness.sh reset`
# before reading a failure there as a regression.
#
#   scripts/dev/test-linux-ui.sh
#   scripts/dev/test-linux-ui.sh --start-harness
#   scripts/dev/test-linux-ui.sh --leg meeting          # with the legs it needs, and no others
#   scripts/dev/test-linux-ui.sh --list-legs
#   scripts/dev/test-linux-ui.sh --no-build --artifacts /tmp/mailcal-linux-ui
set -euo pipefail
source "$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)/lib.sh"
source "$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)/sdk.sh"
source "$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)/linux_session.sh"
for part in agent app fixtures session legs_start legs_mail legs_calendar legs_people legs_desktop; do
  source "$REPO_ROOT/scripts/dev/linux-ui/$part.sh"
done

SELF="$REPO_ROOT/scripts/dev/test-linux-ui.sh"
START_HARNESS=0
NO_BUILD=0
INSIDE_SESSION=0
ACTIVATED_APP=0
LIST_LEGS=0
ARTIFACT_DIR=""
CHOSEN_LEGS=()

while [[ $# -gt 0 ]]; do
  case "$1" in
    --start-harness) START_HARNESS=1; shift ;;
    --no-build) NO_BUILD=1; shift ;;
    --artifacts) ARTIFACT_DIR="${2:?--artifacts needs a directory}"; shift 2 ;;
    --leg) CHOSEN_LEGS+=("${2:?--leg needs a leg: see --list-legs}"); shift 2 ;;
    --list-legs) LIST_LEGS=1; shift ;;
    --inside-session) INSIDE_SESSION=1; shift ;;
    --activated-app) ACTIVATED_APP=1; shift ;;
    -h|--help)
      sed -n '2,27p' "$SELF"
      exit 0
      ;;
    *) die "unknown argument '$1' (--start-harness|--no-build|--artifacts|--leg|--list-legs)" ;;
  esac
done

if [[ "$LIST_LEGS" == 1 ]]; then
  list_legs
  exit 0
fi
for leg in "${CHOSEN_LEGS[@]}"; do
  leg_known "$leg" || die "no leg '$leg': see --list-legs"
done

if [[ "$INSIDE_SESSION" == 1 ]]; then
  run_inside_session "${CHOSEN_LEGS[@]}"
  exit 0
fi

# What the private bus runs when something calls the app's bus name while it is not running: the
# Exec of the service file `click_notification_cold` writes, standing in for the one the Flatpak
# installs. Waits on the client, because the bus counts the service as started for as long as this
# process holds the name it asked for.
if [[ "$ACTIVATED_APP" == 1 ]]; then
  session_paths
  launch_app "" "$ARTIFACT_DIR/notification-cold" --gapplication-service
  echo "$APP_PID" >"$ARTIFACT_DIR/notification-cold.pid"
  wait "$APP_PID"
  exit 0
fi

is_linux || die "the Linux UI acceptance test runs only on Linux"
require_cmd cargo
require_cmd curl
require_cmd dbus-daemon
require_cmd dbus-run-session
require_cmd gdbus
require_cmd sway
require_cmd grim
require_cmd pdftotext
[[ -x "$PYTHON" ]] || die "the distro /usr/bin/python3 is required"
atspi_registryd_bin >/dev/null ||
  die "at-spi2-registryd is required: no org.a11y.atspi.Registry.service names one that exists"
"$PYTHON" -c 'import pyatspi' 2>/dev/null ||
  die "python3-pyatspi is required (install the clients/linux/README.md prerequisites)"

if ! curl -fsS "http://$STALWART_HTTP_ADDR/healthz/live" >/dev/null; then
  if [[ "$START_HARNESS" == 1 ]]; then
    "$REPO_ROOT/scripts/dev/harness.sh" up
  else
    die "the Stalwart harness is not reachable: run scripts/dev/harness.sh up, or pass --start-harness"
  fi
fi

sdk_available || die "the GNOME $(sdk_runtime_version) runtime is required to verify the client
       against the toolkit it ships on: $(sdk_install_hint)"

if [[ "$NO_BUILD" == 0 ]]; then
  info "building the Linux dev-harness client inside the GNOME $(sdk_runtime_version) SDK"
  sdk_cargo build -p mailcal-linux -p mailcal-mcp-shim --features mailcal-linux/dev-harness
  # On the host, not in the SDK: it drives the compositor from outside the sandbox. Built here so
  # the pointer leg does not stop to compile it halfway through a run.
  info "building mailcal-vpointer (host)"
  (cd "$REPO_ROOT" && cargo build -q -p mailcal-vpointer)
fi
info "toolkit under test: $(sdk_versions)"

if [[ -z "$ARTIFACT_DIR" ]]; then
  ARTIFACT_DIR="$REPO_ROOT/target/ui-test-artifacts/linux/$(date '+%Y%m%d-%H%M%S')"
fi
mkdir -p "$ARTIFACT_DIR/xdg-data" "$ARTIFACT_DIR/xdg-cache" "$ARTIFACT_DIR/xdg-config"
ARTIFACT_DIR="$(cd "$ARTIFACT_DIR" && pwd)"
SESSION_RUNTIME="$(mktemp -d /tmp/mailcal-ui-runtime.XXXXXX)"
chmod 700 "$SESSION_RUNTIME"
# Keep the Unix socket's literal path representative of a real install. Pointing XDG_DATA_HOME at
# the long content-addressed artifact directory pushes `mailcal/mcp.sock` past `sun_path`'s 103-byte
# payload limit; the core correctly refuses that endpoint, but that tests an artificial path rather
# than the Flatpak. The symlink keeps every database/log under the artifact directory while the
# app and relay name the short private-runtime path.
ln -s "$ARTIFACT_DIR/xdg-data" "$SESSION_RUNTIME/data"
cleanup_runtime() {
  for _ in {1..20}; do
    if rm -rf -- "$SESSION_RUNTIME" 2>/dev/null; then
      return
    fi
    sleep 0.1
  done
  warn "could not remove private runtime directory $SESSION_RUNTIME (a portal mount may still be exiting)"
}
trap cleanup_runtime EXIT INT TERM

# A compositor of our own, not the developer's desktop. It gives the run three things GNOME will
# not: the client's own pixels (GNOME offers a script no per-window capture at all), a screen
# nobody is using, so a flow cannot be broken by the developer clicking elsewhere, and a fixed
# size, so a capture means the same thing on every machine. scripts/dev/linux_session.sh has the
# rest, including why grim sees popovers that a window capture would miss.
#
# It is started out here rather than inside the D-Bus session, so the trap below takes it away on
# every exit including an interrupt; sway itself wants no session bus.
#
# ⚠️ **The compositor's socket has to live in the run's own `XDG_RUNTIME_DIR`, so it is exported
# before sway starts rather than handed to the run below.** A Wayland socket is found as
# `$XDG_RUNTIME_DIR/$WAYLAND_DISPLAY`, and this run replaces `XDG_RUNTIME_DIR` with a private
# directory on purpose (see the `sun_path` note above). A compositor left in the real one would
# then be invisible to everything inside: grim would find no display, and flatpak, which resolves
# the same pair to decide what to bind into the sandbox, would give the client none either.
info "running the Linux UI acceptance suite on a private headless compositor"
export LINUX_SESSION_LOG="$ARTIFACT_DIR/compositor.log"
export XDG_RUNTIME_DIR="$SESSION_RUNTIME"
linux_session_start 1440x900 1
trap 'linux_session_stop; cleanup_runtime' EXIT INT TERM

leg_arguments=()
for leg in "${CHOSEN_LEGS[@]}"; do
  leg_arguments+=(--leg "$leg")
done
LIBGL_ALWAYS_SOFTWARE=1 \
  env -u DISPLAY \
    ARTIFACT_DIR="$ARTIFACT_DIR" \
    WAYLAND_DISPLAY="$LINUX_SESSION_DISPLAY" \
    LINUX_SESSION_DISPLAY="$LINUX_SESSION_DISPLAY" \
    LINUX_SESSION_SWAYSOCK="$LINUX_SESSION_SWAYSOCK" \
    LINUX_SESSION_PID="$LINUX_SESSION_PID" \
    GDK_DPI_SCALE=1 \
    GDK_SCALE=1 \
    GTK_A11Y=atspi \
    LANG=C.UTF-8 \
    LIBGL_ALWAYS_SOFTWARE=1 \
    MAILCAL_DEV_ACCOUNT=stalwart \
    MAILCAL_CALENDAR_VIEW=agenda \
    MAILCAL_FAKE_DEVICE_TIMEZONE=Pacific/Auckland \
    MAILCAL_SHOWCASE_SCREEN=signatures \
    RUN_STAMP="$(date '+%s%N')" \
    WEBKIT_DISABLE_DMABUF_RENDERER=1 \
    XDG_CACHE_HOME="$ARTIFACT_DIR/xdg-cache" \
    XDG_CONFIG_HOME="$ARTIFACT_DIR/xdg-config" \
    XDG_DATA_HOME="$SESSION_RUNTIME/data" \
    XDG_RUNTIME_DIR="$SESSION_RUNTIME" \
    dbus-run-session -- "$SELF" --inside-session --artifacts "$ARTIFACT_DIR" "${leg_arguments[@]}"

trap - EXIT INT TERM
linux_session_stop
cleanup_runtime
info "Linux UI acceptance passed"
info "artifacts: $ARTIFACT_DIR"
