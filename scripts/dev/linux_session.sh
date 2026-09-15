#!/usr/bin/env bash
# A private headless Wayland session for the Linux client: start it, photograph it, type into it,
# take it away again. Sourced by screenshot.sh, control.sh, showcase.sh, test-linux-ui.sh and
# clients/linux/build-and-run.sh; not run directly.
#
# **Why a compositor of our own, rather than the developer's desktop.** GNOME's compositor offers
# a Wayland client no way to read pixels: `wayland-info` on a GNOME session lists no capture
# protocol at all, neither `zwlr_screencopy_manager_v1` nor `ext_image_copy_capture_v1`, and that
# is a decision rather than a version, so no newer grim reaches it. The D-Bus doors are shut too:
# `org.gnome.Shell.Screenshot.Screenshot`, `.ScreenshotWindow` and
# `org.gnome.Shell.Introspect.GetWindows` all answer `AccessDenied`. What is left there is the
# desktop portal, which hands back the **whole screen** and nothing smaller.
#
# sway on its headless backend has all of it: it tiles one client full-bleed with no border, so
# its output *is* the window, `grim` reads that over `wlr-screencopy` with no portal permission
# and no focus, and `swaymsg -t get_tree` states the geometry GNOME will not. The app runs on the
# Wayland backend with the GL renderer, which is what ships.
#
# ⚠️ **grim reads the output, so it sees popovers.** A menu, an autosuggest list, a dropdown and a
# tooltip are each a surface of their own, which is why a *window* capture misses them; an output
# capture does not. A popover missing from a capture taken here is a real finding about the app.

if [[ ${BASH_VERSION%%.*} -lt 5 ]]; then
  echo "error: ${0##*/} needs bash 5 or newer, and got ${BASH_VERSION:-no bash at all}" >&2
  exit 1
fi

# One session at a time, addressed by a marker so separate script invocations find the same one:
# `build-and-run.sh --headless` starts it and exits, and the `screenshot.sh` and `control.sh` calls
# after it have no other way to know it is there. It carries the compositor's pid, so a marker left
# behind by a run that was killed is detectably stale rather than quietly wrong.
LINUX_SESSION_MARKER="${XDG_RUNTIME_DIR:-/run/user/$(id -u)}/mailcal-linux-session"

# The Wayland sockets that exist now, each as `name:inode`. The compositor names its own socket
# (`wl_display_add_socket_auto`), so a run identifies it by taking the one that was not there
# before: the developer's session already owns wayland-0, and a stale compositor from an
# interrupted run can own more.
#
# The inode is what makes that work across a sweep. The compositor releases its number when it
# exits and the next capture is handed the *same name* back; comparing names alone then finds
# nothing new and the second screenshot of every run fails. A reused name is still a newly created
# file, so it has a new inode.
linux_session_sockets() {
  local runtime="${XDG_RUNTIME_DIR:-/run/user/$(id -u)}" sock
  for sock in "$runtime"/wayland-*; do
    [[ -S "$sock" ]] || continue
    printf '%s:%s ' "$(basename "$sock")" "$(stat -c '%i' "$sock" 2>/dev/null)"
  done
}

linux_session_new_socket() { # <linux_session_sockets output from before the compositor started>
  local waited=0 entry
  while [[ "$waited" -lt 40 ]]; do
    for entry in $(linux_session_sockets); do
      case " $1 " in *" $entry "*) continue ;; esac
      printf '%s\n' "${entry%:*}"
      return 0
    done
    sleep 0.5
    waited=$((waited + 1))
  done
  return 1
}

linux_session_requirements() {
  require_cmd sway
  require_cmd grim
}

# Start an empty compositor. The caller launches the client against it afterwards:
#
#   linux_session_start <width>x<height> <scale> <dialog-app-id>
#   WAYLAND_DISPLAY="$LINUX_SESSION_DISPLAY" <the client>
#
# Sets LINUX_SESSION_DISPLAY, LINUX_SESSION_SWAYSOCK and LINUX_SESSION_PID in the caller, and
# writes the marker.
#
# Empty rather than `exec`ing the client from sway's config, because on the default build the
# client runs **inside the Flatpak sandbox**, reached through a shell function rather than a
# command line a config can hold. Connecting afterwards works for both builds, and flatpak binds
# whatever `$WAYLAND_DISPLAY` names, so the sandboxed client lands on this compositor unchanged.
# The client exits by itself when the compositor goes away.
#
# `--unsupported-gpu` is required: sway refuses to start under the proprietary Nvidia driver, over
# a GPU the headless backend never touches. `WLR_RENDERER=pixman` drops it to software rendering
# for a runner with no GPU at all; the capture is the same size and the same pixels.
#
# ⚠️ **The config floats this client's dialogs, because sway tiles them.** Settings, the signature
# editor and the first-account screen are each a window of their own, and a tiling compositor gives
# a second window half the output and shrinks the app into the other half: a capture then shows two
# half-width windows side by side, which is not what a desktop puts on screen. Those dialogs carry
# the **process name** as their Wayland `app_id`, not the branded application id the main window
# has, which is why the criterion is passed in from the binary's own path rather than read from the
# brand files.
linux_session_start() { # <resolution> <scale> <dialog-app-id>
  local resolution="$1" scale="$2" dialog_app_id="$3"
  linux_session_requirements

  local config before
  config="$(mktemp)"
  cat >"$config" <<CONFIG
output HEADLESS-1 resolution $resolution scale $scale
default_border none
default_floating_border none
gaps inner 0
gaps outer 0
for_window [app_id="$dialog_app_id"] floating enable, move position center
CONFIG
  before="$(linux_session_sockets)"
  env -u DISPLAY -u WAYLAND_DISPLAY \
    WLR_BACKENDS=headless WLR_LIBINPUT_NO_DEVICES=1 \
    sway --unsupported-gpu -c "$config" >"${LINUX_SESSION_LOG:-/dev/null}" 2>&1 &
  LINUX_SESSION_PID=$!
  # Detach it, so a later pkill does not print a "Terminated" job notice over the log.
  disown

  LINUX_SESSION_DISPLAY="$(linux_session_new_socket "$before")" || {
    rm -f "$config"
    kill -KILL "$LINUX_SESSION_PID" 2>/dev/null
    die "sway opened no Wayland socket, so it did not start. What it printed: ${LINUX_SESSION_LOG:-nothing, set LINUX_SESSION_LOG to keep it}"
  }
  LINUX_SESSION_SWAYSOCK="${XDG_RUNTIME_DIR:-/run/user/$(id -u)}/sway-ipc.$(id -u).$LINUX_SESSION_PID.sock"
  printf '%s\n%s\n%s\n' \
    "$LINUX_SESSION_PID" "$LINUX_SESSION_DISPLAY" "$LINUX_SESSION_SWAYSOCK" >"$LINUX_SESSION_MARKER"
  rm -f "$config"
  export LINUX_SESSION_DISPLAY LINUX_SESSION_SWAYSOCK LINUX_SESSION_PID
}

