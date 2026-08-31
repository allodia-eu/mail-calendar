#!/usr/bin/env bash
# Shared helpers for the scripts/dev/* debugging tooling: repo/host detection, the per-platform
# app identifiers and log paths, simulator resolution, and harness readiness. Sourced by the
# other scripts; not run directly. Keep this the single place that knows a bundle id, a package
# name, or a log path, so the boot / logs / screenshot / control scripts stay in agreement.

# Resolve the repo root from this file's location (scripts/dev/lib.sh -> repo root), so the
# scripts work regardless of the caller's working directory.
DEV_LIB_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$DEV_LIB_DIR/../.." && pwd)"
export REPO_ROOT

# Per-platform app identifiers, taken from the same brand files the clients' own builds read
# (docs/branding.md) rather than repeated here; an unbranded build installs under a different id,
# and a script holding the branded one would attach to nothing and report the app as crashed.
# shellcheck source=scripts/dev/brand.sh
. "$DEV_LIB_DIR/brand.sh"
brand_load

APPLE_BUNDLE_ID="$MAILCAL_APP_ID"
# The macOS window's owner name, for window-only capture: CFBundleDisplayName, which is the brand
# name; not the target or the executable name, which stay `AllodiaMail`.
APPLE_APP_NAME="$MAILCAL_APP_NAME"
ANDROID_PKG="$MAILCAL_APP_ID"
ANDROID_ACTIVITY="$ANDROID_PKG/.MainActivity"

# The adb binary: a PATH adb if present, else the SDK's platform-tools (where the Android
# build-and-run.sh finds it). Prints the resolved path, or empty if neither exists.
adb_bin() {
  if command -v adb >/dev/null 2>&1; then command -v adb; return; fi
  # Probe the standard SDK locations for each host: macOS/Linux `adb`, Windows `adb.exe` (Git Bash).
  local sdk
  for sdk in \
    "${ANDROID_HOME:-}/platform-tools/adb" \
    "${ANDROID_HOME:-}/platform-tools/adb.exe" \
    "$HOME/Library/Android/sdk/platform-tools/adb" \
    "${LOCALAPPDATA:-}/Android/Sdk/platform-tools/adb.exe"; do
    [[ -n "$sdk" && -x "$sdk" ]] && { printf '%s\n' "$sdk"; return; }
  done
}

# The harness compose project (loopback-only; see docker/stalwart/README.md).
#
# The ports are the 12xxx/28080 block, deliberately NOT the engine repo's 11xxx/18080: both repos
# ship a docker/stalwart/docker-compose.yml, and until they were separated an `up` in one adopted
# the other's container and volumes and re-seeded it. The reasoning lives in that compose file's
# header; the numbers here have to agree with it.
STALWART_DIR="$REPO_ROOT/docker/stalwart"
STALWART_HTTP_ADDR="127.0.0.1:28080"
STALWART_IMAP_ADDR="127.0.0.1:12993"      # implicit-TLS IMAP (harness.sh deliver appends here)
STALWART_ALICE_PW="harness-alice-pw"      # the seeded alice@test.local password (a fixture, not a secret)
# The extracted harness IMAP cert (self-signed, SAN=localhost) a dev build trusts via
# MAILCAL_EXTRA_CA for the `stalwart-imap` mode. Regenerated each up/reset (it changes when the
# volumes are wiped), so it is gitignored, not committed.
HARNESS_CA="$STALWART_DIR/tls/harness-ca.pem"

# ---- logging ----------------------------------------------------------------------------------

# A red-ish error to stderr and exit non-zero.
die() { printf 'error: %s\n' "$*" >&2; exit 1; }
info() { printf '==> %s\n' "$*"; }
warn() { printf 'warning: %s\n' "$*" >&2; }

require_cmd() { command -v "$1" >/dev/null 2>&1 || die "'$1' is required but not found in PATH"; }

# ---- the core's cargo features -----------------------------------------------------------------

