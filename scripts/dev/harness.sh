#!/usr/bin/env bash
# Lifecycle for the local seeded Stalwart mail/calendar test server the app is debugged against
# (docker/stalwart). A thin, human-runnable wrapper over `docker compose` so the harness is one
# command away; and so the debug-app tooling can bring it up before booting a client.
#
#   scripts/dev/harness.sh up [--bulk]   # start + seed; blocks until healthy (the default target)
#   scripts/dev/harness.sh down          # stop and remove the container
#   scripts/dev/harness.sh reset [--bulk] [--keep-clients]
#                                        # wipe volumes and re-bootstrap from empty (clean slate),
#                                        # clearing this host's client dev stores with it
#   scripts/dev/harness.sh status        # health + host port table
#   scripts/dev/harness.sh logs [-f]     # the server's own logs (seeding, requests)
#   scripts/dev/harness.sh test          # run the gated JMAP live test against it
#   scripts/dev/harness.sh deliver [--from B] [--subject S]
#                                        # drop a fresh message into alice@test.local's INBOX (via
#                                        # IMAP APPEND) so a background-sync 'detect' pass has new
#                                        # mail to find; the self-serve inbound for the bgsync loop
#
# --bulk seeds dozens of extra messages (SEED_BULK) into dev-only folders for a fuller mailbox;
# without it the seed is the deterministic CI fixture. See docker/stalwart/README.md.
set -euo pipefail
source "$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)/lib.sh"

# Absolute path to this script, resolved BEFORE the cd below so the `help` case can read it.
SELF="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)/$(basename "${BASH_SOURCE[0]}")"

require_cmd docker
cd "$STALWART_DIR"

# Collect a trailing --bulk / --keep-clients from anywhere in the args; export the seed toggle for
# compose.
BULK=0
KEEP_CLIENTS=0
ARGS=()
for arg in "$@"; do
  case "$arg" in
    --bulk) BULK=1 ;;
    --keep-clients) KEEP_CLIENTS=1 ;;
    *) ARGS+=("$arg") ;;
  esac
done
export SEED_BULK="$BULK"
cmd="${ARGS[0]:-up}"

