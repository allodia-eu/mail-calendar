#!/usr/bin/env bash
# Find where a GLib diagnostic came from. GTK and libadwaita report a failed precondition as a
# **critical** and carry on, so the message names the assertion that failed and nothing about the
# code that reached it; and the widget almost always still renders, which is why no rendering
# assertion catches one.
#
#   scripts/dev/gtk-trace.sh test [<test-filter>]              # backtrace at the first critical
#   scripts/dev/gtk-trace.sh dbus [<test-filter>]              # every D-Bus call the suite makes
#   scripts/dev/gtk-trace.sh probe <substring> -- <command…>   # backtrace from the running app
#   scripts/dev/gtk-trace.sh squatters                         # who owns this app's bus names
#
# `test` is the fast one and answers most questions. Reach for `probe` when the critical appears
# only in the running app: the toolkit the app links is the **runtime's**, not the distribution's,
# and the two disagree about focus; so a critical the app raises need not reproduce in the host
# suite at all, and the host suite can raise the same message from a different place entirely.
set -euo pipefail
source "$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)/lib.sh"

CRATE=mailcal-linux

usage() { sed -n '2,15p' "${BASH_SOURCE[0]}"; exit "${1:-1}"; }

# The crate's test binary. `--no-run` prints the path rather than us guessing the hash cargo
# appends to it.
test_binary() {
  cargo test -p "$CRATE" --all-features --no-run 2>&1 |
    grep -oE "target/debug/deps/mailcal_linux-[a-z0-9]+" | head -1
}

# A display, because `gtk::init` needs one. The session bus stays the session's own: a private bus
# hides a squatter, which is one of the things this script exists to find.
in_session() {
  require_cmd xvfb-run
  xvfb-run --auto-servernum "$@"
}

mode_test() {
  require_cmd gdb
  local filter="${1:-}" binary
  binary="$(test_binary)"
  [[ -n "$binary" ]] || die "no test binary for $CRATE"
  info "gdb on $binary, stopping at the first GLib critical"
  info "if the message above the stack is not the one you are hunting, that critical came first"
  # `fatal-criticals` turns the first critical into an abort, which is the only way to still be
  # standing in the frame that raised it. Rust frames come out named because the dev profile keeps
  # line tables (root Cargo.toml).
  # `GTK_A11Y=none` because a headless display has no accessibility bus, and GTK says so with a
  # critical of its own; which `fatal-criticals` would otherwise stop at every single time,
  # before reaching yours.
  G_DEBUG=fatal-criticals GTK_A11Y=none RUST_BACKTRACE=1 in_session gdb --batch \
    -ex run -ex "bt 45" \
    --args "$REPO_ROOT/$binary" --test-threads=1 --nocapture ${filter:+"$filter"}
}

mode_dbus() {
  local filter="${1:-}"
  info "tracing every D-Bus call; the last one before a stall is the one that hung"
  # A call that fails after exactly 25 s has hit GIO's default timeout, which is a peer that never
  # answered; not a bus that is missing.
  G_DBUS_DEBUG=call in_session cargo test -p "$CRATE" --all-features ${filter:+-- "$filter"}
}

mode_probe() {
  local wanted="${1:-}"
  shift || true
  [[ "${1:-}" == "--" ]] && shift
  [[ -n "$wanted" && $# -gt 0 ]] || usage
  local target="$REPO_ROOT/clients/linux/src/crash.rs" backup
  backup="$(mktemp)"
  cp "$target" "$backup"
  # Restored however this exits, so an interrupted run never leaves the probe in the tree.
  trap 'cp "$backup" "$target"; rm -f "$backup"; info "probe removed"' EXIT
  python3 - "$target" "$wanted" <<'PY'
import sys

path, wanted = sys.argv[1], sys.argv[2]
source = open(path).read()
anchor = "        glib::log_default_handler(domain, level, Some(message));"
if anchor not in source:
    raise SystemExit(f"error: {path} no longer has the handler this probe attaches to")
probe = (
    f"        if message.contains({wanted!r}) {{\n"
    '            log::error!("GTKTRACE {}", std::backtrace::Backtrace::force_capture());\n'
    "        }\n"
) + anchor
open(path, "w").write(source.replace(anchor, probe, 1))
PY
  info "probe added for '$wanted'; its backtrace lands in the app's own log, tagged GTKTRACE"
  "$@"
}

mode_squatters() {
  require_cmd busctl
  # A leaked run of a widget test keeps the application id it registered, and every later run then
  # becomes the *remote* instance and blocks talking to it. The name outlives the run by hours.
  local id
  id="$(sed -n 's/^MAILCAL_APP_ID=//p' "$REPO_ROOT/branding/allodia.env" 2>/dev/null |
    tr -d '"' | head -1)"
  id="${id:-$(sed -n 's/^MAILCAL_APP_ID=//p' "$REPO_ROOT/branding/default.env" |
    tr -d '"' | head -1)}"
  info "bus names under '$id'"
  busctl --user list --no-pager | grep -F "$id" ||
    info "none: no leaked run is holding an application id"
}

case "${1:-}" in
  test) shift; mode_test "$@" ;;
  dbus) shift; mode_dbus "$@" ;;
  probe) shift; mode_probe "$@" ;;
  squatters) shift; mode_squatters "$@" ;;
  -h | --help) usage 0 ;;
  *) usage ;;
esac
