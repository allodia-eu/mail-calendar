#!/usr/bin/env bash
# What the Linux UI suite does outside the app: the harness mailboxes and calendars it reads and
# writes behind the app's back, the MCP relay it speaks to, and the second launch a mail link
# makes. Sourced by test-linux-ui.sh.

# Wait until <address>'s harness mailbox holds a scheduling message (a `text/calendar` part) whose
# subject contains <words>, which is what proves the server told an invitee rather than only storing
# the meeting. Stalwart sends one per invitee for a create, a roster change and a cancellation.
wait_for_scheduling_mail() { # <address> <words>
  "$PYTHON" - "$1" "$2" <<'PY' || die "no scheduling message containing '$2' reached $1"
import base64, json, sys, time, urllib.request

address, words = sys.argv[1], sys.argv[2]
auth = "Basic " + base64.b64encode(
    f"{address}:harness-{address.split('@')[0]}-pw".encode()
).decode()


def fetch(url, body=None):
    request = urllib.request.Request(
        url,
        data=None if body is None else json.dumps(body).encode(),
        headers={"Authorization": auth, "Content-Type": "application/json"},
    )
    return json.load(urllib.request.urlopen(request, timeout=10))


def calendar_parts(part):
    found = "calendar" in (part.get("type") or "")
    return found or any(calendar_parts(child) for child in part.get("subParts") or [])


session = fetch("http://127.0.0.1:28080/jmap/session")
account = session["primaryAccounts"]["urn:ietf:params:jmap:mail"]
deadline = time.monotonic() + 60
while time.monotonic() < deadline:
    mails = fetch(
        "http://127.0.0.1:28080/jmap/",
        {
            "using": ["urn:ietf:params:jmap:core", "urn:ietf:params:jmap:mail"],
            "methodCalls": [
                # Newest first: unsorted, Stalwart answers oldest first, so once a mailbox holds
                # more than the limit, which every run adds to, the message just sent is never in
                # the window.
                [
                    "Email/query",
                    {
                        "accountId": account,
                        "sort": [{"property": "receivedAt", "isAscending": False}],
                        "limit": 50,
                    },
                    "q",
                ],
                [
                    "Email/get",
                    {
                        "accountId": account,
                        "#ids": {"resultOf": "q", "name": "Email/query", "path": "/ids"},
                        "properties": ["subject", "bodyStructure"],
                    },
                    "g",
                ],
            ],
        },
    )["methodResponses"][1][1]["list"]
    if any(words in (mail["subject"] or "") and calendar_parts(mail["bodyStructure"]) for mail in mails):
        sys.exit(0)
    time.sleep(1)
sys.exit(1)
PY
}

# Bob writes a meeting inviting Alice to his own calendar over CalDAV, and the harness's scheduler
# puts it on hers: an invitation that exists only for this run, so answering it leaves the seed as
# it was. Tomorrow, so it is inside the agenda's window wherever the run's clock is.
bob_invites_alice() { # <title>
  local start
  start="$(date -u -d "tomorrow 10:00" '+%Y%m%dT%H%M%SZ')"
  printf 'BEGIN:VCALENDAR\r\nVERSION:2.0\r\nPRODID:-//mailcal//ui-suite//EN\r\nBEGIN:VEVENT\r\nUID:%s\r\nDTSTAMP:%s\r\nDTSTART:%s\r\nDURATION:PT1H\r\nSUMMARY:%s\r\nORGANIZER;CN=Bob Tester:mailto:bob@test.local\r\nATTENDEE;CN=Bob Tester;ROLE=CHAIR;PARTSTAT=ACCEPTED:mailto:bob@test.local\r\nATTENDEE;CN=Alice Tester;ROLE=REQ-PARTICIPANT;PARTSTAT=NEEDS-ACTION;RSVP=TRUE:mailto:alice@test.local\r\nEND:VEVENT\r\nEND:VCALENDAR\r\n' \
    "$(answer_uid)" "$(date -u '+%Y%m%dT%H%M%SZ')" "$start" "$1" |
    curl -sf -o /dev/null -u bob@test.local:harness-bob-pw -X PUT \
      -H 'Content-Type: text/calendar' --data-binary @- \
      "http://127.0.0.1:28080/dav/cal/bob@test.local/default/$(answer_uid).ics" ||
    die "Bob could not write the meeting he invites Alice to"
}

