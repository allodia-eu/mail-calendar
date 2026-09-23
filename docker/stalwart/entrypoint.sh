#!/bin/sh
# Self-bootstrapping entrypoint for the deterministic Stalwart test harness.
#
# Stalwart v0.16 has no configuration file: a fresh server boots into "bootstrap mode", is
# configured through its management API, and restarts to come up as a full server. This wrapper
# drives that sequence with `stalwart-cli apply` (Stalwart's own CLI, built into the image by
# ./Dockerfile), then seeds the shared dataset, so `docker compose up` yields an identical, ready
# server every time:
#
#   1. start the server (bootstrap mode if the store is empty),
#   2. complete setup (no ACME, no DKIM) and restart into a full server,
#   3. apply the harness configuration below and restart, since Stalwart reads it at startup,
#   4. seed mail (IMAP over TLS) and calendars (CalDAV over plain HTTP),
#   5. write a readiness marker and run the server in the foreground.
#
# HARNESS_SEED=0 applies alice alone and seeds nothing: the sign-in server (`stalwart-oauth` in
# docker-compose.yml) needs an account to sign in as and nothing to read.
#
# It is idempotent: every plan is an `upsert` or a singleton `update`, which converges on a warm
# data volume rather than duplicating, and the content seeder clears before it appends.
set -eu

CONFIG="${STALWART_CONFIG:-/etc/stalwart/config.json}"
HTTP="http://127.0.0.1:8080"
ADMIN_PW="${HARNESS_ADMIN_PW:-harness-admin-pw}"
MARKER="/var/lib/stalwart/.harness-ready"

log() { printf '[harness] %s\n' "$1"; }

# The CLI caches the server's schema under $HOME, and the image's user has no home directory.
cli() {
  HOME=/tmp stalwart-cli --no-color --url "$HTTP" --user admin --password "$ADMIN_PW" "$@"
}

# Applies the NDJSON plan on stdin, stopping the entrypoint (and so the container) on any failure.
apply() {
  if ! out=$(cli apply --quiet --stdin 2>&1); then
    log "FAILED to apply a configuration plan: $out"
    exit 1
  fi
  log "$out"
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

restart_server() {
  log "restarting: $1"
  stop_server
  start_server
  wait_http
}

trap 'stop_server; exit 0' TERM INT

rm -f "$MARKER"
log "starting Stalwart"
start_server
wait_http

# Only a server in bootstrap mode has the Bootstrap object.
if cli get Bootstrap singleton >/dev/null 2>&1; then
  log "completing first-run bootstrap"
  # The mail domain is stated rather than derived: Stalwart derives it from the hostname, and the
  # sign-in server's hostname is `localhost`, which derives to `example.org`.
  apply <<'EOF'
{"@type":"update","object":"Bootstrap","value":{"defaultDomain":"test.local","requestTlsCertificate":false,"generateDkimKeys":false}}
EOF
  restart_server "into a full server"
else
  log "store already bootstrapped; skipping setup"
fi

# alice, on both servers. The anonymous rate limit goes because every request a test sends comes
# from one address: a few sign-in runs in a row pass the default and get `429`, which a sign-in
# pre-flight reads as "this server offers no sign-in".
apply <<'EOF'
{"@type":"upsert","object":"Domain","matchOn":["name"],"value":{"domain":{"name":"test.local"}}}
{"@type":"upsert","object":"Account","matchOn":["name"],"value":{"alice":{"@type":"User","name":"alice","domainId":"#domain","description":"Alice Tester","credentials":{"0":{"@type":"Password","secret":"harness-alice-pw"}},"roles":{"@type":"User"}}}}
{"@type":"update","object":"Http","value":{"rateLimitAnonymous":null}}
EOF

# bob, and STARTTLS listeners for IMAP (143) and SMTP submission (587). Stalwart supports both but
# recommends implicit TLS (993/465), so a fresh bootstrap has neither; they are here to exercise the
# engine's STARTTLS transports, which real, older servers require.
if [ "${HARNESS_SEED:-1}" != 0 ]; then
  apply <<'EOF'
{"@type":"upsert","object":"Domain","matchOn":["name"],"value":{"domain":{"name":"test.local"}}}
{"@type":"upsert","object":"Account","matchOn":["name"],"value":{"bob":{"@type":"User","name":"bob","domainId":"#domain","description":"Bob Tester","credentials":{"0":{"@type":"Password","secret":"harness-bob-pw"}},"roles":{"@type":"User"}}}}
{"@type":"upsert","object":"NetworkListener","matchOn":["name"],"value":{"imap":{"name":"imap","bind":{"[::]:143":true},"protocol":"imap","useTls":true,"tlsImplicit":false},"submission":{"name":"submission","bind":{"[::]:587":true},"protocol":"smtp","useTls":true,"tlsImplicit":false}}}
EOF
fi

restart_server "to bind new listeners and read the settings above"

if [ "${HARNESS_SEED:-1}" != 0 ]; then
  log "seeding shared dataset"
  SEED_DIR="${SEED_DIR:-/harness/seed}" /bin/sh /harness/seed.sh
fi

touch "$MARKER"
log "harness ready"

wait "$SRV"
