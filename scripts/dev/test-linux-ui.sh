#!/usr/bin/env bash
# Run the Linux reading/composer + search + calendar + invitations + contacts + mail actions +
# signatures + MCP + cross-account merge acceptance path on a private X11 + D-Bus session. Controls
# are selected through GTK's AT-SPI tree, never by screen coordinates. The only mailbox is the local
# Stalwart fixture, and every screenshot/tree/log is kept under target/ui-test-artifacts for inspection.
#
#   scripts/dev/test-linux-ui.sh
#   scripts/dev/test-linux-ui.sh --start-harness
#   scripts/dev/test-linux-ui.sh --no-build --artifacts /tmp/mailcal-linux-ui
set -euo pipefail
source "$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)/lib.sh"
source "$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)/sdk.sh"

SELF="$REPO_ROOT/scripts/dev/test-linux-ui.sh"
START_HARNESS=0
NO_BUILD=0
INSIDE_SESSION=0
ARTIFACT_DIR=""
APP_PID=""
ATSPI_BUS_PID=""
ATSPI_REGISTRY_PID=""
PORTAL_PID=""
MCP_RELAY_PID=""
MCP_INPUT_FD=""
MCP_OUTPUT_FD=""

while [[ $# -gt 0 ]]; do
  case "$1" in
    --start-harness) START_HARNESS=1; shift ;;
    --no-build) NO_BUILD=1; shift ;;
    --artifacts) ARTIFACT_DIR="${2:?--artifacts needs a directory}"; shift 2 ;;
    --inside-session) INSIDE_SESSION=1; shift ;;
    -h|--help)
      sed -n '2,9p' "$SELF"
      exit 0
      ;;
    *) die "unknown argument '$1' (--start-harness|--no-build|--artifacts)" ;;
  esac
done

PYTHON=/usr/bin/python3
ATSPI="$REPO_ROOT/scripts/dev/linux_ui_atspi.py"
REMOTE_SUBJECT="HTML message with a remote image"
# Who that fixture is from, and so who a reply to it is addressed to.
REMOTE_SENDER="news@example.com"
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
CALENDAR_CRUD_TITLE="${CALENDAR_CRUD_TITLE:-}"
# The message the mail-action leg delivers for itself and archives again, so the leg leaves the
# seeded mailbox as it found it however often the suite runs.
MAIL_ACTION_SUBJECT="${MAIL_ACTION_SUBJECT:-}"
# The person the harness files in *both* address books, so a two-account boot merges them.
MERGED_CONTACT="Iris Jansen"

