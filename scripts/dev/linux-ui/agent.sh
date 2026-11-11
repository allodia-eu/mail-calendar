#!/usr/bin/env bash
# The app's test agent (docs/test-agent.md) over one open connection, for the Linux UI suite.
# Sourced by test-linux-ui.sh; needs PYTHON, REPO_ROOT, ARTIFACT_DIR and `die` from lib.sh.
#
# One `agent.py` process runs for the whole session in `--lines` mode and is spoken to through a
# FIFO pair. It reconnects by itself when the app is relaunched, so a leg that restarts the app
# keeps using it. A request costs a round trip on that connection, about a tenth of what a new
# Python process costs and far less than an AT-SPI walk of the whole tree.
#
# FIFOs rather than a coproc: bash tracks one coprocess at a time, and the MCP leg holds that one
# for the relay it drives.
#
# Requests are written as agent.py's own command line takes them, `verb key=value…`, so a line
# here can be tried by hand with `scripts/dev/control.sh linux agent`.

UI_SOCKET=""
UI_PID=""
UI_IN=""
UI_OUT=""
UI_REPLY=""
UI_H=""

# The fields whose value is always text, as agent.py's TEXT_FIELDS: `text=2024` is a label.
UI_TEXT_FIELDS=" text textLike type id name icon role window value script path target "

ui_start() { # <socket>
  UI_SOCKET="$1"
  local fifos
  fifos="$(mktemp -d "${TMPDIR:-/tmp}/mailcal-ui-agent.XXXXXX")"
  mkfifo "$fifos/requests" "$fifos/replies"
  "$PYTHON" "$REPO_ROOT/scripts/dev/agent.py" --lines --socket "$UI_SOCKET" --connect-ms 60000 \
    <"$fifos/requests" >"$fifos/replies" 2>"$ARTIFACT_DIR/agent-client.stderr.log" &
  UI_PID=$!
  # Opened in the order the client opens them, so neither side waits on the other for ever.
  exec {UI_IN}>"$fifos/requests" {UI_OUT}<"$fifos/replies"
  rm -rf -- "$fifos"
}

# Ends the client. Its input does not reach end of file on its own: every app launched since it
# started holds the FIFO open too, having inherited the descriptor.
ui_stop() {
  [[ -n "$UI_PID" ]] || return 0
  exec {UI_IN}>&- {UI_OUT}<&-
  kill -TERM "$UI_PID" 2>/dev/null || true
  wait "$UI_PID" 2>/dev/null || true
  UI_PID=""
}

# Sets <variable> to <text> as a JSON string.
ui_json_string() { # <variable> <text>
  local text="$2"
  text="${text//\\/\\\\}"
  text="${text//\"/\\\"}"
  text="${text//$'\n'/\\n}"
  text="${text//$'\r'/\\r}"
  text="${text//$'\t'/\\t}"
  printf -v "$1" '"%s"' "$text"
}

# Sets <variable> to the JSON object `key=value…` describes. A value that reads as JSON is sent as
# JSON, as agent.py does: `h=12`, `absent=true`, `has={"text":"Inbox"}`.
ui_object() { # <variable> [key=value…]
  local variable="$1" object="" value key pair
  shift
  for pair in "$@"; do
    [[ "$pair" == ?*=* ]] || die "agent request: '$pair' is not key=value"
    key="${pair%%=*}"
    value="${pair#*=}"
    if [[ "$UI_TEXT_FIELDS" == *" $key "* ]] ||
      ! [[ "$value" =~ ^(true|false|null|-?[0-9]+|\{.*\}|\[.*\])$ ]]; then
      ui_json_string value "$value"
    fi
    object+="${object:+,}\"$key\":$value"
  done
  printf -v "$variable" '{%s}' "$object"
}

# One request, `verb key=value…`; its reply is left in UI_REPLY. Returns 1 on a refusal, which it
# describes on stderr unless UI_QUIET is set.
ui() { # <verb> [key=value…]
  local request
  ui_object request verb="$@"
  printf '%s\n' "$request" >&"$UI_IN"
  IFS= read -r -u "$UI_OUT" UI_REPLY || die "the test agent's client went away"
  [[ "$UI_REPLY" == '{"ok": true'* ]] && return 0
  [[ -n "${UI_QUIET:-}" ]] || printf 'test agent refused %s\n  %s\n' "$request" "$UI_REPLY" >&2
  return 1
}

# The default the AT-SPI driver had: twenty seconds, unless the call names its own.
ui_timed() { # <verb> [key=value…]
  local verb="$1" pair
  shift
  for pair in "$@"; do
    [[ "$pair" == timeoutMs=* ]] && { ui "$verb" "$@"; return; }
  done
  ui "$verb" "$@" timeoutMs=20000
}

ui_wait() { ui_timed wait "$@"; }
ui_gone() { ui_timed wait absent=true "$@"; }
# A press or a fill waits for a control a person could use: one that is still disabled, such as an
# action waiting for its message to open, is waited for rather than refused.
ui_press() { ui_timed press enabled=true "$@"; }
ui_fill() { ui_timed fill enabled=true "$@"; }

# Exactly one element, whose handle is left in UI_H: the scope a later `under=` names.
ui_one() { # <filter…>
  ui_wait "$@" || return 1
  [[ "$UI_REPLY" =~ \"h\":\ ([0-9]+) ]] || die "the test agent found no element for $*"
  UI_H="${BASH_REMATCH[1]}"
  # Every element carries one `"h": ` and nothing else does, so counting them counts elements.
  local marks="${UI_REPLY//\"h\": /$'\x01'}"
  marks="${marks//[^$'\x01']/}"
  (( ${#marks} == 1 )) || die "the test agent found ${#marks} elements for $*, not one"
}

# <verb> on <inner> inside the element <outer> finds. A list rebuilds its rows as the core's
# snapshots arrive, so a row found a moment ago can be gone by the next request. A wait is one
# filter the agent asks again on every poll (<outer> that `has` <inner>); a press or a fill finds
# the row again when it went in between. A timeout belongs with <outer>.
ui_within() { # <wait|gone|press|fill> <outer filter…> -- <inner filter…>
  local verb="$1" outer=() inner=()
  shift
  while [[ $# -gt 0 && "$1" != -- ]]; do
    outer+=("$1")
    shift
  done
  shift
  inner=("$@")
  local has
  ui_object has "${inner[@]}"
  case "$verb" in
    wait | gone)
      "ui_$verb" "${outer[@]}" has="$has"
      return
      ;;
  esac
  ui_wait "${outer[@]}" has="$has" || return 1
  for _ in {1..20}; do
    ui_one "${outer[@]}" || return 1
    UI_QUIET=1 "ui_$verb" under="$UI_H" "${inner[@]}" && return 0
    [[ "$UI_REPLY" == *'"code": "gone"'* ]] || break
    sleep 0.1
  done
  printf 'test agent refused %s inside %s\n  %s\n' "${inner[*]}" "${outer[*]}" "$UI_REPLY" >&2
  return 1
}

# Turns the one switch, check or toggle <filter> finds on or off, as a person flipping it would.
ui_switch() { # <true|false> <filter…>
  local active="$1"
  shift
  ui_one "$@" || return 1
  ui toggle h="$UI_H" active="$active"
}

# The first string of the first element's `text` in UI_REPLY.
ui_first_text() {
  [[ "$UI_REPLY" =~ \"text\":\ \[\"(([^\"\\]|\\.)*)\" ]] || return 1
  printf '%b' "${BASH_REMATCH[1]//\\\"/\"}"
}