# Clear this host's client dev stores, which a re-bootstrap has just invalidated.
#
# THE FAILURE THIS PREVENTS IS SILENT AND LOOKS LIKE A PRODUCT BUG. Stalwart mints its ids
# deterministically from an empty database, so the server that comes back up hands out the SAME
# ids; to a different set of messages, because the seed has grown since. A client store synced
# against the old generation keeps its cached bodies under those ids, so every message opens
# somebody else's body, with somebody else's attachments and no invitation part. Nothing errors;
# the app renders exactly what it was given. It cost a full round of "the invitation card is
# broken" before the store was read (docker/stalwart/README.md).
#
# Only this host's own desktop client. A mobile store lives inside an app container, so those are
# named rather than reached into.
clear_dev_stores() {
  local client mode dir cleared=()
  client="$(host_desktop_client)"
  if [[ -z "$client" ]]; then
    return
  fi
  for mode in "${DEV_STORE_MODES[@]}"; do
    dir="$(dev_store_dir "$client" "$mode")" || continue
    [[ -d "$dir" ]] || continue
    if [[ "$KEEP_CLIENTS" == 1 ]]; then
      cleared+=("$dir")
      continue
    fi
    # A running app holds the sqlite file open, and on Windows that makes the delete fail. Say so
    # rather than leaving a half-cleared store, which is the same silent-wrong-body state again.
    if rm -rf "$dir" 2>/dev/null; then
      cleared+=("$dir")
    else
      warn "could not clear $dir: close the app and re-run, or the client will read this seed's ids against the last one's bodies"
    fi
  done
  if [[ ${#cleared[@]} -eq 0 ]]; then
    return
  fi
  if [[ "$KEEP_CLIENTS" == 1 ]]; then
    warn "--keep-clients: these $client stores still hold the PREVIOUS server's ids and will show the wrong body for every message:"
    printf '  %s
' "${cleared[@]}" >&2
    return
  fi
  info "cleared the $client client's harness stores (the new server reuses the old ids):"
  printf '  %s
' "${cleared[@]}"
  info "a client store on a phone or simulator is stale too: reinstall the app there, or clear its data"
}

# Health + the host-port table + seeded accounts. A function (not a re-exec) since we've cd'd
# into the compose dir and $0 no longer resolves as a relative path.
show_status() {
  docker compose ps
  cat <<EOF

Host ports (loopback only): the 12xxx/28080 block, kept clear of the engine repo's 11xxx/18080:
  JMAP + CalDAV + admin  http://127.0.0.1:28080
  SMTP (plaintext)       127.0.0.1:12025
  IMAP (implicit TLS)    127.0.0.1:12993

Seeded account: alice@test.local / harness-alice-pw   (also bob@test.local, admin)
EOF
}

case "$cmd" in
  up)
    [[ "$BULK" == 1 ]] && info "seeding bulk dev mailbox (SEED_BULK=1)"
    info "starting the Stalwart harness (bootstrap + seed; blocks until healthy)"
    docker compose up -d --wait
    info "harness healthy"
    extract_harness_ca && info "extracted harness IMAP cert -> $HARNESS_CA (for --account stalwart-imap)" || true
    show_status
    ;;
  down)
    info "stopping the Stalwart harness"
    docker compose down
    ;;
  reset)
    info "resetting the Stalwart harness (wiping volumes and re-bootstrapping)"
    docker compose down -v
    docker compose up -d --wait
    info "harness healthy (clean slate)"
    extract_harness_ca && info "extracted harness IMAP cert -> $HARNESS_CA (for --account stalwart-imap)" || true
    clear_dev_stores
    show_status
    ;;
  status)
    show_status
    ;;
  logs)
    shift_flag="${ARGS[1]:-}"
    if [[ "$shift_flag" == "-f" ]]; then docker compose logs -f; else docker compose logs; fi
    ;;
  test)
    require_harness
    info "running the gated JMAP live test against the harness"
    cd "$REPO_ROOT"
    STALWART_HTTP_ADDR="$STALWART_HTTP_ADDR" \
      STALWART_ACCOUNT="alice@test.local" \
      STALWART_PASSWORD="harness-alice-pw" \
      cargo test -p mailcal-account --test live_jmap -- --nocapture
    ;;
  deliver)
    require_harness
    require_cmd python3
    to="alice@test.local"; from="bob@test.local"; subject="Harness test $(date '+%H:%M:%S')"
    i=1
    while [[ $i -lt ${#ARGS[@]} ]]; do
      case "${ARGS[$i]}" in
        --from)    from="${ARGS[$((i+1))]:?--from needs a value}"; i=$((i+2)) ;;
        --subject) subject="${ARGS[$((i+1))]:?--subject needs a value}"; i=$((i+2)) ;;
        *) die "deliver: unknown arg '${ARGS[$i]}' (--from|--subject)" ;;
      esac
    done
    info "appending to $to's INBOX (from $from): \"$subject\""
    # IMAP APPEND, not SMTP: Stalwart's spam filter files an unauthenticated bob->alice SMTP message
    # into Junk, but the background-sync detect pass only scans the INBOX; so append straight into
    # the INBOX (also how the feature was first live-tested). Self-signed cert => unverified TLS.
    HOST="${STALWART_IMAP_ADDR%%:*}" PORT="${STALWART_IMAP_ADDR##*:}" \
    ACCOUNT="$to" PW="$STALWART_ALICE_PW" MAIL_FROM="$from" SUBJECT="$subject" python3 - <<'PY' || die "IMAP APPEND to $STALWART_IMAP_ADDR failed"
import datetime, email.utils, imaplib, os, ssl, sys, time
# A UTC (+0000) Date, matching the seed fixtures' convention. (A non-UTC numeric offset used to
# fail the whole JMAP sync; the engine's `sentAt` parser accepted only a Z/UTC date-time; fixed
# upstream in engine#42, which parses RFC 3339 offsets and degrades an unparseable value to None.)
date = email.utils.format_datetime(datetime.datetime.now(datetime.timezone.utc))
msg = (f"From: {os.environ['MAIL_FROM']}\r\nTo: {os.environ['ACCOUNT']}\r\n"
       f"Subject: {os.environ['SUBJECT']}\r\nDate: {date}\r\n"
       "\r\nAppended by scripts/dev/harness.sh deliver.\r\n")
try:
    M = imaplib.IMAP4_SSL(os.environ["HOST"], int(os.environ["PORT"]),
                          ssl_context=ssl._create_unverified_context())
    M.login(os.environ["ACCOUNT"], os.environ["PW"])
    ok, _ = M.append("INBOX", "", imaplib.Time2Internaldate(time.time()), msg.encode())
    M.logout()
    sys.exit(0 if ok == "OK" else 1)
except Exception as e:
    print(f"imap error: {e}", file=sys.stderr); sys.exit(1)
PY
    info "delivered to the INBOX: a background-sync 'detect' pass will now find it"
    ;;
  -h|--help|help)
    sed -n '2,22p' "$SELF"
    ;;
  *)
    die "unknown command '$cmd' (up|down|reset|status|logs|test|deliver)"
    ;;
esac
