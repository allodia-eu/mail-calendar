#!/usr/bin/env bash
# Boot a client on a platform against the local Stalwart harness (the default), then hand off to
# the platform's existing build-and-run script; which honours the MAILCAL_DEV_ACCOUNT env this
# sets. This is the dev-account-aware entry point; it adds host-OS gating and (for stalwart) an
# up-front harness check on top of the plain build-and-run scripts.
#
#   scripts/dev/boot.sh <platform> [--account stalwart|stalwart-multi|stalwart-imap|personal|demo] [-- <extra build-run args>]
#
#   platform : macos | iphone | ipad | android | windows | linux
#   --account  stalwart (default); the local seeded harness over JMAP (brought up separately; run
#                                    scripts/dev/harness.sh up first)
#              stalwart-multi    ; the same harness over JMAP as TWO accounts (alice + bob), which
#                                    is what proves contact dedup ACROSS accounts; one account
#                                    cannot show it
#              stalwart-imap     ; the same harness over IMAP (full mail actions + IDLE push)
#              personal          ; the developer's stored accounts (today's behaviour)
#              demo              ; the in-memory demo provider (Apple)
#              first-run         ; an EMPTY namespace of its own: no accounts, no consent answered,
#                                    so the app opens on the screens a person sees once
#                                    (Apple and Windows)
#
# Anything after `--` passes through to the client build-and-run script (e.g. --simulator "iPhone
# 16", --no-core). The Android script takes no flags.
set -euo pipefail
source "$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)/lib.sh"

[[ $# -ge 1 ]] || die "usage: boot.sh <macos|iphone|ipad|android|windows|linux> [--account stalwart|stalwart-multi|stalwart-imap|personal|demo] [-- <extra args>]"
platform_raw="$1"; shift

ACCOUNT="stalwart"
PASSTHRU=()
while [[ $# -gt 0 ]]; do
  case "$1" in
    --account) ACCOUNT="${2:?missing value for --account}"; shift 2 ;;
    --) shift; while [[ $# -gt 0 ]]; do PASSTHRU+=("$1"); shift; done ;;
    *) PASSTHRU+=("$1"); shift ;;
  esac
done

case "$ACCOUNT" in
  stalwart|stalwart-multi|stalwart-imap|personal|demo|first-run) ;;
  *) die "unknown --account '$ACCOUNT' (stalwart|stalwart-multi|stalwart-imap|personal|demo|first-run)" ;;
esac

platform="$(normalize_platform "$platform_raw")"

# Export the switch the build-and-run scripts read. `personal` leaves it unset so the client uses
# its stored accounts unchanged.
if [[ "$ACCOUNT" != "personal" ]]; then
  export MAILCAL_DEV_ACCOUNT="$ACCOUNT"
fi

case "$ACCOUNT" in
  stalwart)
    require_harness
    info "booting $platform against the local Stalwart harness over JMAP (alice@test.local)" ;;
  stalwart-multi)
    require_harness
    info "booting $platform against the harness over JMAP as TWO accounts (alice + bob@test.local)" ;;
  stalwart-imap)
    require_harness
    # IMAP fidelity (full mail actions + IDLE) needs the dev-harness custom-root path to trust the
    # harness's self-signed cert. Apple and Windows debug cores get it via debug_assertions; the
    # Android dev build gets it via the `dev-harness` Cargo feature. How the cert reaches the app
    # differs per platform.
    [[ -f "$HARNESS_CA" ]] || extract_harness_ca || die "no harness IMAP cert at $HARNESS_CA: run: scripts/dev/harness.sh up"
    case "$platform" in
      macos|iphone|ipad|linux)
        # The app (macOS) / sim reads the cert file directly via MAILCAL_EXTRA_CA (a host path).
        export MAILCAL_EXTRA_CA="$HARNESS_CA" ;;
      android)
        # The emulator can't read a host path, so pass the cert base64 as an intent extra; the app
        # writes it into its sandbox and points the core at it (see clients/android/build-and-run.sh).
        # An already-exported value wins: that is how the client is pointed at something standing in
        # front of the harness with its own certificate (`imap-fault-proxy.py`).
        require_cmd base64
        if [[ -n "${MAILCAL_EXTRA_CA_PEM:-}" ]]; then
          info "using the MAILCAL_EXTRA_CA_PEM already in the environment, not the harness's own cert"
        else
          export MAILCAL_EXTRA_CA_PEM="$(base64 < "$HARNESS_CA" | tr -d '\n')"
        fi ;;
      windows)
        # Also a host path; but a *Windows* one: the core opens it with the Win32 file APIs, which
        # don't understand the MSYS `/d/repos/...` form this bash sees. The variable rides the
        # bash -> pwsh -> Start-Process hop by plain inheritance (MSYS passes unrecognised env vars
        # through verbatim; Start-Process hands the parent's environment to the child), and
        # build-and-run.ps1 asserts the file is readable before it launches.
        export MAILCAL_EXTRA_CA="$(to_win_path "$HARNESS_CA")" ;;
    esac
    info "booting $platform against the local Stalwart harness over IMAP (full mail actions + IDLE)" ;;
  personal) info "booting $platform against your stored (personal) accounts" ;;
  demo)     info "booting $platform in demo mode (in-memory sample mailbox)" ;;
  first-run)
    # Nothing is injected and nothing is read: the namespace starts empty and stays empty unless
    # an account is added inside it. Delete it to start over; the path is printed below.
    case "$platform" in
      macos|iphone|ipad|windows) ;;
      *) die "--account first-run is not supported on $platform yet" ;;
    esac
    info "booting $platform on an EMPTY namespace: the welcome screen, then the first-account screen"
    if [[ "$platform" == "windows" ]]; then
      info 'its store: %LOCALAPPDATA%\Allodia\MailCalendar\dev-first-run (delete it to see the first run again)'
    else
      info "its store: ~/.local/share/mailcal-dev-first-run (delete it to see the first run again)"
    fi ;;
