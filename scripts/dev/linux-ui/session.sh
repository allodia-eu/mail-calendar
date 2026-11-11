#!/usr/bin/env bash
# The Linux UI suite's legs, in the order a whole run takes them, and the session they run in.
# Sourced by test-linux-ui.sh.
#
# A leg is a function `leg_<name>` that starts from the mail screen with nothing open over it and
# leaves it that way, unless the leg after it in this list carries on from where it stopped, which
# LEG_NEEDS then says. `--leg` runs the welcome and the time zone question (the app's own first
# screens, which every launch here raises), what the leg needs, and the leg.

LEGS=(
  welcome zone-question mailbox settings mcp notifications reply forward print search
  calendar jitsi events meeting answer invitations contacts autosuggest mail-actions
  conversation-menu draft-resume signatures mailto-warm contacts-merge mailto-cold
  notification-click server-change reauth-notices question-notices
)

declare -A LEG_ABOUT=(
  [welcome]="the analytics welcome: consent, the payload window, and its persisted install id"
  [zone-question]="the device time zone question, answered from the banner's strip"
  [mailbox]="the mail chrome, the command bar at two widths, folder expansion and switch timing"
  [settings]="Privacy (withdrawing consent) and Diagnostics (view, copy, export, detail)"
  [mcp]="the MCP server: each grant through Settings, then four calls over the shipped relay"
  [notifications]="new-mail notifications on, off and on again, through the desktop portal"
  [reply]="blocked remote images, opt-in, Reply with the derived recipient, Send"
  [forward]="a forward and its files; leaving it untouched, and leaving it changed, then Discard"
  [print]="printing a message twice through the print portal, read back as PDF text"
  [search]="search: narrowing, threading, how far back, the scope filter, clearing it"
  [calendar]="the calendar tab by pointer, the agenda's hold, and managing calendars"
  [jitsi]="Settings, Videoconferencing: a refused and a stored Jitsi server"
  [events]="an event created, edited and deleted, offering the stored meeting server"
  [meeting]="organising a meeting whose invitations and cancellations reach the invitees"
  [answer]="answering an invitation from the event in the calendar"
  [invitations]="the three invitation fixtures: the card, its day preview, and the published .ics"
  [contacts]="contacts: sections, namesakes staying two rows, search, the editor's refusal"
  [autosuggest]="recipient autosuggest from the people index, over a composer Settings opened on"
  [mail-actions]="read, flag and archive on a message the leg delivers for itself"
  [conversation-menu]="a conversation row's menu: Flag and Unflag across the thread"
  [draft-resume]="a draft listed in the Inbox opens a composer and is kept on leaving"
  [signatures]="the signature library: the empty state, a create, twelve edit and save cycles"
  [mailto-warm]="a mail link handed to the running app over an open Settings window"
  [contacts-merge]="two accounts filing one person: the merged row says so"
  [mailto-cold]="a mail link that starts the app"
  [notification-click]="a notification clicked with the app running, and with it quit"
  [server-change]="the showcase's paused calendar: Escape and Return refuse, Allow is refused"
  [reauth-notices]="the showcase's sign-in notices on the mail and calendar screens"
  [question-notices]="the showcase's three questions, answered from Issues and the banner"
)

# What a leg needs to have run first, beyond the welcome and the time zone question.
declare -A LEG_NEEDS=(
  [search]="notifications"
  [events]="jitsi"
  [invitations]="calendar"
  [autosuggest]="contacts"
  [mailto-warm]="signatures"
)

list_legs() {
  local leg
  for leg in "${LEGS[@]}"; do
    printf '%-19s %s%s\n' "$leg" "${LEG_ABOUT[$leg]}" "${LEG_NEEDS[$leg]:+ (needs ${LEG_NEEDS[$leg]})}"
  done
}

leg_known() { [[ -n "${LEG_ABOUT[$1]:-}" ]]; }