# Load a session started by an earlier invocation. Returns non-zero when there is none, so a caller
# can fall back rather than fail.
linux_session_attach() {
  [[ -f "$LINUX_SESSION_MARKER" ]] || return 1
  local pid display swaysock
  { read -r pid; read -r display; read -r swaysock; } <"$LINUX_SESSION_MARKER" || return 1
  # A marker whose compositor is gone is stale. Take it away rather than reporting it, so the next
  # caller sees no session instead of the same dead one.
  if ! kill -0 "$pid" 2>/dev/null; then
    rm -f "$LINUX_SESSION_MARKER"
    return 1
  fi
  LINUX_SESSION_PID="$pid"
  LINUX_SESSION_DISPLAY="$display"
  LINUX_SESSION_SWAYSOCK="$swaysock"
  export LINUX_SESSION_DISPLAY LINUX_SESSION_SWAYSOCK LINUX_SESSION_PID
  return 0
}

# ⚠️ SIGKILL the compositor. wlroots aborts inside `wl_display_terminate`, and each abort files an
# apport crash report: 49 of them across a full showcase set. Taking its client away first reaches
# the same call. SIGKILL has no core-dump action, and grim has already read the pixels.
linux_session_stop() {
  [[ -n "${LINUX_SESSION_PID:-}" ]] && kill -KILL "$LINUX_SESSION_PID" 2>/dev/null
  rm -f "$LINUX_SESSION_MARKER"
  LINUX_SESSION_PID=""
  LINUX_SESSION_DISPLAY=""
  LINUX_SESSION_SWAYSOCK=""
  return 0
}

# ⚠️ **An empty compositor photographs as a clean PNG of nothing**, at the right size, which reads
# like a pass: the client exited, or never started, and the capture says so only if someone looks
# at it. So the window list is checked first, and a capture with nothing on the output is an error
# rather than a file.
linux_session_capture() { # <out.png>
  [[ -n "${LINUX_SESSION_DISPLAY:-}" ]] || die "no headless session: call linux_session_start or linux_session_attach first"
  [[ -n "$(linux_session_windows)" ]] ||
    die "the headless compositor has no window on it: the client exited or never started.
     Its log: ${XDG_DATA_HOME:-$HOME/.local/share}/mailcal/mailcal-launch.log"
  mkdir -p "$(dirname "$1")"
  WAYLAND_DISPLAY="$LINUX_SESSION_DISPLAY" grim "$1" ||
    die "grim could not capture the compositor's output on $LINUX_SESSION_DISPLAY"
}

# What is on the compositor, as `app_id<TAB>title<TAB>WxH+X+Y`. This is the geometry GNOME refuses
# to give a script, and it is how a capture proves it photographed the client rather than an empty
# output.
linux_session_windows() {
  [[ -n "${LINUX_SESSION_SWAYSOCK:-}" ]] || die "no headless session to inspect"
  SWAYSOCK="$LINUX_SESSION_SWAYSOCK" swaymsg -t get_tree -r 2>/dev/null |
    "${MAILCAL_PYTHON:-/usr/bin/python3}" -c '
import json, sys


def walk(node):
    if node.get("app_id"):
        rect = node["rect"]
        geometry = "%dx%d+%d+%d" % (rect["width"], rect["height"], rect["x"], rect["y"])
        print("\t".join([node["app_id"], node.get("name") or "", geometry]))
    for child in node.get("nodes", []) + node.get("floating_nodes", []):
        walk(child)


walk(json.load(sys.stdin))
'
}

# Type into the session. `wtype` carries its own virtual keyboard, so this works where the
# developer's GNOME session offers nothing at all.
#
# ⚠️ **The first keystroke is dropped without a settle.** `wtype` creates the keyboard, uploads its
# keymap and sends the key in one run, and the compositor has not routed focus to the new device by
# the time the key arrives: measured here, a bare `wtype -k Escape` left the popover open and
# `wtype -s 300 -k Escape` closed it, and the first character of a bare `wtype "Hello"` was lost.
# So every call sleeps first. That is also why this is not a thin alias.
linux_session_type() { # <wtype args...>
  [[ -n "${LINUX_SESSION_DISPLAY:-}" ]] || die "no headless session to type into"
  require_cmd wtype
  WAYLAND_DISPLAY="$LINUX_SESSION_DISPLAY" wtype -s "${MAILCAL_WTYPE_SETTLE_MS:-300}" "$@"
}
