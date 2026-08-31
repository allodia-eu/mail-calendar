#!/usr/bin/env bash
# Probe whether a CalDAV server implements RFC 6638 calendar-auto-schedule.
#
# Read-only: sends OPTIONS and PROPFIND only, so it never creates, changes or
# mails anything. Answers one question; "if we rewrite PARTSTAT on this server,
# does the server email the organiser?"; because an RSVP button that only
# writes a local PARTSTAT tells nobody, which is worse than no button.
#
# Credentials come from an env file outside the repo (mode 600) and are passed
# to curl through a `-K` config file, never `-u`: `-u` puts the password in the
# process list where any local process can read it.
#
# Usage:
#   scripts/dev/caldav-autoschedule-probe.sh [env-file]
#
# The env file defines:
#   SOVERIN_CALDAV_URL=https://caldav.example.net
#   SOVERIN_USER=…
#   SOVERIN_PASS=…
#
# See docs/invitations.md → "Does this server reply for us?".

set -euo pipefail

ENV_FILE="${1:-$HOME/.config/allodia/soverin-test.env}"

if [[ ! -f "$ENV_FILE" ]]; then
  echo "error: no env file at $ENV_FILE" >&2
  exit 1
fi

# shellcheck disable=SC1090
source "$ENV_FILE"

: "${SOVERIN_CALDAV_URL:?env file must set SOVERIN_CALDAV_URL}"
: "${SOVERIN_USER:?env file must set SOVERIN_USER}"
: "${SOVERIN_PASS:?env file must set SOVERIN_PASS}"

BASE="${SOVERIN_CALDAV_URL%/}"

CURL_CFG="$(mktemp)"
chmod 600 "$CURL_CFG"
trap 'rm -f "$CURL_CFG"' EXIT

# The password never reaches argv, only this 600 file.
{
  printf 'user = "%s:%s"\n' "$SOVERIN_USER" "$SOVERIN_PASS"
  printf 'silent\n'
  printf 'location\n'
  printf 'max-time = 30\n'
} >"$CURL_CFG"

# curl -K "$CURL_CFG" plus per-call args; -D - dumps response headers.
dav() { curl -K "$CURL_CFG" "$@"; }

say() { printf '\n\033[1m%s\033[0m\n' "$*"; }

say "1. OPTIONS $BASE: the DAV: header lists the compliance classes"
DAV_HEADER="$(dav -X OPTIONS -D - -o /dev/null "$BASE/" | tr -d '\r' | grep -i '^dav:' || true)"
echo "${DAV_HEADER:-  (no DAV: header returned)}"

if grep -qi 'calendar-auto-schedule' <<<"$DAV_HEADER"; then
  echo "  => ADVERTISED: calendar-auto-schedule (RFC 6638)"
else
  echo "  => NOT advertised at this path (try the calendar home below)"
fi

say "2. PROPFIND $BASE: current-user-principal"
PRINCIPAL_XML="$(dav -X PROPFIND -H 'Depth: 0' -H 'Content-Type: application/xml' \
  --data-binary '<?xml version="1.0" encoding="utf-8"?>
<d:propfind xmlns:d="DAV:"><d:prop><d:current-user-principal/></d:prop></d:propfind>' \
  "$BASE/")"
PRINCIPAL_PATH="$(sed -n 's:.*<[^>]*href[^>]*>\([^<]*\)</[^>]*href>.*:\1:p' <<<"$PRINCIPAL_XML" | tail -1)"
echo "  principal: ${PRINCIPAL_PATH:-<none found>}"

if [[ -z "$PRINCIPAL_PATH" ]]; then
  echo "  (raw response follows)"
  echo "$PRINCIPAL_XML"
  exit 1
fi

# href may be a path or an absolute URL.
case "$PRINCIPAL_PATH" in
  http*) PRINCIPAL_URL="$PRINCIPAL_PATH" ;;
  *) PRINCIPAL_URL="$BASE$PRINCIPAL_PATH" ;;
esac

say "3. PROPFIND $PRINCIPAL_URL: schedule-inbox-URL / schedule-outbox-URL / calendar-home-set"
SCHED_XML="$(dav -X PROPFIND -H 'Depth: 0' -H 'Content-Type: application/xml' \
  --data-binary '<?xml version="1.0" encoding="utf-8"?>
<d:propfind xmlns:d="DAV:" xmlns:c="urn:ietf:params:xml:ns:caldav"><d:prop>
  <c:calendar-home-set/>
  <c:schedule-inbox-URL/>
  <c:schedule-outbox-URL/>
  <c:calendar-user-address-set/>
</d:prop></d:propfind>' \
  "$PRINCIPAL_URL")"
echo "$SCHED_XML" | tr '>' '>\n' | sed 's/^/  /'

for prop in schedule-inbox-URL schedule-outbox-URL; do
  if grep -qi "$prop" <<<"$SCHED_XML" && ! grep -qi "404\|not found" <<<"$SCHED_XML"; then
    echo "  => principal exposes $prop"
  fi
done

HOME_PATH="$(sed -n 's:.*calendar-home-set[^>]*>.*<[^>]*href[^>]*>\([^<]*\)</.*:\1:p' <<<"$(tr -d '\n' <<<"$SCHED_XML")" | head -1)"
if [[ -n "$HOME_PATH" ]]; then
  case "$HOME_PATH" in
    http*) HOME_URL="$HOME_PATH" ;;
    *) HOME_URL="$BASE$HOME_PATH" ;;
  esac
  say "4. OPTIONS $HOME_URL: the calendar home's DAV: header"
  HOME_DAV="$(dav -X OPTIONS -D - -o /dev/null "$HOME_URL" | tr -d '\r' | grep -i '^dav:' || true)"
  echo "${HOME_DAV:-  (no DAV: header returned)}"
  if grep -qi 'calendar-auto-schedule' <<<"$HOME_DAV"; then
    echo "  => ADVERTISED at the calendar home: calendar-auto-schedule (RFC 6638)"
  fi
fi

say "Verdict"
if grep -qi 'calendar-auto-schedule' <<<"$DAV_HEADER${HOME_DAV:-}"; then
  echo "  RFC 6638 auto-schedule IS advertised: a PARTSTAT rewrite makes the"
  echo "  server send the iTIP REPLY, so the CalDAV RSVP button is honest."
else
  echo "  auto-schedule NOT advertised: a PARTSTAT rewrite would be local-only."
  echo "  Do not offer an RSVP button on this account without client-side iMIP."
fi