# The legs a run takes, in their order: every leg, or the chosen ones and what they need.
legs_to_run() { # [leg…]
  if [[ $# == 0 ]]; then
    printf '%s\n' "${LEGS[@]}"
    return
  fi
  local -A wanted=([welcome]=1 [zone-question]=1)
  local -a queue=("$@")
  local leg need
  while [[ ${#queue[@]} -gt 0 ]]; do
    leg="${queue[0]}"
    queue=("${queue[@]:1}")
    [[ -z "${wanted[$leg]:-}" ]] || continue
    wanted[$leg]=1
    for need in ${LEG_NEEDS[$leg]:-}; do
      queue+=("$need")
    done
  done
  for leg in "${LEGS[@]}"; do
    [[ -z "${wanted[$leg]:-}" ]] || printf '%s\n' "$leg"
  done
}

# Runs one leg and says how long it took, on the console and in leg-times.tsv.
run_leg() { # <leg>
  local started="$EPOCHREALTIME" elapsed
  info "leg $1"
  "leg_${1//-/_}"
  elapsed="$(awk -v a="$started" -v b="$EPOCHREALTIME" 'BEGIN { printf "%.1f", b - a }')"
  printf '%s\t%s\n' "$1" "$elapsed" >>"$ARTIFACT_DIR/leg-times.tsv"
  info "leg $1: ${elapsed} s"
}

run_inside_session() { # [leg…]
  session_paths
  start_accessibility_bus
  start_notification_portal
  bob_invites_alice "$ANSWER_TITLE"
  launch_app "" "$ARTIFACT_DIR/app"
  ui_start "$UI_SOCKET_PATH"
  trap session_cleanup EXIT INT TERM

  local leg
  while IFS= read -r leg; do
    run_leg "$leg"
  done < <(legs_to_run "$@")

  if grep -Eq 'critical from |unhandled error from ' "$APP_LOG"; then
    grep -E 'critical from |unhandled error from ' "$APP_LOG" >"$ARTIFACT_DIR/toolkit-errors.log"
    die "GTK reported a critical or fatal error; see toolkit-errors.log"
  fi
  # The whole accessibility tree, as a screen reader finds it at the end of the run.
  atspi dump >"$ARTIFACT_DIR/accessibility.txt"
}

session_cleanup() {
  local status=$?
  trap - EXIT INT TERM
  set +e
  if [[ -n "${MCP_RELAY_PID:-}" ]]; then
    kill -TERM "$MCP_RELAY_PID" 2>/dev/null || true
    wait "$MCP_RELAY_PID" 2>/dev/null || true
    MCP_RELAY_PID=""
  fi
  if [[ $status -ne 0 ]]; then
    UI_QUIET=1 ui dump && printf '%s\n' "$UI_REPLY" >"$ARTIFACT_DIR/agent-failure.json"
    atspi dump --timeout 2 >"$ARTIFACT_DIR/accessibility-failure.txt" 2>&1
    capture failure
  fi
  ui_stop
  stop_app 0
  bob_withdraws
  for pid in "${PORTAL_PID:-}" "${ATSPI_REGISTRY_PID:-}" "${ATSPI_BUS_PID:-}"; do
    [[ -z "$pid" ]] || kill -TERM "$pid" 2>/dev/null || true
  done
  for pid in "${PORTAL_PID:-}" "${ATSPI_REGISTRY_PID:-}"; do
    [[ -z "$pid" ]] || wait "$pid" 2>/dev/null || true
  done
  if [[ $status -ne 0 ]]; then
    warn "Linux UI acceptance failed; artifacts: $ARTIFACT_DIR"
  fi
  exit "$status"
}

start_accessibility_bus() {
  local -a bus
  mapfile -t bus < <(
    dbus-daemon --config-file="$REPO_ROOT/scripts/dev/atspi-test-bus.conf" \
      --fork --print-address=1 --print-pid=1
  )
  [[ ${#bus[@]} == 2 ]] || die "private AT-SPI bus did not report its address and PID"
  export AT_SPI_BUS_ADDRESS="${bus[0]}"
  ATSPI_BUS_PID="${bus[1]}"
  local registryd
  registryd="$(atspi_registryd_bin)" || die "at-spi2-registryd is required"
  "$registryd" --dbus-name=org.a11y.atspi.Registry \
    >"$ARTIFACT_DIR/atspi-registry.log" 2>&1 &
  ATSPI_REGISTRY_PID=$!
  for _ in {1..100}; do
    gdbus call --address "$AT_SPI_BUS_ADDRESS" \
      --dest org.freedesktop.DBus \
      --object-path /org/freedesktop/DBus \
      --method org.freedesktop.DBus.NameHasOwner org.a11y.atspi.Registry 2>/dev/null |
      grep -q true && return
    kill -0 "$ATSPI_REGISTRY_PID" 2>/dev/null || break
    sleep 0.05
  done
  die "private AT-SPI registry did not become ready"
}

start_notification_portal() {
  "$PYTHON" "$REPO_ROOT/scripts/dev/linux_notification_portal.py" "$PORTAL_CAPTURE" \
    >"$ARTIFACT_DIR/notification-portal.log" 2>&1 &
  PORTAL_PID=$!
  for _ in {1..100}; do
    gdbus call --session \
      --dest org.freedesktop.DBus \
      --object-path /org/freedesktop/DBus \
      --method org.freedesktop.DBus.NameHasOwner org.freedesktop.portal.Desktop 2>/dev/null |
      grep -q true && return
    kill -0 "$PORTAL_PID" 2>/dev/null || break
    sleep 0.05
  done
  die "notification portal fixture did not become ready"
}
