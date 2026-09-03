#!/bin/sh
# Self-bootstrapping entrypoint for the deterministic Stalwart test harness.
#
# Stalwart v0.16 has no declarative config file: a fresh server boots into
# "bootstrap mode" and is configured through its JMAP management API, after
# which it must restart to come up as a full server. This wrapper drives that
# sequence non-interactively and deterministically, then seeds the shared
# dataset, so `docker compose up` yields an identical, ready server every time:
#
#   1. start the server (bootstrap mode if the store is empty),
#   2. complete setup via `x:Bootstrap/set` (no ACME/auto-TLS), restart to full,
#   3. create the test accounts via `x:Account/set` (idempotent),
#   4. seed mail (IMAP over TLS) + calendars (CalDAV over plain HTTP),
#   5. write a readiness marker and run the server in the foreground.
#
# It is idempotent: on a re-run against an already-bootstrapped data volume it
# skips bootstrap and skips existing accounts, and the content seeder clears
# before it appends. See docs/agent-guidance/stalwart-harness.md.
set -eu

CONFIG="${STALWART_CONFIG:-/etc/stalwart/config.json}"
HTTP="http://127.0.0.1:8080"
ADMIN_USER="admin"
ADMIN_PW="${HARNESS_ADMIN_PW:-harness-admin-pw}"
MARKER="/var/lib/stalwart/.harness-ready"

log() { printf '[harness] %s\n' "$1"; }

# One JMAP management call. $1 is the methodCalls array body (without brackets).
jmap() {
  curl -s -u "$ADMIN_USER:$ADMIN_PW" -H 'Content-Type: application/json' \
    -X POST "$HTTP/jmap" \
    -d "{\"using\":[\"urn:ietf:params:jmap:core\",\"urn:stalwart:jmap\"],\"methodCalls\":[$1]}"
}

start_server() {
  stalwart --config "$CONFIG" &
  SRV=$!
}

stop_server() {
  kill "$SRV" 2>/dev/null || true
  wait "$SRV" 2>/dev/null || true
}

wait_http() {
  i=0
  until curl -sf "$HTTP/healthz/live" >/dev/null 2>&1; do
    i=$((i + 1))
    [ "$i" -gt 90 ] && {
      log "server HTTP never became ready"
      return 1
    }
    sleep 1
  done
}

# Bootstrap mode exposes the singleton Bootstrap object; normal mode 404s it.
in_bootstrap_mode() {
  jmap '["x:Bootstrap/get",{"ids":null},"c0"]' | grep -q '"serverHostname"'
}

# Id of the auto-created default domain (the only domain), parsed from the
# single-line JSON. No jq/python in the image, so this stays grep/sed.
domain_id() {
  jmap '["x:Domain/get",{"ids":null,"properties":["name"]},"c0"]' \
    | grep -oE '"id":"[^"]+"' | head -1 | cut -d'"' -f4
}

account_exists() {
  jmap '["x:Account/get",{"ids":null,"properties":["name"]},"c0"]' \
    | grep -q "\"name\":\"$1\""
}

ensure_account() { # local-name  description  password
  if account_exists "$1"; then
    log "account $1 already present"
    return 0
  fi
  resp=$(jmap "[\"x:Account/set\",{\"create\":{\"x\":{\"@type\":\"User\",\"name\":\"$1\",\"domainId\":\"$DOMAIN_ID\",\"description\":\"$2\",\"credentials\":{\"0\":{\"@type\":\"Password\",\"secret\":\"$3\"}},\"roles\":{\"@type\":\"User\"}}}},\"c0\"]")
  if ! printf '%s' "$resp" | grep -q '"created"'; then
    log "FAILED to create account $1: $resp"
    return 1
  fi
  log "created account $1"
}

listener_exists() { # name
  jmap '["x:NetworkListener/get",{"ids":null,"properties":["name"]},"c0"]' \
    | grep -q "\"name\":\"$1\""
}