esac

case "$platform" in
  macos)  exec "$REPO_ROOT/clients/apple/Scripts/build-and-run.sh" --macos ${PASSTHRU[@]+"${PASSTHRU[@]}"} ;;
  # `--simulator` pins these to a simulator even when an iPhone is plugged in (build-and-run.sh
  # would prefer the device): the harness this script boots against is loopback-only, so nothing on
  # a physical device can reach it; that path is scripts/dev/device.sh's. It comes BEFORE the
  # passthrough, so `boot.sh iphone -- --device` still overrides it.
  iphone) exec "$REPO_ROOT/clients/apple/Scripts/build-and-run.sh" --iphone --simulator ${PASSTHRU[@]+"${PASSTHRU[@]}"} ;;
  ipad)   exec "$REPO_ROOT/clients/apple/Scripts/build-and-run.sh" --ipad --simulator ${PASSTHRU[@]+"${PASSTHRU[@]}"} ;;
  android)
    [[ ${#PASSTHRU[@]} -eq 0 ]] || warn "ignoring extra args for android (build-and-run.sh takes none): ${PASSTHRU[*]}"
    if [[ "$ACCOUNT" == "demo" ]]; then
      warn "the Android client has no demo provider; it will fall back to your stored accounts"
    fi
    exec "$REPO_ROOT/clients/android/build-and-run.sh" ;;
  linux)
    [[ ${#PASSTHRU[@]} -eq 0 ]] || warn "ignoring extra args for linux (build-and-run.sh takes none): ${PASSTHRU[*]}"
    exec "$REPO_ROOT/clients/linux/build-and-run.sh"
    ;;
  windows)
    # The WinUI client is a PowerShell build; drive it through pwsh on the Windows host (this case
    # is only reachable there; normalize_platform gates it). MAILCAL_DEV_ACCOUNT is already
    # exported above for a non-personal account, and the launched app reads it. Extra args after
    # `--` pass through to build-and-run.ps1 (e.g. -Arch x64, -Configuration Release, -NoRun).
    ps="$(pwsh_bin)"; [[ -n "$ps" ]] || die "no PowerShell (pwsh/powershell) found to build the Windows client"
    script="$(to_win_path "$REPO_ROOT/clients/windows/build-and-run.ps1")"
    exec "$ps" -NoProfile -ExecutionPolicy Bypass -File "$script" ${PASSTHRU[@]+"${PASSTHRU[@]}"} ;;
esac