# The cargo features a client build must ask the core for, as a space-separated list, or nothing.
#
# There is one, and it is the Allodia sign-in. The code it turns on is source-available rather than
# GPL and the open tree must build without it (docs/pledge.md, promise 4), so it is an optional
# off-by-default dependency that an Allodia build asks for -- see BUILDING.md.
#
# It is derived from the registration rather than being a switch of its own, so the two halves
# cannot disagree: a build with the client id gets the code that uses it, and a build without one
# links nothing closed and offers no sign-in. Environment first, then the repo's gitignored `.env`,
# which is the order and the file the core's own build script reads. Blank counts as absent, the
# way a CI run without access to the secrets sets the empty string rather than leaving the name
# unbound.
core_cargo_features() {
  local value="${MAILCAL_ALLODIA_CLIENT_ID:-}"
  if [[ -z "$value" && -f "$REPO_ROOT/.env" ]]; then
    value="$(sed -n 's/^[[:space:]]*\(export[[:space:]][[:space:]]*\)\{0,1\}MAILCAL_ALLODIA_CLIENT_ID[[:space:]]*=[[:space:]]*//p' \
      "$REPO_ROOT/.env" | tail -n 1)"
    # One pair of surrounding quotes, which every other reader of this file tolerates too.
    value="${value%\"}"; value="${value#\"}"
    value="${value%\'}"; value="${value#\'}"
  fi
  [[ -n "$value" ]] && printf 'allodia-license'
  return 0
}

# The PowerShell executable for driving the Windows client from these bash wrappers on a Windows
# host: PowerShell 7 (`pwsh`) if present, else Windows PowerShell (`powershell`). Prints the
# resolved command, or nothing if neither exists. (Windows-only; the callers gate on the host.)
pwsh_bin() {
  local candidate
  for candidate in pwsh pwsh.exe powershell powershell.exe; do
    command -v "$candidate" >/dev/null 2>&1 && { command -v "$candidate"; return; }
  done
}

# Translate a path to Windows form (C:\...) for handing to PowerShell; a no-op passthrough if
# `cygpath` isn't available. Git Bash/MSYS ship cygpath, so this resolves on the Windows host.
to_win_path() { cygpath -w "$1" 2>/dev/null || printf '%s' "$1"; }

# ImageMagick's entry point, or nothing: `magick` (v7) if it is there, else `convert` (v6, which is
# what Ubuntu still ships); but only once `convert` has said it is ImageMagick.
#
# On Windows `convert` is System32's FAT-to-NTFS volume converter, which ships with the OS. It
# answers `-version` with "Invalid drive specification." and EXITS 0, so both `command -v convert`
# and an exit-code check call it ImageMagick; only its output tells them apart. Same shape as the
# `bash`-is-WSL trap in AGENTS.md, and it fails the same way; the caller hands a real image to a
# disk utility and reads the error as its own script misbehaving.
imagemagick_bin() {
  if command -v magick >/dev/null 2>&1; then echo magick; return 0; fi
  if command -v convert >/dev/null 2>&1 && convert -version 2>&1 | grep -qi imagemagick; then
    echo convert; return 0
  fi
  return 1
}

# ---- host detection ---------------------------------------------------------------------------

host_os() { uname -s; }
is_macos() { [[ "$(uname -s)" == "Darwin" ]]; }
is_linux() { [[ "$(uname -s)" == "Linux" ]]; }
# Git Bash / MSYS report MINGW*/MSYS* on Windows.
is_windows() { [[ "$(uname -s)" == MINGW* || "$(uname -s)" == MSYS* || "$(uname -s)" == CYGWIN* ]]; }

