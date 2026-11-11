#!/usr/bin/env bash
# The Linux UI suite's fixtures and the app it drives: launching, stopping and relaunching the
# client, and the steps more than one leg takes. Sourced by test-linux-ui.sh.

PYTHON=/usr/bin/python3
ATSPI_DRIVER="$REPO_ROOT/scripts/dev/linux_ui_atspi.py"
APP_PID=""
ATSPI_BUS_PID=""
ATSPI_REGISTRY_PID=""
PORTAL_PID=""
MCP_RELAY_PID=""
MCP_INPUT_FD=""
MCP_OUTPUT_FD=""

REMOTE_SUBJECT="HTML message with a remote image"
# Who that fixture is from, and so who a reply to it is addressed to.
REMOTE_SENDER="news@example.com"
REMOTE_SENDER_NAME="Newsletter"
# The fixture a forward is driven from, and the one file it carries. Both from the deterministic
# seed rather than the bulk one, which --start-harness does not seed.
ATTACHMENT_SUBJECT="Message with a text attachment"
ATTACHMENT_FILE="report.csv"
# Somewhere else to navigate to, for the leg that asks whether abandoning a forward stops the user.
PLAIN_SUBJECT="Harness baseline message"
# Two of the seed's conversations (docker/stalwart/README.md), each listed under its newest
# message's subject: the newest on the first is received, the newest on the second a draft filed
# in Drafts.
CONVERSATION_SUBJECT="Re: Venue for the spring meeting"
DRAFT_CONVERSATION_SUBJECT="Re: Contribution for next year"
# The three seeded iMIP fixtures (docker/stalwart/seed.sh). Monday's sits in the review/triage
# overlap; the weekend one is on the day the living week deliberately leaves empty, which is the
# only fixture that can tell the preview rule from the one it replaced; the published .ics is the
# contrast that proves the gate is the METHOD and not the media type.
INVITE_SUBJECT="Quarterly planning"
FREE_DAY_SUBJECT="Weekend walk"
PUBLISH_SUBJECT="Annual general meeting"
# The one string the hold contract binds. The sentence around it is the fixture's own title, a time
# range and a calendar name, so this is matched as a substring rather than pinning the fixture.
AWAITING_LABEL="Awaiting your response"
# This run's own records, each unique to it: an event, a meeting Alice invites Bob and Dave to, a
# meeting Bob invites Alice to, and a message the mail-action leg delivers and archives again.
CALENDAR_CRUD_TITLE="Linux acceptance event ${RUN_STAMP:-}"
MEETING_TITLE="Linux acceptance meeting ${RUN_STAMP:-}"
ANSWER_TITLE="Linux acceptance answer ${RUN_STAMP:-}"
MAIL_ACTION_SUBJECT="Linux acceptance action ${RUN_STAMP:-}"
# The person the harness files in *both* address books, so a two-account boot merges them.
MERGED_CONTACT="Iris Jansen"
MAILTO_URI="mailto:ada@example.test?cc=copy@example.test&bcc=audit@example.test&subject=Linux%20mail%20link&body=Hello%20from%20a%20mail%20link%0ASecond%20line"

# Where the session keeps what the legs read back, once the session's own directories are set.
session_paths() {
  APP_LOG="$XDG_DATA_HOME/$MAILCAL_DATA_DIR_NAME/mailcal.log"
  CORE_PREFERENCES="$XDG_DATA_HOME/$MAILCAL_DATA_DIR_NAME/dev/preferences.toml"
  HOST_PREFERENCES="$XDG_CONFIG_HOME/mailcal/host.json"
  PORTAL_CAPTURE="$ARTIFACT_DIR/notifications.jsonl"
  # Where the app's test agent listens: this run's private runtime directory, which the sandbox
  # shares with the host through `--filesystem=/tmp` as well as the agent's own grant.
  UI_SOCKET_PATH="$(linux_agent_socket)"
}

# The AT-SPI driver, for an assertion about what a screen reader gets.
atspi() { "$PYTHON" "$ATSPI_DRIVER" "$@"; }

# The compositor's output, which is this client tiled full-bleed on it, so the capture is the
# window. It is also the whole of what is on screen, so unlike a window capture it holds the
# popovers: a menu, an autosuggest list and a tooltip are each their own surface.
capture() {
  linux_session_capture "$ARTIFACT_DIR/$1.png"
}

# Polls a test for up to five seconds; the test is a command and its arguments.
eventually() {
  for _ in {1..100}; do
    "$@" && return 0
    sleep 0.05
  done
  "$@"
}