# Add a STARTTLS listener (`useTls` with `tlsImplicit:false`) if absent. Stalwart supports
# STARTTLS on 143/587 but recommends implicit TLS (993/465), so a fresh bootstrap comes up
# with 993/465 only; we add 143/587 to exercise the engine's STARTTLS transports (which
# real, older servers require). Sets CREATED_LISTENER=1 when it creates one, so the caller
# restarts once to bind the new socket (a fresh bootstrap; a warm start already has them).
ensure_starttls_listener() { # create-key  name  bind  protocol
  if listener_exists "$2"; then
    log "listener $2 already present"
    return 0
  fi
  resp=$(jmap "[\"x:NetworkListener/set\",{\"create\":{\"$1\":{\"name\":\"$2\",\"bind\":{\"$3\":true},\"protocol\":\"$4\",\"useTls\":true,\"tlsImplicit\":false}}},\"c0\"]")
  if ! printf '%s' "$resp" | grep -q '"created"'; then
    log "FAILED to create listener $2: $resp"
    return 1
  fi
  CREATED_LISTENER=1
  log "created STARTTLS listener $2 ($3, $4)"
}

# Open OAuth client registration (RFC 7591). Stalwart defaults it off, so a client that
# has never met this server cannot mint a client id and the whole discovered sign-in path
# is unreachable against the harness. The setting needs a restart to take effect, so this
# reports whether it changed anything and the caller restarts once for all of them.
#
# Off is the right default for a real deployment and wrong for a fixture: this server exists
# to be met by clients that have never seen it, on loopback, with throwaway accounts.
ensure_anonymous_registration() {
  current=$(jmap '["x:OidcProvider/get",{"ids":["singleton"],"properties":["anonymousClientRegistration"]},"c0"]')
  case "$current" in
    *'"anonymousClientRegistration":true'*)
      log "open client registration already enabled"
      return 0
      ;;
  esac
  jmap '["x:OidcProvider/set",{"update":{"singleton":{"anonymousClientRegistration":true}}},"c0"]' >/dev/null
  CREATED_LISTENER=1
  log "enabled open OAuth client registration"
}

trap 'stop_server; exit 0' TERM INT

rm -f "$MARKER"
log "starting Stalwart"
start_server
wait_http

if in_bootstrap_mode; then
  log "completing first-run bootstrap (internal directory, no ACME/auto-TLS)"
  jmap '["x:Bootstrap/set",{"update":{"singleton":{"requestTlsCertificate":false,"generateDkimKeys":false}}},"c0"]' >/dev/null
  log "restarting into full server"
  stop_server
  start_server
  wait_http
else
  log "store already bootstrapped; skipping setup"
fi

DOMAIN_ID="$(domain_id)"
[ -n "$DOMAIN_ID" ] || {
  log "could not resolve default domain id"
  exit 1
}
log "default domain id: $DOMAIN_ID"

ensure_account alice "Alice Tester" "${HARNESS_ALICE_PW:-harness-alice-pw}"
ensure_account bob "Bob Tester" "${HARNESS_BOB_PW:-harness-bob-pw}"

# STARTTLS listeners for the IMAP (143) and SMTP submission (587) transports the engine
# speaks in addition to implicit TLS. A newly created listener needs a server restart to
# bind its socket; a warm start already has them in its persisted config.
CREATED_LISTENER=0
ensure_starttls_listener imapstarttls imap "[::]:143" imap
ensure_starttls_listener submission submission "[::]:587" smtp
ensure_anonymous_registration
if [ "$CREATED_LISTENER" = 1 ]; then
  log "restarting to apply the new listeners and settings"
  stop_server
  start_server
  wait_http
fi

log "seeding shared dataset"
SEED_DIR="${SEED_DIR:-/harness/seed}" /bin/sh /harness/seed.sh

touch "$MARKER"
log "harness ready"

wait "$SRV"