# ---- engine stores for the harness dev modes ---------------------------------------------------
#
# Each MAILCAL_DEV_ACCOUNT mode gets an isolated store dir so harness data never mixes with real
# accounts. THE mapping lives here because two scripts need it and they must not drift: store.sh
# reads one of them, and harness.sh clears them all after a `reset`.
#
# Why `reset` has to clear them: Stalwart mints its ids deterministically from an empty database,
# so a re-bootstrapped server hands out the SAME ids for a DIFFERENT set of messages. A client
# store synced against the previous generation then serves every cached body, attachment set and
# calendar part under the wrong message; silently, and looking entirely normal
# (docker/stalwart/README.md).
#
# Desktop only. A mobile client's store lives in an app container on a device or simulator, so
# harness.sh names those rather than reaching into them.
#   windows  clients/windows/Mailcal/Services/AppPaths.cs
#   macos    MailcalModel.connect
#   linux    clients/linux/src/boot.rs (a SUBDIR of the root, unlike macOS's sibling dirs)
DEV_STORE_MODES=(dev dev-multi dev-imap)

# The store dir for one dev mode on one desktop client. The platform is explicit rather than
# taken from the host, so `store.sh --platform macos` keeps naming the macOS path wherever it runs.
# `host_desktop_client` is what harness.sh passes, since it can only clear its own host's stores.
dev_store_dir() {
  local platform="$1" mode="$2"
  case "$platform" in
    windows) printf '%s/Allodia/MailCalendar/%s' "${LOCALAPPDATA:-$HOME/AppData/Local}" "$mode" ;;
    macos)   printf '%s/.local/share/mailcal-%s' "$HOME" "$mode" ;;
    linux)   printf '%s/mailcal/%s' "${XDG_DATA_HOME:-$HOME/.local/share}" "$mode" ;;
    *) return 1 ;;
  esac
}

# The desktop client this host runs, or nothing (a host with none; e.g. CI on a bare container).
host_desktop_client() {
  if is_windows; then printf 'windows'
  elif is_macos; then printf 'macos'
  elif is_linux; then printf 'linux'
  fi
}

# The macOS log file the app writes (docs/logging.md). macOS only.
macos_log_path() { printf '%s/.local/share/mailcal/mailcal.log\n' "$HOME"; }
# The XDG data log written by the native Linux client (docs/logging.md). Linux only.
linux_log_path() { printf '%s/mailcal/mailcal.log\n' "${XDG_DATA_HOME:-$HOME/.local/share}"; }


# ---- simulators (Apple) -----------------------------------------------------------------------

# Emit "<udid>\t<name>\t<state>" for every available simulator.
list_available_simulators() {
  xcrun simctl list devices available |
    sed -nE 's/^[[:space:]]*(.*) \(([0-9A-Fa-f-]{36})\) \((Booted|Shutdown)\).*$/\2	\1	\3/p'
}

# The udid of the available simulator with exactly this name (e.g. "iPhone 17 Pro Max"). Prints
# nothing and returns 1 when no such simulator exists.
sim_udid_by_name() { # <name>
  local uuid name state
  while IFS=$'\t' read -r uuid name state; do
    [[ "$name" == "$1" ]] || continue
    printf '%s\n' "$uuid"
    return 0
  done < <(list_available_simulators)
  return 1
}

# The CGWindowID of the macOS app's own window, for `screencapture -l` (window-only capture; a
# store screenshot must not show the rest of the desktop). Returns non-zero when there is no window
# to photograph; the helper's own exit codes say which case it is (2 none · 3 another Space · 4 it
# would not come to the front). Pass `--activate` to require a *key* window: an inactive one is
# captured perfectly happily and renders grey, which no later check can see.
macos_window_id() {
  xcrun swift "$DEV_LIB_DIR/macos-window-id.swift" "$APPLE_APP_NAME" "$@" 2>/dev/null
}

# The udid of the first BOOTED simulator, optionally filtered by family (iphone|ipad). Prints
# nothing and returns 1 if none is booted; the caller should boot one (via boot.sh) first.
booted_sim_udid() {
  local family="${1:-}"
  local uuid name state
  while IFS=$'\t' read -r uuid name state; do
    [[ "$state" == "Booted" ]] || continue
    case "$family" in
      iphone) [[ "$name" == *iPhone* ]] || continue ;;
      ipad) [[ "$name" == *iPad* ]] || continue ;;
    esac
    printf '%s\n' "$uuid"
    return 0
  done < <(list_available_simulators)
  return 1
}