# Start the client on the private session, opening <subject> in the reading pane, and log to
# <log-prefix>.stdout.log / .stderr.log.
#
# The app runs inside the GNOME runtime it ships on; the driver stays out here on the host. That is
# the whole point of this run; the toolkit under test is the one users get, not whichever GTK this
# distribution happens to carry (scripts/dev/sdk.sh).
#
# WebKit's own bubblewrap cannot create a second user namespace inside flatpak's, and the sandbox
# around this deterministic harness run is flatpak's own.
#
# The test agent is named here and nowhere else: `flatpak run` passes the caller's environment on,
# so an exported MAILCAL_TEST_AGENT would reach the mail-link leg's second launch too, which binds
# the socket afresh before it learns that it is not the primary instance and hands over.
launch_app() { # <open-subject> <log-prefix> [arguments...]
  local open_subject="$1"
  local log_prefix="$2"
  shift 2
  local -a agent_args
  mapfile -t agent_args < <(linux_agent_sandbox_args)
  # No --env=WAYLAND_DISPLAY: `sdk_exec` passes --socket=wayland, and flatpak binds whatever
  # $WAYLAND_DISPLAY names on the host to `wayland-0` *inside* the sandbox. Naming the host's
  # socket here would name one that does not exist in there, and the client would find no display.
  sdk_exec \
    --no-a11y-bus \
    "${agent_args[@]}" \
    --env=MAILCAL_TEST_AGENT="$UI_SOCKET_PATH" \
    --env=AT_SPI_BUS_ADDRESS="$AT_SPI_BUS_ADDRESS" \
    --env=GTK_A11Y=atspi \
    --env=LIBGL_ALWAYS_SOFTWARE=1 \
    --env=WEBKIT_DISABLE_DMABUF_RENDERER=1 \
    --env=WEBKIT_DISABLE_SANDBOX_THIS_IS_DANGEROUS=1 \
    --env=LANG=C.UTF-8 \
    --env=MAILCAL_DEV_ACCOUNT="${MAILCAL_DEV_ACCOUNT:-}" \
    --env=MAILCAL_EXTRA_CA="${MAILCAL_EXTRA_CA:-}" \
    --env=MAILCAL_CALENDAR_VIEW="${MAILCAL_CALENDAR_VIEW:-}" \
    --env=MAILCAL_CALENDAR_REFRESH_LIMIT=2 \
    --env=MAILCAL_CALENDAR_REFRESH_SECONDS=1 \
    --env=MAILCAL_DIAGNOSTICS_EXPORT_PATH="$ARTIFACT_DIR/exported-mailcal.log" \
    --env=MAILCAL_FAKE_DEVICE_TIMEZONE="${MAILCAL_FAKE_DEVICE_TIMEZONE:-}" \
    --env=MAILCAL_FAKE_SYNC_PROGRESS=1200/3387 \
    --env=MAILCAL_FORCE_ANALYTICS_WELCOME=1 \
    --env=MAILCAL_OPEN_SUBJECT="$open_subject" \
    --env=MAILCAL_SHOWCASE_SCREEN="${MAILCAL_SHOWCASE_SCREEN:-}" \
    --env=MAILCAL_SHOWCASE="${MAILCAL_SHOWCASE:-}" \
    --env=MAILCAL_SHOWCASE_SERVER_CHANGE="${MAILCAL_SHOWCASE_SERVER_CHANGE:-}" \
    --env=MAILCAL_SHOWCASE_REAUTH="${MAILCAL_SHOWCASE_REAUTH:-}" \
    --env=MAILCAL_SHOWCASE_QUESTIONS="${MAILCAL_SHOWCASE_QUESTIONS:-}" \
    --env=XDG_DATA_HOME="$XDG_DATA_HOME" \
    --env=XDG_CONFIG_HOME="$XDG_CONFIG_HOME" \
    --env=XDG_CACHE_HOME="$XDG_CACHE_HOME" \
    "$(sdk_target_dir)/debug/mailcal-linux" "$@" \
    >"$log_prefix.stdout.log" 2>"$log_prefix.stderr.log" &
  APP_PID=$!
}