# Bob calls the meeting off again, which takes it out of Alice's calendar too.
bob_withdraws() {
  curl -s -o /dev/null -u bob@test.local:harness-bob-pw -X DELETE \
    "http://127.0.0.1:28080/dav/cal/bob@test.local/default/$(answer_uid).ics" || true
}

answer_uid() {
  printf '%s@test.local' "$(printf '%s' "$ANSWER_TITLE" | tr -c 'A-Za-z0-9' '-')"
}

# The configuration Settings offers to copy names this build's relay, run the way the Flatpak runs.
mcp_config_is_the_relay() { # <config.json>
  "$PYTHON" - "$1" "$(sdk_runtime_version)" \
    "$(sdk_target_dir)/debug/mailcal-mcp" "$XDG_DATA_HOME/$MAILCAL_DATA_DIR_NAME/mcp.sock" <<'PY'
import json
import os
import sys

config_path, version, relay, endpoint = sys.argv[1:]
with open(config_path, encoding="utf-8") as source:
    entry = json.load(source)["mcpServers"][os.environ["MAILCAL_MCP_SERVER_NAME"]]
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
}

# Start the relay <config.json> names and make four calls over it, the last creating a draft.
mcp_session() { # <config.json>
  coproc MCP_RELAY {
    env XDG_DATA_HOME="$HOME/.local/share" "$PYTHON" -c '
import json
import os
import sys

with open(sys.argv[1], encoding="utf-8") as source:
    entry = json.load(source)["mcpServers"][os.environ["MAILCAL_MCP_SERVER_NAME"]]
os.execvp(entry["command"], [entry["command"], *entry["args"]])
' "$1" \
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
import os
import sys

responses = [json.loads(line) for line in open(sys.argv[1], encoding="utf-8")]
assert [response["id"] for response in responses] == [1, 2, 3, 4]
assert responses[0]["result"]["serverInfo"]["name"] == os.environ["MAILCAL_MCP_SERVER_NAME"]
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
}

mcp_session_end() {
  exec {MCP_INPUT_FD}>&-
  wait "$MCP_RELAY_PID"
  exec {MCP_OUTPUT_FD}<&-
  MCP_RELAY_PID=""
  MCP_INPUT_FD=""
  MCP_OUTPUT_FD=""
}

mcp_request() { # <json-request> <response-variable>
  local request="$1" destination="$2" response
  printf '%s\n' "$request" >&"$MCP_INPUT_FD"
  IFS= read -r -t 20 -u "$MCP_OUTPUT_FD" response ||
    die "the Linux MCP relay did not answer within 20 seconds"
  printf -v "$destination" '%s' "$response"
}

# Hand <uri> to the running app the way a desktop does: a second launch, which finds the first on
# the session bus and passes its command line over.
#
# The launch is given the test agent's socket, as one from a shell `build-and-run.sh` set up would
# be; only the primary instance may listen there, which the fresh connection at the end checks.
forward_mailto() { # <uri>
  local -a existing_instances=() agent_args
  mapfile -t existing_instances < <(
    flatpak ps 2>/dev/null | awk '$NF == "org.gnome.Sdk" { print $1 }'
  )
  mapfile -t agent_args < <(linux_agent_sandbox_args)
  local before launcher_pid received=0
  before="$(grep -c 'mail link received' "$APP_LOG" 2>/dev/null || true)"
  sdk_exec \
    --no-a11y-bus \
    "${agent_args[@]}" \
    --env=MAILCAL_TEST_AGENT="$UI_SOCKET_PATH" \
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
    if (( $(grep -c 'mail link received' "$APP_LOG" 2>/dev/null || true) > before )); then
      received=1
      break
    fi
    sleep 0.05
  done

  # The launch hands its command line to the running app and exits on its own, and cleanly. One
  # that lingers is a process left behind for every mail link clicked.
  local exited=0 status=0
  for _ in {1..200}; do
    if ! kill -0 "$launcher_pid" 2>/dev/null; then
      exited=1
      break
    fi
    sleep 0.05
  done
  if (( exited == 1 )); then
    wait "$launcher_pid" 2>/dev/null || status=$?
  else
    # Clean up the new SDK instance only, never the primary app's, before failing.
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
  fi
  (( received == 1 )) || die "running application did not receive the mail link"
  (( exited == 1 )) || die "the launch that handed the mail link over did not exit"
  (( status == 0 )) || die "the launch that handed the mail link over exited $status"
  "$PYTHON" "$REPO_ROOT/scripts/dev/agent.py" --socket "$UI_SOCKET_PATH" ping >/dev/null ||
    die "the launch that handed the mail link over took the running app's test agent socket"
}