# The data-container path of the installed app on simulator <udid> (holds its log under
# Library/Application Support/mailcal). Empty if the app isn't installed yet.
sim_app_container() {
  xcrun simctl get_app_container "$1" "$APPLE_BUNDLE_ID" data 2>/dev/null || true
}

# The booted simulator to drive with idb, with a live companion behind it. Prints its udid.
#
# `idb` never talks to a simulator directly; it talks to a per-target `idb_companion` process, and
# every call has to say WHICH one. Companions outlive the simulator that spawned them and stay
# registered in /tmp/idb/state, so any laptop that has debugged more than one simulator accumulates
# several, and an unqualified call then dies with "No udid provided and there are multiple
# companions to run against". That is the whole reason `control.sh iphone tap` never worked.
#
# `--udid` alone is not enough either: the socket file outlives the process, so a companion that has
# since exited leaves a stale socket that answers "Connection refused". `idb connect` starts one
# when none is running and is a cheap no-op (~0.8s) when one is, so calling it before every command
# repairs both cases and needs no state of our own.
#
# The udid can only come from `booted_sim_udid`, which reads `simctl`; so this resolves a
# SIMULATOR and nothing else. A physical iPhone never appears in that list and cannot be driven
# from here; real hardware is `scripts/dev/device.sh`'s job, which is deliberately explicit about
# touching it.
idb_sim_udid() { # <iphone|ipad>
  local udid
  udid="$(booted_sim_udid "$1")" ||
    die "no booted $1 simulator: boot one first: scripts/dev/boot.sh $1"
  idb connect "$udid" >/dev/null 2>&1 ||
    die "could not start an idb companion for $udid.
Install Meta's companion if it's missing: brew tap facebook/fb && brew install idb-companion"
  printf '%s\n' "$udid"
}

# ---- Android emulators --------------------------------------------------------------------------
#
# The counterpart of the simulator helpers above. A store screenshot set needs a *specific* device
#; Google Play has separate 7-inch and 10-inch tablet slots; and unlike `simctl`, adb has no
# concept of "the AVD named X": it only knows serials. These map one to the other, so a caller can
# ask for an AVD by name and get a serial back, booting it if it isn't already up.

# The SDK's emulator binary (it is not on PATH by default, unlike adb). Empty if absent.
emulator_bin() {
  if command -v emulator >/dev/null 2>&1; then command -v emulator; return; fi
  local sdk
  for sdk in \
    "${ANDROID_HOME:-}/emulator/emulator" \
    "${ANDROID_HOME:-}/emulator/emulator.exe" \
    "$HOME/Library/Android/sdk/emulator/emulator" \
    "${LOCALAPPDATA:-}/Android/Sdk/emulator/emulator.exe"; do
    [[ -n "$sdk" && -x "$sdk" ]] && { printf '%s\n' "$sdk"; return; }
  done
}

# Every AVD the SDK knows about, one per line.
list_avds() {
  local emu; emu="$(emulator_bin)"
  [[ -n "$emu" ]] || return 1
  "$emu" -list-avds 2>/dev/null | tr -d '\r' | sed '/^$/d'
}

avd_exists() { # <avd>
  local name
  while read -r name; do [[ "$name" == "$1" ]] && return 0; done < <(list_avds)
  return 1
}

# The adb serial of the *already running* emulator whose AVD is <avd>, or nothing. `emu avd name`
# is the only way to ask a serial which AVD it is; `ro.product.model` is a property of the system
# image, so two AVDs built on the same image (a 7-inch and a 10-inch tablet, say) report the same
# model and cannot be told apart by it.
emulator_serial_for_avd() { # <avd>
  local adb serial state name
  adb="$(adb_bin)"; [[ -n "$adb" ]] || return 1
  while read -r serial state; do
    [[ "$serial" == emulator-* && "$state" == "device" ]] || continue
    name="$("$adb" -s "$serial" emu avd name 2>/dev/null | tr -d '\r' | head -1)"
    [[ "$name" == "$1" ]] || continue
    printf '%s\n' "$serial"
    return 0
  done < <("$adb" devices 2>/dev/null | tr -d '\r' | tail -n +2)
  return 1
}