capture() {
  local name="$1"
  MAILCAL_LINUX_HEADLESS=1 "$REPO_ROOT/scripts/dev/screenshot.sh" linux \
    "$ARTIFACT_DIR/$name.png" >/dev/null
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
launch_app() { # <open-subject> <log-prefix> [arguments...]
  local open_subject="$1"
  local log_prefix="$2"
  shift 2
  sdk_exec \
    --no-a11y-bus \
    --env=DISPLAY="$DISPLAY" \
    --env=AT_SPI_BUS_ADDRESS="$AT_SPI_BUS_ADDRESS" \
    --env=GTK_A11Y=atspi \
    --env=GSK_RENDERER=cairo \
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
# The `gone` wait is what makes any of it a barrier rather than a hope.
stop_app() { # [bus-barrier-timeout: 0 to skip, for the teardown path]
  [[ -n "${APP_PID:-}" ]] || return 0
  local instance
  while read -r instance; do
    [[ -z "$instance" ]] || flatpak kill "$instance" >/dev/null 2>&1 || true
  done < <(flatpak ps 2>/dev/null | awk '$NF == "org.gnome.Sdk" { print $1 }')
  kill -TERM "$APP_PID" 2>/dev/null || true
  # Skipped on the way out: teardown has no next launch to protect, and a failing run should not
  # spend the barrier's timeout before printing where its artifacts are.
  [[ "${1:-45}" == 0 ]] || "$PYTHON" "$ATSPI" gone --timeout "${1:-45}"
  for _ in {1..40}; do
    kill -0 "$APP_PID" 2>/dev/null || break
    sleep 0.1
  done
  kill -KILL "$APP_PID" 2>/dev/null || true
  wait "$APP_PID" 2>/dev/null || true
  APP_PID=""
}

# Relaunch on a different fixture account. Ordinary message navigation stays in one process and
# uses the row's AT-SPI action; only the cross-account contacts leg needs this helper.
reopen_app_on() { # <subject> <log-prefix>
  stop_app
  launch_app "$1" "$ARTIFACT_DIR/$2"
  "$PYTHON" "$ATSPI" wait --name "Message list" --role list --timeout 60
}

open_mail_message() { # <subject>
  "$PYTHON" "$ATSPI" activate --name "Mail" --timeout 20
  "$PYTHON" "$ATSPI" set-text --name "Search mail" --text "$1" --timeout 20
  "$PYTHON" "$ATSPI" wait \
    --name "$1" --role "list item" --enabled --showing --timeout 45
  "$PYTHON" "$ATSPI" activate \
    --name "$1" --role "push button" \
    --within "$1" --within-role "list item" --timeout 20
}

open_calendar_event() { # <title>
  # A saved event replaces its agenda row once the core snapshot arrives. AT-SPI can resolve the
  # outgoing button just before that replacement, so retry against the settled row until its
  # details popover is observable.
  for _ in {1..3}; do
    "$PYTHON" "$ATSPI" activate --name "$1" --role "push button" --timeout 20
    if "$PYTHON" "$ATSPI" wait --name "Edit" --enabled --showing --timeout 5; then
      return
    fi
  done
  die "calendar event details did not open"
}

mcp_request() { # <json-request> <response-variable>
  local request="$1" destination="$2" response
  printf '%s\n' "$request" >&"$MCP_INPUT_FD"
  IFS= read -r -t 20 -u "$MCP_OUTPUT_FD" response ||
    die "the Linux MCP relay did not answer within 20 seconds"
  printf -v "$destination" '%s' "$response"
}

forward_mailto() { # <uri>
  local -a existing_instances=()
  mapfile -t existing_instances < <(
    flatpak ps 2>/dev/null | awk '$NF == "org.gnome.Sdk" { print $1 }'
  )
  local log="$XDG_DATA_HOME/mailcal/mailcal.log" before launcher_pid received=0
  before="$(grep -c 'mail link received' "$log" 2>/dev/null || true)"
  sdk_exec \
    --no-a11y-bus \
    --env=DISPLAY="$DISPLAY" \
    --env=AT_SPI_BUS_ADDRESS="$AT_SPI_BUS_ADDRESS" \
    --env=GTK_A11Y=atspi \
    --env=LANG=C.UTF-8 \
    --env=XDG_DATA_HOME="$XDG_DATA_HOME" \
    --env=XDG_CONFIG_HOME="$XDG_CONFIG_HOME" \
    --env=XDG_CACHE_HOME="$XDG_CACHE_HOME" \
    "$(sdk_target_dir)/debug/mailcal-linux" "$1" \
    >"$ARTIFACT_DIR/mailto.stdout.log" 2>"$ARTIFACT_DIR/mailto.stderr.log" &
  launcher_pid=$!
  for _ in {1..200}; do
    if (( $(grep -c 'mail link received' "$log" 2>/dev/null || true) > before )); then
      received=1
      break
    fi
    sleep 0.05
  done

  # The development launch runs the binary as the command of an org.gnome.Sdk sandbox. Its portal
  # wrapper can outlive the secondary process after GApplication has forwarded the command line;
  # stop only that new SDK instance, never the primary app's instance.
  local instance known
  while read -r instance; do
    known=0
    for existing in "${existing_instances[@]}"; do
      [[ "$instance" == "$existing" ]] && known=1
    done
    (( known == 1 )) || flatpak kill "$instance" >/dev/null 2>&1 || true
  done < <(flatpak ps 2>/dev/null | awk '$NF == "org.gnome.Sdk" { print $1 }')
  kill -TERM "$launcher_pid" 2>/dev/null || true
  wait "$launcher_pid" 2>/dev/null || true
  (( received == 1 )) || die "running application did not receive the mail link"
}

run_inside_session() {
  mapfile -t bus < <(
    dbus-daemon --config-file="$REPO_ROOT/scripts/dev/atspi-test-bus.conf" \
      --fork --print-address=1 --print-pid=1
  )
  [[ ${#bus[@]} == 2 ]] || die "private AT-SPI bus did not report its address and PID"
  export AT_SPI_BUS_ADDRESS="${bus[0]}"
  ATSPI_BUS_PID="${bus[1]}"
  /usr/libexec/at-spi2-registryd --dbus-name=org.a11y.atspi.Registry \
    >"$ARTIFACT_DIR/atspi-registry.log" 2>&1 &
  ATSPI_REGISTRY_PID=$!
  registry_ready=0
  for _ in {1..100}; do
    if gdbus call --address "$AT_SPI_BUS_ADDRESS" \
      --dest org.freedesktop.DBus \
      --object-path /org/freedesktop/DBus \
      --method org.freedesktop.DBus.NameHasOwner org.a11y.atspi.Registry 2>/dev/null |
      grep -q true; then
      registry_ready=1
      break
    fi
    kill -0 "$ATSPI_REGISTRY_PID" 2>/dev/null || break
    sleep 0.05
  done
  [[ "$registry_ready" == 1 ]] || die "private AT-SPI registry did not become ready"

  local portal_capture="$ARTIFACT_DIR/notifications.jsonl"
  "$PYTHON" "$REPO_ROOT/scripts/dev/linux_notification_portal.py" "$portal_capture" \
    >"$ARTIFACT_DIR/notification-portal.log" 2>&1 &
  PORTAL_PID=$!
  portal_ready=0
  for _ in {1..100}; do
    if gdbus call --session \
      --dest org.freedesktop.DBus \
      --object-path /org/freedesktop/DBus \
      --method org.freedesktop.DBus.NameHasOwner org.freedesktop.portal.Desktop 2>/dev/null |
      grep -q true; then
      portal_ready=1
      break
    fi
    kill -0 "$PORTAL_PID" 2>/dev/null || break
    sleep 0.05
  done
  [[ "$portal_ready" == 1 ]] || die "notification portal fixture did not become ready"

  launch_app "${MAILCAL_OPEN_SUBJECT:-}" "$ARTIFACT_DIR/app"

  cleanup() {
    local status=$?
    trap - EXIT INT TERM
    set +e
    if [[ -n "${MCP_RELAY_PID:-}" ]]; then
      kill -TERM "$MCP_RELAY_PID" 2>/dev/null || true
      wait "$MCP_RELAY_PID" 2>/dev/null || true
      MCP_RELAY_PID=""
    fi
    if [[ $status -ne 0 ]]; then
      "$PYTHON" "$ATSPI" dump --timeout 2 >"$ARTIFACT_DIR/accessibility-failure.txt" 2>&1
      capture failure
    fi
    stop_app 0
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
  trap cleanup EXIT INT TERM

  "$PYTHON" "$ATSPI" wait --name "Welcome to Allodia Mail & Calendar" --showing --timeout 60
  "$PYTHON" "$ATSPI" activate \
    --name "Share usage statistics" --role switch --timeout 20
  "$PYTHON" "$ATSPI" activate --name "See exactly what we send" --timeout 20
  "$PYTHON" "$ATSPI" wait \
    --name "See exactly what we send" --role frame --showing --timeout 20
  "$PYTHON" "$ATSPI" activate \
    --name "Close" --within "See exactly what we send" --within-role frame --timeout 20
  "$PYTHON" "$ATSPI" activate --name "Get started" --timeout 20
  "$PYTHON" "$ATSPI" wait --name "Welcome to Allodia Mail & Calendar" --absent --timeout 20

  local core_preferences="$XDG_DATA_HOME/mailcal/dev/preferences.toml"
  for _ in {1..100}; do
    [[ -f "$core_preferences" ]] && grep -q '^analytics_consent = true$' "$core_preferences" && break
    sleep 0.05
  done
  grep -q '^analytics_consent = true$' "$core_preferences" ||
    die "analytics consent was not persisted"
  grep -q '^analytics_install_id = ' "$core_preferences" ||
    die "analytics consent minted no install id"

  "$PYTHON" "$ATSPI" wait --name "Time zone changed" --role frame --showing --timeout 20
  "$PYTHON" "$ATSPI" activate \
    --name-substring "Keep " --role "push button" --timeout 20
  "$PYTHON" "$ATSPI" wait --name "Time zone changed" --absent --timeout 20
  MAILCAL_FAKE_DEVICE_TIMEZONE=""

  "$PYTHON" "$ATSPI" wait --name "Message list" --role list --timeout 60
  # Folder and message rows expose a native primary-action button. Invoking it over AT-SPI, not
  # Return or a pointer coordinate, is the accessibility contract this run qualifies.
  local archive_measure inbox_measure archive_switch_ms inbox_switch_ms
  archive_measure="$("$PYTHON" "$ATSPI" measure \
    --name "Archive" --role "push button" \
    --within "Archive" --within-role "list item" \
    --until-name "$REMOTE_SUBJECT" --until-role "list item" --until-absent --timeout 30)"
  printf '%s\n' "$archive_measure"
  archive_switch_ms="$(printf '%s\n' "$archive_measure" | sed -n 's/^elapsed_ms=//p')"
  inbox_measure="$("$PYTHON" "$ATSPI" measure \
    --name "All Inboxes" --role "push button" \
    --within "All Inboxes" --within-role "list item" \
    --until-role "list item" --until-within "Message list" --until-within-role list \
    --until-showing --timeout 30)"
  printf '%s\n' "$inbox_measure"
  inbox_switch_ms="$(printf '%s\n' "$inbox_measure" | sed -n 's/^elapsed_ms=//p')"
  printf 'archive_ms=%s\nall_inboxes_ms=%s\n' \
    "$archive_switch_ms" "$inbox_switch_ms" >"$ARTIFACT_DIR/folder-switch-timing.txt"
  "$PYTHON" "$ATSPI" wait \
    --name "Downloading 1,200 of 3,387…" --showing --timeout 20
  capture essential-feedback

  local calendar_refreshes=0
  for _ in {1..100}; do
    calendar_refreshes="$(grep -c 'refresh_calendar:' "$XDG_DATA_HOME/mailcal/mailcal.log" 2>/dev/null || true)"
    (( calendar_refreshes >= 2 )) && break
    sleep 0.1
  done
  (( calendar_refreshes >= 2 )) || die "foreground calendar timer did not refresh twice"

  # Settings is one reachable taxonomy, not a collection of launch-only screens. Exercise the
  # privacy lifecycle and the complete diagnostic-log path before returning to mail.
  "$PYTHON" "$ATSPI" activate --name "Settings" --timeout 20
  "$PYTHON" "$ATSPI" activate --name "Privacy" --role "toggle button" --timeout 20
  "$PYTHON" "$ATSPI" wait --name "Usage statistics" --showing --timeout 20
  "$PYTHON" "$ATSPI" activate \
    --name "Share usage statistics" --role switch --timeout 20
  for _ in {1..100}; do
    grep -q '^analytics_consent = false$' "$core_preferences" && break
    sleep 0.05
  done
  grep -q '^analytics_consent = false$' "$core_preferences" ||
    die "analytics withdrawal was not persisted"
  grep -q '^analytics_install_id = ' "$core_preferences" &&
    die "analytics withdrawal kept the install id"

  "$PYTHON" "$ATSPI" activate --name "Diagnostics" --role "toggle button" --timeout 20
  "$PYTHON" "$ATSPI" wait --name "Log size" --showing --timeout 20
  "$PYTHON" "$ATSPI" activate --name "View log" --role "push button" --timeout 20
  "$PYTHON" "$ATSPI" wait --name "Diagnostic log" --role frame --showing --timeout 20
  "$PYTHON" "$ATSPI" wait --name "Jump to end" --role "push button" --showing --timeout 20
  "$PYTHON" "$ATSPI" activate \
    --name "Close" --within "Diagnostic log" --within-role frame --timeout 20
  "$PYTHON" "$ATSPI" activate --name "Copy path" --role "push button" --timeout 20
  "$PYTHON" "$ATSPI" activate --name "Export log…" --role "push button" --timeout 20
  "$PYTHON" "$ATSPI" wait --name "Share diagnostic log?" --role frame --showing --timeout 20
  "$PYTHON" "$ATSPI" wait \
    --name-substring "never contains message content" --showing --timeout 20
  "$PYTHON" "$ATSPI" activate \
    --name "Export log…" --within "Share diagnostic log?" --within-role frame --timeout 20
  for _ in {1..100}; do
    [[ -s "$ARTIFACT_DIR/exported-mailcal.log" ]] && break
    sleep 0.05
  done
  [[ -s "$ARTIFACT_DIR/exported-mailcal.log" ]] || die "diagnostic log export wrote no file"
  "$PYTHON" "$ATSPI" activate --name "Include more detail" --role switch --timeout 20
  for _ in {1..100}; do
    grep -q '"diagnostics_debug": true' "$XDG_CONFIG_HOME/mailcal/host.json" 2>/dev/null && break
    sleep 0.05
  done
  grep -q '"diagnostics_debug": true' "$XDG_CONFIG_HOME/mailcal/host.json" ||
    die "diagnostic detail setting was not persisted"

  # MCP: grant each layer separately through Settings, then drive the shipped relay over its real
  # Unix socket. Four calls in one session catch a relay that answers exactly once, while the tool
  # listing proves direct send stays absent until its separate toggle is granted. The final call
  # crosses back onto GTK's main thread and opens the ordinary composer, unsent.
  "$PYTHON" "$ATSPI" activate --name "Advanced" --role "toggle button" --timeout 20
  "$PYTHON" "$ATSPI" wait --name "AI assistant access" --showing --timeout 20
  "$PYTHON" "$ATSPI" wait --name "Off" --showing --timeout 20
  "$PYTHON" "$ATSPI" activate \
    --name "Allow assistants to use my mail" --role switch --timeout 20
  "$PYTHON" "$ATSPI" wait \
    --name "Running · waiting for a client to connect" --showing --timeout 20
  "$PYTHON" "$ATSPI" wait \
    --name "No accounts are shared yet, so an assistant sees an empty mailbox." \
    --showing --timeout 20
  "$PYTHON" "$ATSPI" activate --name "alice@test.local" --role switch --timeout 20
  "$PYTHON" "$ATSPI" wait \
    --name "No accounts are shared yet, so an assistant sees an empty mailbox." \
    --absent --timeout 20
  # Watch before activating: this confirmation is deliberately transient, and starting a second
  # AT-SPI process after the click can spend its whole lifetime walking a large mailbox tree.
  "$PYTHON" "$ATSPI" wait \
    --name "Copied" --role "push button" --showing --timeout 20 &
  local copied_wait_pid=$!
  "$PYTHON" "$ATSPI" activate --name "Copy configuration" --role "push button" --timeout 20
  wait "$copied_wait_pid"
  local mcp_config="$ARTIFACT_DIR/mcp-config.json"
  "$PYTHON" "$ATSPI" read-text \
    --role text --within "Connect your assistant" --within-role grouping \
    --showing --timeout 20 >"$mcp_config"
  "$PYTHON" - "$mcp_config" "$(sdk_runtime_version)" \
    "$(sdk_target_dir)/debug/allodia-mcp" "$XDG_DATA_HOME/mailcal/mcp.sock" <<'PY'
import json
import os
import sys

config_path, version, relay, endpoint = sys.argv[1:]
with open(config_path, encoding="utf-8") as source:
    entry = json.load(source)["mcpServers"]["allodia-mail-and-calendar"]
assert entry["command"] == "flatpak"
args = entry["args"]
assert args[1] in ("--user", "--system")
assert args[:1] + args[2:] == [
    "run",
    "--devel",
    "--filesystem=host",
    "--filesystem=/tmp",
    "--no-a11y-bus",
    f"--command={relay}",
    f"org.gnome.Sdk/{os.uname().machine}/{version}",
    "--endpoint",
    endpoint,
]
PY
  capture mcp-settings
  "$PYTHON" "$ATSPI" activate --name "Done" --timeout 20

  local mcp_endpoint="$XDG_DATA_HOME/mailcal/mcp.sock"
  for _ in {1..100}; do
    [[ -S "$mcp_endpoint" ]] && break
    sleep 0.05
  done
  [[ -S "$mcp_endpoint" ]] || die "enabling MCP did not bind its Unix socket"
  coproc MCP_RELAY {
    env XDG_DATA_HOME="$HOME/.local/share" "$PYTHON" -c '
import json
import os
import sys

with open(sys.argv[1], encoding="utf-8") as source:
    entry = json.load(source)["mcpServers"]["allodia-mail-and-calendar"]
os.execvp(entry["command"], [entry["command"], *entry["args"]])
' "$mcp_config" \
      2>"$ARTIFACT_DIR/mcp-relay.stderr.log"
  }
  MCP_RELAY_PID=$!
  MCP_OUTPUT_FD="${MCP_RELAY[0]}"
  MCP_INPUT_FD="${MCP_RELAY[1]}"
  local mcp_init mcp_tools mcp_accounts mcp_draft
  mcp_request \
    '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-11-25"}}' \
    mcp_init
  printf '%s\n' '{"jsonrpc":"2.0","method":"notifications/initialized"}' >&"$MCP_INPUT_FD"
  mcp_request '{"jsonrpc":"2.0","id":2,"method":"tools/list","params":{}}' mcp_tools
  mcp_request \
    '{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"list_accounts","arguments":{}}}' \
    mcp_accounts
  mcp_request \
    '{"jsonrpc":"2.0","id":4,"method":"tools/call","params":{"name":"create_draft","arguments":{"to":["mcp-recipient@example.test"],"subject":"Assistant draft","body_text":"Prepared through the local MCP server."}}}' \
    mcp_draft
  printf '%s\n' "$mcp_init" "$mcp_tools" "$mcp_accounts" "$mcp_draft" \
    >"$ARTIFACT_DIR/mcp-session.jsonl"
  "$PYTHON" - "$ARTIFACT_DIR/mcp-session.jsonl" <<'PY'
import json
import sys

responses = [json.loads(line) for line in open(sys.argv[1], encoding="utf-8")]
assert [response["id"] for response in responses] == [1, 2, 3, 4]
assert responses[0]["result"]["serverInfo"]["name"] == "allodia-mail-and-calendar"
tools = [tool["name"] for tool in responses[1]["result"]["tools"]]
assert "list_accounts" in tools
assert "create_draft" in tools
assert "send_message" not in tools
accounts = responses[2]["result"]["structuredContent"]["accounts"]
assert len(accounts) == 1
assert accounts[0]["address"] == "alice@test.local"
assert responses[3]["result"]["isError"] is False
assert "not sent" in responses[3]["result"]["structuredContent"]["outcome"]
PY
  "$PYTHON" "$ATSPI" wait \
    --name "mcp-recipient@example.test: Remove recipient" --role "push button" \
    --enabled --showing --timeout 30
  "$PYTHON" "$ATSPI" wait --name "Write your message" --showing --timeout 30
  capture mcp-draft
  "$PYTHON" "$ATSPI" activate --name "Cancel" --timeout 20
  exec {MCP_INPUT_FD}>&-
  wait "$MCP_RELAY_PID"
  exec {MCP_OUTPUT_FD}<&-
  MCP_RELAY_PID=""
  MCP_INPUT_FD=""
  MCP_OUTPUT_FD=""

  # New-mail notifications, enabled half first, and the order is the point. The disabled half
  # asserts an **absence**, which proves nothing on its own: a notification path that never fires
  # passes it silently, and did, for as long as the portal call hung. So the enabled half runs
  # first, on the shipped default, and its record in the capture file is the anchor the absence
  # below is measured against.
  #
  # Twice, because the shape this leg exists to catch is a portal call that works **exactly
  # once**: the process shares one connection, and a caller that owns the runtime it was opened on
  # takes it down on the way out, so the first notification lands and every later one hangs for
  # the life of the process (docs/client-traps.md). A single delivery reads as a pass over that.
  local notification_on="Linux notification on $RANDOM-$RANDOM"
  local notification_again="Linux notification again $RANDOM-$RANDOM"
  local subject
  for subject in "$notification_on" "$notification_again"; do
    "$REPO_ROOT/scripts/dev/harness.sh" deliver --subject "$subject" >/dev/null
    "$PYTHON" "$ATSPI" activate --name "Refresh" --role "push button" --timeout 20
    "$PYTHON" "$ATSPI" wait --name "$subject" --role "list item" --showing --timeout 60
    for _ in {1..200}; do
      grep -Fq "$subject" "$portal_capture" 2>/dev/null && break
      sleep 0.05
    done
    grep -Fq "$subject" "$portal_capture" ||
      die "an enabled new-mail notification never reached the desktop portal: $subject"
  done

  "$PYTHON" "$ATSPI" activate --name "Settings" --timeout 20
  "$PYTHON" "$ATSPI" activate --name "Advanced" --role "toggle button" --timeout 20
  "$PYTHON" "$ATSPI" activate \
    --name "Allow assistants to use my mail" --role switch --timeout 20
  "$PYTHON" "$ATSPI" wait --name "Off" --showing --timeout 20
  for _ in {1..100}; do
    [[ ! -e "$mcp_endpoint" ]] && break
    sleep 0.05
  done
  [[ ! -e "$mcp_endpoint" ]] || die "turning MCP off left its socket behind"
  "$PYTHON" "$ATSPI" activate --name "Notifications" --role "toggle button" --timeout 20
  "$PYTHON" "$ATSPI" activate --name "New-mail notifications" --role switch --timeout 20
  # Read the switch back off disk before trusting the leg below: an activation that missed leaves
  # notifications on, and every assertion after it would then be testing the wrong state.
  for _ in {1..100}; do
    grep -q '"notifications_enabled": false' "$XDG_CONFIG_HOME/mailcal/host.json" 2>/dev/null &&
      break
    sleep 0.05
  done
  grep -q '"notifications_enabled": false' "$XDG_CONFIG_HOME/mailcal/host.json" ||
    die "turning new-mail notifications off was not persisted"
  "$PYTHON" "$ATSPI" activate --name "Done" --timeout 20
  local notification_off="Linux notification off $RANDOM-$RANDOM"
  "$REPO_ROOT/scripts/dev/harness.sh" deliver --subject "$notification_off" >/dev/null
  "$PYTHON" "$ATSPI" activate --name "Refresh" --role "push button" --timeout 20
  "$PYTHON" "$ATSPI" wait \
    --name "$notification_off" --role "list item" --showing --timeout 60
  sleep 1
  grep -Fq "$notification_off" "$portal_capture" 2>/dev/null &&
    die "a notification crossed the portal while notifications were disabled"
  grep -Fq "$notification_again" "$portal_capture" ||
    die "the notification capture lost its anchor, so the silence above proves nothing"
  capture settings-feedback

  "$PYTHON" "$ATSPI" wait --name "Reply" --enabled --showing --timeout 30
  "$PYTHON" "$ATSPI" wait \
    --name "Remote images are blocked to protect your privacy." --showing --timeout 30
  capture reading-blocked

  "$PYTHON" "$ATSPI" activate --name "Load images" --timeout 20
  "$PYTHON" "$ATSPI" wait --name "Load images" --absent --timeout 20
  capture reading-opted-in
  "$PYTHON" "$ATSPI" activate --name "Reply" --timeout 20
  # This static name is replaced with the localised value by setComposerLabels. Seeing its
  # editable node proves that WebKit exposed the loaded editor through AT-SPI.
  "$PYTHON" "$ATSPI" wait --name "Write your message" --showing --timeout 30
  capture reply

  # A reply's derived recipient is *finished*, so it is a pill with its own remove control and
  # nothing is left half-typed in the input; the failure this catches is a field that looks like
  # it dropped the person it is in fact holding. The address is the fixture's own sender, and the
  # remove control names it rather than repeating a bare "Remove".
  "$PYTHON" "$ATSPI" wait \
    --name "$REMOTE_SENDER: Remove recipient" --role "push button" \
    --enabled --showing --timeout 20
  capture reply-recipients

  # Start observing before the action for the same reason as the Copy confirmation above: the
  # core owns a 2.5-second terminal status, so a post-click process launch can miss a real banner.
  "$PYTHON" "$ATSPI" wait --name "Message sent" --showing --timeout 45 &
  local sent_wait_pid=$!
  "$PYTHON" "$ATSPI" activate --name "Send" --timeout 20
  wait "$sent_wait_pid"
  capture sent

  # Search. What is worth driving here is the half a screenshot cannot check: that the core's
  # narrowing reaches the screen, that the two controls docs/search.md requires of a client are on
  # the accessibility bus at all, and that clearing the field puts the unsearched list back.
  #
  # The query is a thread, which is also the assertion that these are search results: the list is
  # threaded, so "Project kickoff" is folded into its reply's row everywhere except here, where
  # results are flat. The remote-image fixture matches nothing in it and must leave.
  "$PYTHON" "$ATSPI" set-text --name "Search mail" --text "kickoff" --timeout 20
  "$PYTHON" "$ATSPI" wait \
    --name "Project kickoff" --role "list item" --enabled --showing --timeout 30
  "$PYTHON" "$ATSPI" wait --name "$REMOTE_SUBJECT" --role "list item" --absent --timeout 30
  # Rule 8: how far back it looked; the harness account's depth is the shipping default; and the
  # route to the setting that decides it. A statement the user cannot act on is half the value.
  "$PYTHON" "$ATSPI" wait --name "Searching the last 3 months" --showing --timeout 20
  "$PYTHON" "$ATSPI" wait --name "Change" --role "push button" --enabled --showing --timeout 20
  # Rule 4: the two-way filter, its narrowing side naming the view the search was opened from,
  # the unified inbox here, which is every account's Inbox rather than any one folder.
  "$PYTHON" "$ATSPI" wait \
    --name "All mail" --role "toggle button" --enabled --showing --timeout 20
  "$PYTHON" "$ATSPI" activate --name "Inboxes" --role "toggle button" --timeout 20
  "$PYTHON" "$ATSPI" wait \
    --name "Project kickoff" --role "list item" --enabled --showing --timeout 30
  capture search-results

  # Rules 5 and 6: clearing the field leaves search; the folder view returns, threaded again,
  # and takes the filter with it, so no narrowing is left claiming to be in force.
  "$PYTHON" "$ATSPI" set-text --name "Search mail" --text "" --timeout 20
  "$PYTHON" "$ATSPI" wait \
    --name "$REMOTE_SUBJECT" --role "list item" --enabled --showing --timeout 30
  "$PYTHON" "$ATSPI" wait --name "All mail" --role "toggle button" --absent --timeout 20
  capture search-cleared

  # Calendar: the same process changes to the deterministic living-week calendar. The agenda's
  # event buttons and every editor control are driven through AT-SPI; no coordinates or key events.
  "$PYTHON" "$ATSPI" activate --name "Calendar" --timeout 20
  "$PYTHON" "$ATSPI" wait --name "New Event" --enabled --showing --timeout 45
  # The seeded NEEDS-ACTION hold, on the surface with no border to dash: an agenda row prints the
  # disclosure instead, and says it too (docs/calendar.md §4; the picture is per surface, the
  # spoken label is not).
  "$PYTHON" "$ATSPI" wait \
    --name-substring "$AWAITING_LABEL" --showing --timeout 45
  capture calendar-agenda

  # Calendar management is one round trip through the same cache the agenda draws. Toggle the
  # calendar away and back, apply a palette colour, reset it, then reach the default-calendar
  # picker through the shared Settings taxonomy.
  "$PYTHON" "$ATSPI" activate --name "Manage calendars" --role "push button" --timeout 20
  "$PYTHON" "$ATSPI" wait --name "Manage calendars" --role frame --showing --timeout 20
  "$PYTHON" "$ATSPI" wait --name "alice@test.local" --showing --timeout 20
  "$PYTHON" "$ATSPI" activate \
    --role switch --within "Manage calendars" --within-role frame --timeout 20
  "$PYTHON" "$ATSPI" activate \
    --role switch --within "Manage calendars" --within-role frame --timeout 20
  "$PYTHON" "$ATSPI" activate \
    --name-substring "Choose a colour for" --role "toggle button" \
    --within "Manage calendars" --within-role frame --timeout 20
  "$PYTHON" "$ATSPI" activate --name "#4f5ba6" --role "push button" --timeout 20
  for _ in {1..100}; do
    grep -Fq '#4f5ba6' "$core_preferences" && break
    sleep 0.05
  done
  grep -Fq '#4f5ba6' "$core_preferences" || die "calendar colour was not persisted"
  "$PYTHON" "$ATSPI" activate \
    --name "Use the server's colour" --role "push button" --timeout 20
  "$PYTHON" "$ATSPI" activate \
    --name "Done" --within "Manage calendars" --within-role frame --timeout 20

  "$PYTHON" "$ATSPI" activate --name "Settings" --timeout 20
  "$PYTHON" "$ATSPI" activate \
    --name "Calendar" --role "toggle button" \
    --within "Settings" --within-role frame --timeout 20
  "$PYTHON" "$ATSPI" wait --name "Default calendar" --showing --timeout 20
  "$PYTHON" "$ATSPI" wait --role "check box" --showing --timeout 20
  "$PYTHON" "$ATSPI" activate \
    --name "Done" --within "Settings" --within-role frame --timeout 20
  capture calendar-management

  "$PYTHON" "$ATSPI" activate --name "New Event" --timeout 20
  "$PYTHON" "$ATSPI" wait --name "Title" --enabled --showing --timeout 20
  "$PYTHON" "$ATSPI" set-text \
    --name "Title" --text "$CALENDAR_CRUD_TITLE" --timeout 20
  "$PYTHON" "$ATSPI" activate --name "Save" --timeout 20
  "$PYTHON" "$ATSPI" wait \
    --name "$CALENDAR_CRUD_TITLE" --role "push button" --enabled --showing --timeout 45
  capture calendar-created

  open_calendar_event "$CALENDAR_CRUD_TITLE"
  "$PYTHON" "$ATSPI" activate --name "Edit" --timeout 20
  "$PYTHON" "$ATSPI" set-text \
    --name "Title" --text "$CALENDAR_CRUD_TITLE updated" --timeout 20
  "$PYTHON" "$ATSPI" activate --name "Save" --timeout 20
  "$PYTHON" "$ATSPI" wait \
    --name "$CALENDAR_CRUD_TITLE updated" --role "push button" \
    --enabled --showing --timeout 45
  capture calendar-edited

  open_calendar_event "$CALENDAR_CRUD_TITLE updated"
  "$PYTHON" "$ATSPI" activate --name "Delete" --timeout 20
  "$PYTHON" "$ATSPI" wait --name "Delete this event?" --showing --timeout 20
  "$PYTHON" "$ATSPI" activate --name "Delete" --timeout 20
  "$PYTHON" "$ATSPI" wait \
    --name "$CALENDAR_CRUD_TITLE updated" --role "push button" --absent --timeout 45
  capture calendar-deleted

  # Meeting invitations. This leg runs AFTER the calendar leg on purpose: the card withholds its
  # day preview until the calendar has actually been read (`conflicts_known`), and "we have not
  # looked" is a different fact from "nothing else then"; so a run that asked before the agenda
  # had events would assert the cold-start card and never see the rule. The agenda above proves
  # the read happened.
  #
  # Every assertion here is one a screenshot cannot make. A card missing its button row, drawn over
  # an unread calendar, or with a dashed hold nothing says out loud, photographs exactly as well as
  # a correct one.
  open_mail_message "$INVITE_SUBJECT"
  "$PYTHON" "$ATSPI" wait --name "Meeting invitation" --showing --timeout 45
  # The two-condition gate passing, as three buttons that each say what they act on; three bare
  # verbs read out of context tell a screen-reader user nothing about which invitation they belong
  # to (docs/invitations.md). The qualifier is the DESCRIPTION and it has to be: a GtkButton with a
  # label is `labelled-by` that label, and a relation beats an explicit accessible label, so an
  # assertion on the name alone would pass over a button announcing a bare "Accept".
  "$PYTHON" "$ATSPI" wait --name "Accept" --role "push button" \
    --description "Accept this invitation" --enabled --showing --timeout 20
  "$PYTHON" "$ATSPI" wait --name "Maybe" --role "push button" \
    --description "Answer maybe to this invitation" --enabled --showing --timeout 20
  "$PYTHON" "$ATSPI" wait --name "Decline" --role "push button" \
    --description "Decline this invitation" --enabled --showing --timeout 20
  # The iMIP body part is not a file: a card and NO attachment row. The published .ics below is
  # the other half of that contrast.
  "$PYTHON" "$ATSPI" wait --name "Attachments" --absent --timeout 20
  # The count in words, then the picture; docs/calendar.md §4: a grid the user has to read
  # carefully is not a disclosure. The Monday invitation lands in the review/triage overlap.
  "$PYTHON" "$ATSPI" wait --name "Around this meeting" --showing --timeout 20
  # …and the meeting itself, drawn on that preview as a hold and SAYING so. A dashed border is
  # invisible to a screen reader, and GTK exposes no getter for an accessible label; an AT-SPI run
  # is the only oracle there is for this half (AGENTS.md). The preview is a Cairo surface, so
  # without the label overlay it would be one unnamed node and this could not pass.
  "$PYTHON" "$ATSPI" wait \
    --name-substring "$AWAITING_LABEL" --showing --timeout 20
  capture invitation-card

  # The published .ics, and the contrast the seed exists for: same media type, dispositioned as a
  # file, METHOD:PUBLISH, no ATTENDEE. No card, and the chip stays. Attendee-matching alone is not
  # the gate; nobody is waiting on a reply to this one.
  open_mail_message "$PUBLISH_SUBJECT"
  "$PYTHON" "$ATSPI" wait --name "Attachments" --showing --timeout 45
  "$PYTHON" "$ATSPI" wait --name "Meeting invitation" --absent --timeout 20
  capture invitation-published

  # The weekend invitation is the fixture that makes the preview rule able to FAIL. Its day is
  # deliberately empty, so under the rule this replaced ("open only when the count is non-zero")
  # the disclosure would be shut; and a suite that only ever saw the conflicted Monday would pass
  # under both. Whenever the calendar was read, the grid is open.
  open_mail_message "$FREE_DAY_SUBJECT"
  "$PYTHON" "$ATSPI" wait --name "Meeting invitation" --showing --timeout 45
  "$PYTHON" "$ATSPI" wait --name "Around this meeting" --showing --timeout 20
  "$PYTHON" "$ATSPI" wait \
    --name-substring "$AWAITING_LABEL" --showing --timeout 20
  capture invitation-free-day

  # Contacts: the same process again. What is worth driving here is the pair of rules a screenshot
  # cannot check; that the core's narrowing reaches the screen, and that two people who merely
  # share a name stay two rows. The fixtures exist for exactly that (docker/stalwart/seed/contacts).
  local contacts_refreshes_before
  contacts_refreshes_before="$(grep -c 'refresh_contacts:' "$XDG_DATA_HOME/mailcal/mailcal.log" || true)"
  "$PYTHON" "$ATSPI" activate --name "Contacts" --timeout 20
  "$PYTHON" "$ATSPI" wait \
    --name "Ahmed El Amrani" --role "list item" --enabled --showing --timeout 45
  local contacts_refreshes
  for _ in {1..200}; do
    contacts_refreshes="$(grep -c 'refresh_contacts:' "$XDG_DATA_HOME/mailcal/mailcal.log" || true)"
    (( contacts_refreshes > contacts_refreshes_before )) && break
    sleep 0.05
  done
  (( contacts_refreshes > contacts_refreshes_before )) ||
    die "contacts refresh did not settle before search"
  # A section letter is a row of its own, and `#` is the bucket a name starting with a digit files
  # under rather than minting a section of its own.
  "$PYTHON" "$ATSPI" wait --name "#" --role label --showing --timeout 20
  # `--index 1` is the assertion: a *second* row with that name is on screen. The two namesake
  # fixtures share a name and nothing else, and names never join; a merge on name would leave one.
  "$PYTHON" "$ATSPI" wait \
    --name "Jan de Vries" --role "list item" --index 1 --enabled --showing --timeout 20
  capture contacts-list

  local contact_search_set=0
  for _ in {1..3}; do
    "$PYTHON" "$ATSPI" set-text --name "Search contacts" --text "Vermeulen" --timeout 20
    for _ in {1..40}; do
      if grep -q 'rebuild_contacts: .*query_chars=9' "$XDG_DATA_HOME/mailcal/mailcal.log"; then
        contact_search_set=1
        break 2
      fi
      sleep 0.05
    done
  done
  [[ "$contact_search_set" == 1 ]] || die "contact search never reached the core"
  "$PYTHON" "$ATSPI" wait \
    --name "Sofie Vermeulen" --role "list item" --enabled --showing --timeout 30
  "$PYTHON" "$ATSPI" wait --name "Ahmed El Amrani" --role "list item" --absent --timeout 30
  capture contacts-searched

  # Autosuggest, last: it reads the same people index the contacts leg above has just synced, so a
  # seeded contact is a deterministic query. The list arriving at all is the assertion; it is the
  # half a screenshot cannot check, and a popover is in its own surface where a window capture
  # cannot see it either.
  "$PYTHON" "$ATSPI" activate --name "Mail" --timeout 20
  "$PYTHON" "$ATSPI" activate --name "Compose" --timeout 20
  "$PYTHON" "$ATSPI" wait --name "To" --role text --enabled --showing --timeout 30
  # `--role text`: the caption label beside the field carries the same accessible name on purpose
  # (that association is what tells a screen reader To from Bcc), and only the entry takes text.
  "$PYTHON" "$ATSPI" set-text --name "To" --role text --text "sofie" --timeout 20
  "$PYTHON" "$ATSPI" wait --name "Suggested recipients" --role list --showing --timeout 30
  # A *saved contact*, by name: that it is the name rather than the address is what proves the
  # people index reached the dropdown rather than sent-mail history alone. The address rides along
  # as the row's `described-by` relation, which is why no `--description` assertion is possible
  # here: on a row that carries one, the AT-SPI `description` field is empty (`AGENTS.md`).
  "$PYTHON" "$ATSPI" wait \
    --name "Sofie Vermeulen" --role "list item" --showing --timeout 20
  capture compose-autosuggest
  # An empty token closes the list rather than offering everyone the user has ever written to.
  "$PYTHON" "$ATSPI" set-text --name "To" --role text --text "" --timeout 20
  "$PYTHON" "$ATSPI" wait --name "Suggested recipients" --role list --absent --timeout 20
  "$PYTHON" "$ATSPI" activate --name "Cancel" --timeout 20

  # Mail actions. The dispatches themselves are unit-tested; what only a live run can show is that
  # the core answered; the menu offering the opposite verb the next time it opens; and that
  # archiving from the reading pane advances it to the next message instead of emptying it, which
  # is the rule here no screenshot can check (README → "Archive/delete advances the reading pane").
  #
  # The message is delivered for this run and archived by it. Nothing in the seeded mailbox is
  # touched, so the leg is as repeatable as the calendar one above.
  "$REPO_ROOT/scripts/dev/harness.sh" deliver --subject "$MAIL_ACTION_SUBJECT" >/dev/null
  "$PYTHON" "$ATSPI" activate --name "Mail" --timeout 20
  "$PYTHON" "$ATSPI" activate --name "Refresh" --role "push button" --timeout 20
  open_mail_message "$MAIL_ACTION_SUBJECT"
  # This leg asserts that archiving removes the message from the folder it was delivered to.
  # Global search deliberately keeps archived mail, so narrow to the Inbox before the write.
  "$PYTHON" "$ATSPI" activate --name "Inboxes" --role "toggle button" --timeout 20
  "$PYTHON" "$ATSPI" wait \
    --name "$MAIL_ACTION_SUBJECT" --role "list item" --enabled --showing --timeout 60

  # Opening it read it, so the menu on its own row offers the way back rather than "Mark as read"
  #; the round trip, from a dispatch through the core to the snapshot the row is rebuilt from.
  "$PYTHON" "$ATSPI" activate \
    --name "More actions" --within "$MAIL_ACTION_SUBJECT" --timeout 20
  "$PYTHON" "$ATSPI" wait --name "Mark as unread" --enabled --showing --timeout 20
  "$PYTHON" "$ATSPI" wait --name "Mark as read" --absent --timeout 20
  capture mail-actions-menu
  "$PYTHON" "$ATSPI" activate --name "Flag" --timeout 20
  # Wait for the row to carry the flag before reopening its menu: the list reconciles in place, so
  # a menu opened mid-rebuild belongs to the widget being replaced and answers nothing.
  "$PYTHON" "$ATSPI" wait \
    --name "Flagged" --within "$MAIL_ACTION_SUBJECT" --showing --timeout 30
  "$PYTHON" "$ATSPI" activate \
    --name "More actions" --within "$MAIL_ACTION_SUBJECT" --timeout 20
  "$PYTHON" "$ATSPI" wait --name "Unflag" --enabled --showing --timeout 20
  "$PYTHON" "$ATSPI" activate --name "Unflag" --timeout 20
  "$PYTHON" "$ATSPI" wait \
    --name "Flagged" --within "$MAIL_ACTION_SUBJECT" --absent --timeout 30

  # Archive from the pane the message is open in. The row goes, and the pane keeps a message: an
  # empty one would leave "Select a message to read." and no Reply to press, which is exactly the
  # state this rule exists to prevent.
  # By role: the folder pane has an "Archive" row too, and a list item is not what archives a
  # message.
  "$PYTHON" "$ATSPI" activate --name "Archive" --role "push button" --timeout 20
  "$PYTHON" "$ATSPI" wait \
    --name "$MAIL_ACTION_SUBJECT" --role "list item" --absent --timeout 45
  "$PYTHON" "$ATSPI" wait --name "Select a message to read." --absent --timeout 20
  "$PYTHON" "$ATSPI" wait --name "Reply" --enabled --showing --timeout 20
  capture mail-actions-archived

  # Signatures: the Settings category, and the CRUD round-trip through the core. What is worth
  # driving here is the half a screenshot cannot check; that each control carries a name an
  # assistive technology can act on. Both slot pickers read "None" until one is set, so a picker
  # named only by its value says nothing about which slot it is; and the editor's body is a
  # WebKit view, whose document never reaches the accessibility bus, so its host has to carry the
  # name instead. Neither is visible in a picture.
  #
  # The window opens on this category because MAILCAL_SHOWCASE_SCREEN says so; the same accessible
  # category buttons used above can move away from it and back.
  "$PYTHON" "$ATSPI" activate --name "Settings" --timeout 20
  "$PYTHON" "$ATSPI" wait --name "New signature" --role "push button" --enabled --showing --timeout 30
  "$PYTHON" "$ATSPI" wait --name "You haven't written a signature yet." --showing --timeout 20
  "$PYTHON" "$ATSPI" wait \
    --name "For new messages" --role "combo box" --enabled --showing --timeout 20
  "$PYTHON" "$ATSPI" wait \
    --name "For replies or forwards" --role "combo box" --enabled --showing --timeout 20
  capture signatures-empty

  "$PYTHON" "$ATSPI" activate --name "New signature" --timeout 20
  "$PYTHON" "$ATSPI" wait --name "Name" --role text --enabled --showing --timeout 20
  # The editor bundle has parsed and been told what surface it is: this name is the signature
  # placeholder, not the composer's "Write your message", and it is on the frame because WebKit
  # publishes no document of its own here.
  "$PYTHON" "$ATSPI" wait --name "Write your signature" --showing --timeout 30
  capture signature-editor
  # By role as well: the button that opened this editor carries the same name as its window, and
  # a bare "Save" is also what each attachment in the reading pane offers.
  "$PYTHON" "$ATSPI" activate \
    --name "Save" --within "Settings" --within-role frame --timeout 20
  # The row is the core's answer, not the editor's: the library is re-read from the snapshot after
  # the create, so seeing it proves the whole round-trip rather than a widget left on screen.
  "$PYTHON" "$ATSPI" wait --name "New signature" --role "list item" --showing --timeout 20
  capture signature-created

  # The GTK bridge crashed within one to three repetitions while the signature editor was a
  # separate toplevel. Keep AT-SPI walking and invoking controls over twelve edit/save cycles; the
  # editor now changes detail inside the one Settings toplevel.
  for _ in {1..12}; do
    "$PYTHON" "$ATSPI" activate \
      --name "Edit" --within "New signature" --within-role "list item" --timeout 20
    "$PYTHON" "$ATSPI" wait --name "Write your signature" --showing --timeout 30
    "$PYTHON" "$ATSPI" activate \
      --name "Save" --within "Settings" --within-role frame --timeout 20
    "$PYTHON" "$ATSPI" wait --name "Write your signature" --absent --timeout 20
  done

  # A warm desktop activation has to dismiss Settings and route through the already-running
  # GApplication instance. Bcc is deliberately present: a hidden seeded recipient is a security
  # failure, so all three address pills must be visible and removable.
  "$PYTHON" "$ATSPI" activate \
    --name "Edit" --within "New signature" --within-role "list item" --timeout 20
  local mailto_uri="mailto:ada@example.test?cc=copy@example.test&bcc=audit@example.test&subject=Linux%20mail%20link&body=Hello%20from%20a%20mail%20link%0ASecond%20line"
  forward_mailto "$mailto_uri"
  "$PYTHON" "$ATSPI" wait --name "Settings" --role frame --absent --timeout 20
  "$PYTHON" "$ATSPI" wait \
    --name "ada@example.test: Remove recipient" --role "push button" --showing --timeout 30
  "$PYTHON" "$ATSPI" wait \
    --name "copy@example.test: Remove recipient" --role "push button" --showing --timeout 20
  "$PYTHON" "$ATSPI" wait \
    --name "audit@example.test: Remove recipient" --role "push button" --showing --timeout 20
  grep -Fq "$mailto_uri" "$XDG_DATA_HOME/mailcal/mailcal.log" 2>/dev/null &&
    die "mail link content reached the diagnostic log"
  capture mailto-composer
  "$PYTHON" "$ATSPI" activate --name "Cancel" --timeout 20

  # The cross-account merge; the one contacts rule a single-account boot cannot show, and so the
  # last thing this run does: it swaps the dev account, and nothing follows it. `stalwart-multi`
  # connects alice and bob, and the harness files the same card (`shared-*.vcf`) in both books, so
  # one person with one canonical address arrives from two accounts.
  #
  # The assertion is the disclosure, not the dedup: a list showing "Iris Jansen" once is also what
  # a client that silently dropped the second copy would show, so the row has to say it is a merge
  # (docs/contacts.md §2). The detail's "Also in", which names the two accounts, stays on its
  # widget test; a contacts row exposes no AT-SPI action, which is why the leg above reads rows
  # rather than opening anyone.
  MAILCAL_DEV_ACCOUNT=stalwart-multi reopen_app_on "" contacts-merge
  "$PYTHON" "$ATSPI" activate --name "Contacts" --timeout 20
  "$PYTHON" "$ATSPI" wait \
    --name "$MERGED_CONTACT" --role "list item" --enabled --showing --timeout 60
  "$PYTHON" "$ATSPI" wait --name "In 2 accounts" --showing --timeout 30
  "$PYTHON" "$ATSPI" activate --name "Settings" --timeout 20
  "$PYTHON" "$ATSPI" activate \
    --name "Calendar" --role "toggle button" \
    --within "Settings" --within-role frame --timeout 20
  "$PYTHON" "$ATSPI" wait \
    --name "alice@test.local" --role "list item" \
    --within "Settings" --within-role frame --showing --timeout 60
  "$PYTHON" "$ATSPI" wait \
    --name "bob@test.local" --role "list item" \
    --within "Settings" --within-role frame --showing --timeout 60
  "$PYTHON" "$ATSPI" activate \
    --name "Done" --within "Settings" --within-role frame --timeout 20
  capture contacts-merged

  # Cold activation exercises the other half of the desktop contract: the first process receives
  # the raw URI before its window is active and still brings an exact, pre-filled draft forward.
  stop_app
  launch_app "" "$ARTIFACT_DIR/mailto-cold" "$mailto_uri"
  "$PYTHON" "$ATSPI" wait \
    --name "ada@example.test: Remove recipient" --role "push button" --showing --timeout 60
  "$PYTHON" "$ATSPI" wait \
    --name "audit@example.test: Remove recipient" --role "push button" --showing --timeout 20
  capture mailto-cold

  if grep -Eq 'critical from |unhandled error from ' "$XDG_DATA_HOME/mailcal/mailcal.log"; then
    grep -E 'critical from |unhandled error from ' "$XDG_DATA_HOME/mailcal/mailcal.log" \
      >"$ARTIFACT_DIR/toolkit-errors.log"
    die "GTK reported a critical or fatal error; see toolkit-errors.log"
  fi
  "$PYTHON" "$ATSPI" dump >"$ARTIFACT_DIR/accessibility.txt"
}

if [[ "$INSIDE_SESSION" == 1 ]]; then
  run_inside_session
  exit 0
fi

is_linux || die "the Linux UI acceptance test runs only on Linux"
require_cmd cargo
require_cmd curl
require_cmd dbus-daemon
require_cmd dbus-run-session
require_cmd gdbus
require_cmd xvfb-run
require_cmd xdotool
require_cmd xwd
require_cmd convert
[[ -x "$PYTHON" ]] || die "the distro /usr/bin/python3 is required"
[[ -x /usr/libexec/at-spi2-registryd ]] || die "at-spi2-registryd is required"
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
fi
info "toolkit under test: $(sdk_versions)"

if [[ -z "$ARTIFACT_DIR" ]]; then
  ARTIFACT_DIR="$REPO_ROOT/target/ui-test-artifacts/linux/$(date '+%Y%m%d-%H%M%S')"
fi
CALENDAR_CRUD_TITLE="Linux acceptance event $(date '+%s%N')"
MAIL_ACTION_SUBJECT="Linux acceptance action $(date '+%s%N')"
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
trap cleanup_runtime EXIT

info "running semantic Linux UI acceptance in a private Xvfb session"
__EGL_VENDOR_LIBRARY_FILENAMES=/usr/share/glvnd/egl_vendor.d/50_mesa.json \
  __GLX_VENDOR_LIBRARY_NAME=mesa \
  GALLIUM_DRIVER=llvmpipe \
  LIBGL_ALWAYS_SOFTWARE=1 \
  xvfb-run --auto-servernum \
  --server-args="-screen 0 1440x900x24 -nolisten tcp -extension GLX" \
  env \
    ARTIFACT_DIR="$ARTIFACT_DIR" \
    GDK_BACKEND=x11 \
    GDK_DPI_SCALE=1 \
    GDK_SCALE=1 \
    GSK_RENDERER=cairo \
    GTK_A11Y=atspi \
    LANG=C.UTF-8 \
    LIBGL_ALWAYS_SOFTWARE=1 \
    MAILCAL_DEV_ACCOUNT=stalwart \
    MAILCAL_CALENDAR_VIEW=agenda \
    MAILCAL_FAKE_DEVICE_TIMEZONE=Pacific/Auckland \
    MAILCAL_OPEN_SUBJECT="$REMOTE_SUBJECT" \
    MAILCAL_SHOWCASE_SCREEN=signatures \
    REMOTE_SENDER="$REMOTE_SENDER" \
    CALENDAR_CRUD_TITLE="$CALENDAR_CRUD_TITLE" \
    MAIL_ACTION_SUBJECT="$MAIL_ACTION_SUBJECT" \
    MERGED_CONTACT="$MERGED_CONTACT" \
    WEBKIT_DISABLE_DMABUF_RENDERER=1 \
    XDG_CACHE_HOME="$ARTIFACT_DIR/xdg-cache" \
    XDG_CONFIG_HOME="$ARTIFACT_DIR/xdg-config" \
    XDG_DATA_HOME="$SESSION_RUNTIME/data" \
    XDG_RUNTIME_DIR="$SESSION_RUNTIME" \
    dbus-run-session -- "$SELF" --inside-session --artifacts "$ARTIFACT_DIR"

trap - EXIT
cleanup_runtime
info "Linux UI acceptance passed"
info "artifacts: $ARTIFACT_DIR"