# Stop the running client and wait for it to leave the accessibility bus, so the next launch owns
# that bus alone.
#
# **Signalling `APP_PID` is not enough, and the way it fails is silent.** `sdk_exec` runs the client
# inside flatpak, so `APP_PID` is the *wrapper*: killing it leaves `bwrap` and the app running, still
# publishing their tree; and the next launch's assertions then read the previous instance, which is
# a pass or a fail decided by whichever process answered first.
#
# **`flatpak kill` goes first, and it takes the INSTANCE id, not the app id.** The client runs inside
# the `org.gnome.Sdk` *runtime* (`flatpak run --devel --command=`), which registers no application
# id; so `flatpak kill org.gnome.Sdk` answers "org.gnome.Sdk is not running" over an instance
# `flatpak ps` is listing on the line above. And it has to go first: the wrapper owns the instance
# registration under `XDG_RUNTIME_DIR`, so killing the wrapper *deregisters* the instance and leaves
# an app nothing can name any more. This session's `XDG_RUNTIME_DIR` is private, so every matching
# instance belongs to this run. The mail-link leg briefly starts a second one to exercise
# GApplication forwarding; teardown removes it too if its wrapper is still alive.
#
# The `gone` wait is what makes any of it a barrier rather than a hope; the agent's socket going
# with the app is not, because the next launch replaces the socket file whatever is left of it.
stop_app() { # [bus-barrier-timeout: 0 to skip, for the teardown path]
  [[ -n "${APP_PID:-}" ]] || return 0
  local instance
  while read -r instance; do
    [[ -z "$instance" ]] || flatpak kill "$instance" >/dev/null 2>&1 || true
  done < <(flatpak ps 2>/dev/null | awk '$NF == "org.gnome.Sdk" { print $1 }')
  kill -TERM "$APP_PID" 2>/dev/null || true
  # Skipped on the way out: teardown has no next launch to protect, and a failing run should not
  # spend the barrier's timeout before printing where its artifacts are.
  [[ "${1:-45}" == 0 ]] || atspi gone --timeout "${1:-45}"
  for _ in {1..40}; do
    kill -0 "$APP_PID" 2>/dev/null || break
    sleep 0.1
  done
  kill -KILL "$APP_PID" 2>/dev/null || true
  wait "$APP_PID" 2>/dev/null || true
  APP_PID=""
}

# After a launch: the agent answers from the new app. The client's connection to the one before
# it answers "hung-up" once, and the next request waits for the new app's socket.
app_ready() {
  for _ in 1 2 3; do
    UI_QUIET=1 ui ping && return 0
    [[ "$UI_REPLY" == *'"code": "hung-up"'* ]] || break
  done
  die "the app's test agent did not answer after the launch: $UI_REPLY"
}

# Relaunch on a different fixture account; only the cross-account contacts leg needs it.
reopen_app_on() { # <subject> <log-prefix>
  stop_app
  launch_app "$1" "$ARTIFACT_DIR/$2"
  app_ready
  ui_wait id=message-list timeoutMs=60000
}

# Settings, on <category> when one is named. The category buttons say the same words as the main
# window's destinations (Mail, Calendar), so they are looked for in the Settings window alone.
open_settings() { # [category]
  ui_press text=Settings role=button
  ui_wait type=GtkWindow text=Settings
  [[ -z "${1:-}" ]] || ui_press text="$1" role=toggle-button window=Settings
}

close_settings() {
  ui_press icon=window-close-symbolic window=Settings
  ui_gone type=GtkWindow text=Settings
}

# The centre of the first element in the last reply, as a point on the compositor's output. The
# agent measures an element in its own window, so the window's place is added; the element is
# taken to be in the active window, which the agent lists first.
ui_centre() {
  local number='([0-9]+)(\.[0-9]+)?'
  [[ "$UI_REPLY" =~ \"bounds\":\ \[$number,\ $number,\ $number,\ $number\] ]] ||
    die "the element has no bounds to click: $UI_REPLY"
  local x="${BASH_REMATCH[1]}" y="${BASH_REMATCH[3]}" w="${BASH_REMATCH[5]}" h="${BASH_REMATCH[7]}"
  ui windows
  [[ "$UI_REPLY" =~ \"title\":\ \"(([^\"\\]|\\.)*)\" ]] || die "the app reports no window"
  local title="${BASH_REMATCH[1]}" geometry
  geometry="$(linux_session_windows | awk -F'\t' -v title="$title" '$2 == title { print $3 }')"
  [[ "$geometry" =~ \+([0-9]+)\+([0-9]+)$ ]] || die "no window titled '$title' on the compositor"
  printf '%d %d\n' "$((BASH_REMATCH[1] + x + w / 2))" "$((BASH_REMATCH[2] + y + h / 2))"
}

# Types <query> into the search field on screen. The field reports the change a moment after the
# text changes, and a search landing after the next step navigates away from whatever that step
# opened, so the app is let settle before anything else happens.
search_for() { # <query>
  ui_fill type=GtkSearchEntry value="$1"
  ui settle
}

# Milliseconds since <start>, an EPOCHREALTIME.
elapsed_ms() { # <start>
  awk -v a="$1" -v b="$EPOCHREALTIME" 'BEGIN { printf "%d", (b - a) * 1000 }'
}

# The `value` of the first element in the last reply, as the app holds it: an entry's text.
ui_value() {
  "$PYTHON" -c 'import json, sys; print(json.loads(sys.argv[1])["elements"][0]["value"])' "$UI_REPLY"
}