# Start <avd> on <port> and block until Android has finished booting; prints its adb serial.
#
# `-no-snapshot` is a cold boot both ways: a run never inherits a dirty AVD state (a half-open
# dialog, a stale install) and never writes one back, so the developer's AVD is left as it was.
# `wait-for-device` is not enough on its own; it returns as soon as adbd answers, which is a
# minute before there is a launcher to photograph, so poll `sys.boot_completed` too.
EMULATOR_BOOT_TIMEOUT=300
# How long to wait for a killed emulator to actually leave `adb devices`, freeing its console port.
EMULATOR_SHUTDOWN_TIMEOUT=60
emulator_boot() { # <avd> <port>
  local avd="$1" port="$2" emu adb serial waited=0
  emu="$(emulator_bin)" || true
  [[ -n "$emu" ]] || die "no Android emulator binary found: install the SDK's 'emulator' package (or set ANDROID_HOME)"
  adb="$(adb_bin)"
  serial="emulator-$port"
  # Refuse a port something else already holds. The new emulator would fail to bind and exit, while
  # `wait-for-device` below answers instantly for the squatter; and the run would then photograph
  # whichever device that is, silently.
  if "$adb" devices 2>/dev/null | tr -d '\r' | grep -q "^${serial}[[:space:]]"; then
    die "port $port is already taken by $serial (AVD '$("$adb" -s "$serial" emu avd name 2>/dev/null | tr -d '\r' | head -1)').
Stop it, or pass --serial to use it directly."
  fi
  "$emu" -avd "$avd" -port "$port" -no-snapshot -no-boot-anim >/dev/null 2>&1 &
  # Detach, so the caller's shell doesn't print a job notice when the emulator is later killed.
  disown
  "$adb" -s "$serial" wait-for-device
  until [[ "$("$adb" -s "$serial" shell getprop sys.boot_completed 2>/dev/null | tr -d '[:space:]')" == "1" ]]; do
    sleep 2
    waited=$((waited + 2))
    [[ "$waited" -lt "$EMULATOR_BOOT_TIMEOUT" ]] ||
      die "the '$avd' emulator did not finish booting within ${EMULATOR_BOOT_TIMEOUT}s"
  done
  printf '%s\n' "$serial"
}

emulator_shutdown() { # <serial>
  local adb; adb="$(adb_bin)"
  [[ -n "$adb" ]] || return 0
  "$adb" -s "$1" emu kill >/dev/null 2>&1 || true
  # `emu kill` returns when the emulator ACCEPTS the request, not when the process is gone and its
  # console port is free. Booting another AVD on that port straight afterwards; which is exactly
  # what capturing Play's two tablet slots in sequence does; then dies on "port already taken",
  # naming an emulator that is already on its way out. So wait for it to actually leave.
  #
  # A timeout returns rather than failing: this runs from a cleanup trap, where dying would replace
  # the run's real result with a teardown error. The next boot's own port check is what reports a
  # genuinely stuck emulator, and it says so precisely.
  local waited=0
  while "$adb" devices 2>/dev/null | tr -d '\r' | grep -q "^$1[[:space:]]"; do
    sleep 1
    waited=$((waited + 1))
    [[ "$waited" -lt "$EMULATOR_SHUTDOWN_TIMEOUT" ]] || return 0
  done
}

# ---- harness ----------------------------------------------------------------------------------

# True when the Stalwart harness container is running and healthy.
harness_healthy() {
  command -v docker >/dev/null 2>&1 || return 1
  local status
  status="$(cd "$STALWART_DIR" && docker compose ps --format '{{.Health}}' 2>/dev/null | head -1)"
  [[ "$status" == "healthy" ]]
}

# Fail with guidance unless the harness is up; every stalwart-account boot needs it.
require_harness() {
  harness_healthy || die "the Stalwart harness is not healthy: start it with: scripts/dev/harness.sh up"
}

# Extract the harness's served IMAP TLS cert (self-signed, SAN=localhost) to $HARNESS_CA, so a
# dev build can add it as a custom root via MAILCAL_EXTRA_CA (the `stalwart-imap` mode).
# Best-effort: warns and returns 1 on failure. The dial IP is irrelevant to the cert; server_name
# is `localhost`.
extract_harness_ca() {
  command -v openssl >/dev/null 2>&1 || { warn "openssl not found; cannot extract the harness IMAP cert"; return 1; }
  mkdir -p "$(dirname "$HARNESS_CA")"
  if echo | openssl s_client -connect 127.0.0.1:12993 2>/dev/null | openssl x509 >"$HARNESS_CA" 2>/dev/null \
    && grep -q "BEGIN CERTIFICATE" "$HARNESS_CA"; then
    return 0
  fi
  warn "could not extract the harness IMAP cert from 127.0.0.1:12993"
  return 1
}

# ---- platform validation ----------------------------------------------------------------------

# Normalise a platform argument and reject anything this host can't run. Prints the canonical
# platform (macos|iphone|ipad|android|windows|linux) on stdout.
normalize_platform() {
  local platform="$1"
  case "$platform" in
    ios) platform="iphone" ;;
    ipados) platform="ipad" ;;
    macos|iphone|ipad|android|windows|linux) ;;
    *) die "unknown platform '$platform' (expected: macos|iphone|ipad|android|windows|linux)" ;;
  esac
  case "$platform" in
    macos|iphone|ipad)
      is_macos || die "the $platform client needs macOS (Xcode + simulators); this host is $(host_os)" ;;
    linux)
      is_linux || die "the Linux client builds and runs only on Linux; this host is $(host_os)" ;;
    windows)
      is_windows || die "the Windows client builds and runs only on Windows; this host is $(host_os).
Run scripts\\dev on the Windows machine, or use clients/windows/build-and-run.ps1 there." ;;
    android)
      [[ -n "$(adb_bin)" ]] || die "adb not found: install the Android SDK platform-tools (or set ANDROID_HOME)" ;;
  esac
  printf '%s\n' "$platform"
}

# ---- the showcase (screenshot) interlock -------------------------------------------------------
#
# A screenshot cannot tell you it is showing fictional mail. The size floor in showcase.sh only
# rejects a *blank* frame; a capture of a real, fully-populated mailbox is a perfectly plausible
# 300 kB PNG, and one has been taken by accident (a binary built before showcase mode existed
# ignored MAILCAL_SHOWCASE and opened the developer's accounts, silently and photogenically).
#
# So every capture asserts the property positively before the shutter, using the one signal that is
# identical on all four platforms: `boot::build_showcase` (crates/mailcal-bindings/src/boot.rs)
# logs this line from *inside* the in-memory engine's constructor, and the shared `Logger` FFI port
# routes it into each client's diagnostic log (docs/logging.md). Its presence therefore proves the
# fictional engine was actually built; not merely that a flag was set somewhere.
#
# The marker is duplicated in three languages (Rust emits it; bash and PowerShell match it), so
# scripts/ci/check-showcase-flag.sh asserts all three copies still agree.
SHOWCASE_LOG_MARKER='showcase (screenshot) app starting (in-memory engine, seeded'

# The marker a run in <locale> must log. Rust's `{locale:?}` renders the ShowcaseLocale variant,
# so the seeded language appears with its first letter capitalized: En / Nl / De / Fr / Es / It /
# Pt. Derived from the code rather than listed arm by arm, so a new catalog locale needs no edit
# here; but still rejected unless it is one the showcase actually seeds.
showcase_marker_for() { # <locale>
  case "$1" in
    en | nl | de | fr | es | it | pt)
      printf '%s %s%s sample content)' \
        "$SHOWCASE_LOG_MARKER" "$(printf '%s' "${1:0:1}" | tr '[:lower:]' '[:upper:]')" "${1:1}"
      ;;
    *) return 1 ;;
  esac
}

# The bytes of <file> appended after <offset>; empty when the file doesn't exist yet. Reading only
# what this launch wrote is the point: a showcase line from an *earlier* run must never vouch for
# this one. A rotation (docs/logging.md caps the log and rolls it) shrinks the file below the
# offset, so fall back to the whole file rather than slicing at a stale position.
log_slice_since() { # <file> <offset>
  local file="$1" offset="$2" size
  [[ -f "$file" ]] || return 0
  size="$(wc -c <"$file" | tr -d '[:space:]')"
  if [[ "$size" -lt "$offset" ]]; then offset=0; fi
  tail -c "+$((offset + 1))" "$file"
}

# Whether <text> contains <marker>. Pure bash on purpose: `printf ... | grep -q` would let grep
# exit on the first match, SIGPIPE the writer, and trip `set -o pipefail` into reporting failure on
# the *successful* case; a guard that fails exactly when it should pass.
text_has_marker() { # <text> <marker>
  case "$1" in
    *"$2"*) return 0 ;;
    *) return 1 ;;
  esac
}

# ---- physical iOS devices (Apple) -------------------------------------------------------------
#
# The simulator helpers above cannot test background delivery: BGTaskScheduler never runs on a
# simulator and notification banners don't render there. These drive a real iPhone/iPad via
# `devicectl` (see the `device.sh` verb and the `ios-device-bgsync` skill). The on-device app data
# container also holds the same log/preferences files a simulator does.

# The on-device app-container-relative paths (docs/logging.md, docs/background-sync.md).
APP_LOG_REL="Library/Application Support/mailcal/mailcal.log"
APP_PREFS_REL="Library/Application Support/mailcal/preferences.toml"

# Emit "<udid>\t<name> (<os>)" for every CONNECTED physical iOS/iPadOS device.
#
# Two sections of `xctrace list devices` are deliberately dropped. "== Simulators ==" is obvious;
# "== Devices Offline ==" is not, and costs the auto-detection below its whole point; a device
# that was plugged in once is remembered there forever, so a laptop that has ever seen a second
# iPhone or iPad would report "multiple devices" from then on and refuse to pick either. An offline
# device can be neither built for nor installed to.
#
# Physical UDIDs are 8hex-16hex (modern) or 40 hex (older); the Mac host's own 8-4-4-4-12 UUID
# carries no OS parenthetical, so the shape excludes it.
list_connected_devices() {
  xcrun xctrace list devices 2>/dev/null |
    sed -nE '/^== Devices Offline ==/q; /^== Simulators ==/q;
             s/^(.*) \(([0-9.]+)\) \(([0-9A-Fa-f]{8}-[0-9A-Fa-f]{16}|[0-9A-Fa-f]{40})\)$/\3	\1 (\2)/p'
}

# The UDID of the physical iOS/iPadOS device to target. Honours $MAILCAL_DEVICE; else auto-detects
# the sole connected device. Returns 1 if none / many.
device_udid() {
  if [[ -n "${MAILCAL_DEVICE:-}" ]]; then printf '%s\n' "$MAILCAL_DEVICE"; return 0; fi
  local devices count
  devices="$(list_connected_devices)"
  count="$(printf '%s' "$devices" | grep -c . || true)"
  [[ "$count" -ge 1 ]] || return 1
  [[ "$count" -eq 1 ]] || { warn "multiple iOS devices connected; set MAILCAL_DEVICE=<udid>:
$devices"; return 1; }
  printf '%s\n' "${devices%%$'\t'*}"
}

# The display name of a connected device, empty if it is not connected (or is a simulator).
device_name() { # <udid>
  list_connected_devices | awk -F '\t' -v udid="$1" '$1 == udid { print $2; exit }'
}

# The signing team id for a device build: $DEVELOPMENT_TEAM, else the OU of the first "Apple
# Development" codesigning cert (a cert's OU IS its team id; the name's parenthetical is the cert
# id; a common wrong guess). Returns 1 if none derivable.
signing_team() {
  if [[ -n "${DEVELOPMENT_TEAM:-}" ]]; then printf '%s\n' "$DEVELOPMENT_TEAM"; return 0; fi
  local name ou
  name="$(security find-identity -v -p codesigning 2>/dev/null | awk -F'"' '/Apple Development/{print $2; exit}')"
  [[ -n "$name" ]] || return 1
  ou="$(security find-certificate -c "$name" -p 2>/dev/null | openssl x509 -noout -subject 2>/dev/null |
    sed -nE 's/.*OU ?= ?([0-9A-Za-z]+).*/\1/p')"
  [[ -n "$ou" ]] || return 1
  printf '%s\n' "$ou"
}

# The device's Developer Mode status (enabled|disabled|unknown).
device_dev_mode() { # <udid>
  xcrun devicectl device info details --device "$1" 2>&1 |
    sed -nE 's/.*developerModeStatus: *([a-z]+).*/\1/p' | head -1
}

# Fail with on-device guidance unless Developer Mode is enabled (required to install a dev build).
require_dev_mode() { # <udid>
  [[ "$(device_dev_mode "$1")" == "enabled" ]] || die "Developer Mode is not enabled on device $1.
On the device: Settings -> Privacy & Security -> Developer Mode -> On -> restart -> confirm.
(Manual, on-device: it cannot be toggled from the CLI.)"
}

# The running app's PID on device <udid>, or empty. Matches the executable path case-insensitively
# (the bundle is AllodiaMail.app/AllodiaMail, which a lowercase grep would miss). The `|| true`
# keeps a no-match (app not running) from tripping `set -o pipefail` in the caller.
device_app_pid() { # <udid>
  xcrun devicectl device info processes --device "$1" 2>/dev/null |
    { grep -i "AllodiaMail.app/AllodiaMail" || true; } | awk '{print $1}' | head -1
}

# Terminate the app on the device if running, so the next launch is a genuine fresh process (fresh
# onAppear, so a MAILCAL_* launch hook fires; a launch over a live process only re-foregrounds it).
device_terminate() { # <udid>
  local pid; pid="$(device_app_pid "$1")"
  if [[ -n "$pid" ]]; then
    xcrun devicectl device process terminate --device "$1" --pid "$pid" >/dev/null 2>&1 || true
    sleep 2
  fi
}

# Launch the app on the device, optionally with KEY=VAL environment (the MAILCAL_* launch hooks).
# Translates the common "device is locked" failure into an actionable message (iOS refuses to
# launch an app while the screen is locked).
device_launch() { # <udid> [KEY=VAL ...]
  local udid="$1"; shift
  local args=(--device "$udid")
  if [[ $# -gt 0 ]]; then
    local out="" pair
    for pair in "$@"; do out+="\"${pair%%=*}\":\"${pair#*=}\","; done
    args+=(--environment-variables "{${out%,}}")
  fi
  local err
  if ! err="$(xcrun devicectl device process launch "${args[@]}" "$APPLE_BUNDLE_ID" 2>&1)"; then
    if grep -qi "unlocked\|Locked" <<<"$err"; then
      die "the device is LOCKED: unlock the iPhone/iPad, then re-run (iOS won't launch an app while locked)."
    fi
    die "launch failed: $err"
  fi
}

# Copy a file out of the device app's data container to a local path. Returns 1 if absent.
device_pull() { # <udid> <remote-rel> <local>
  xcrun devicectl device copy from --device "$1" \
    --domain-type appDataContainer --domain-identifier "$APPLE_BUNDLE_ID" \
    --source "$2" --destination "$3" >/dev/null 2>&1
}

# The [notify_marks] body of a preferences.toml, flattened to one line (empty if none/absent).
# The background-sync high-water marks; the deterministic signal a pass ran and detected mail.
notify_marks_of() { # <preferences.toml path>
  [[ -f "$1" ]] || return 0
  awk '/^\[notify_marks\]/{f=1;next} /^\[/{f=0} f && NF{print}' "$1" | tr '\n' ' '
}
