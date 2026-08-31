#!/bin/sh
# Deterministic content seeder for the Stalwart test harness.
#
# Called by entrypoint.sh once the server is a full, bootstrapped server with the
# test accounts present. Pure curl over the real wire protocols the engine's
# clients will use (steps 4-5): IMAP for mail, CalDAV for calendars. No Rust
# provider client and no extra binary.
#
# Transport (v0.16 defaults): IMAP is implicit-TLS on 993: `-k` accepts the
# server's self-signed test certificate (this never touches a host trust store).
# CalDAV rides the plaintext HTTP listener on 8080.
#
# Idempotent: managed mailboxes are cleared before append and CalDAV PUT is
# idempotent by request URI, so a re-run converges to the same state. Asserting
# side: never assert on server-assigned ids (IMAP UID, DAV ETag), fixtures
# carry content the harness controls (subjects, Message-IDs, iCalendar UIDs).
set -eu

SEED_DIR="${SEED_DIR:-/harness/seed}"
MAIL_DIR="$SEED_DIR/mail"
CAL_DIR="$SEED_DIR/calendar"
CARD_DIR="$SEED_DIR/contacts"

ALICE="alice@test.local"
ALICE_PW="${HARNESS_ALICE_PW:-harness-alice-pw}"
# The second account exists so contact dedup can be proven ACROSS accounts, which is the whole
# claim of the feature and cannot be shown with one account (entrypoint.sh provisions both).
BOB="bob@test.local"
BOB_PW="${HARNESS_BOB_PW:-harness-bob-pw}"

IMAPS="imaps://127.0.0.1:993"
HTTP="http://127.0.0.1:8080"
CAL_COLLECTION="$HTTP/dav/cal/$ALICE/default"
ALICE_CARDS="$HTTP/dav/card/$ALICE/default"
BOB_CARDS="$HTTP/dav/card/$BOB/default"

log() { printf '[seed] %s\n' "$1"; }

# curl over implicit-TLS IMAP, accepting the self-signed test cert.
imap() { curl -sk --user "$ALICE:$ALICE_PW" "$@"; }

imap_append() { # file  mailbox
  # curl's IMAP APPEND needs the literal size upfront, so it cannot stream from
  # a pipe ("Cannot APPEND with unknown input file size"); stage to a temp file
  # whose size curl can stat.
  _tmp=$(mktemp)
  sed 's/$/\r/' "$1" >"$_tmp"
  _rc=0
  imap --url "$IMAPS/$2" --upload-file "$_tmp" || _rc=$?
  rm -f "$_tmp"
  return "$_rc"
}

imap_cmd() { # mailbox  command
  imap --url "$IMAPS/$1" --request "$2"
}

imap_clear() { # mailbox
  imap_cmd "$1" "STORE 1:* +FLAGS (\\Deleted)" >/dev/null 2>&1 || true
  imap_cmd "$1" "EXPUNGE" >/dev/null 2>&1 || true
}

put_calendar() { # file  uid
  sed 's/$/\r/' "$1" | curl -sk --user "$ALICE:$ALICE_PW" \
    -X PUT -H 'Content-Type: text/calendar; charset=utf-8' \
    --data-binary @- "$CAL_COLLECTION/$2.ics"
}

# CardDAV PUT of one vCard. Takes the collection and credentials explicitly (unlike put_calendar)
# because contacts are seeded into TWO accounts: that is the point of the shared fixture.
# Emits a card, with `$2` inlined as a `PHOTO` before END:VCARD when a photo is named.
#
# The photos live beside the cards as ordinary JPEGs rather than inside them: a base64 blob is
# 200 unreadable lines in a fixture whose whole point is that a human can see what it says, and
# git stores the binary far better than its expansion. Folded per RFC 6350 3.2: continuation
# lines begin with a single space.
#
# POSIX `[ ]`, never `[[ ]]`: this script's shebang is /bin/sh and the entrypoint runs it with
# dash, where `[[` is simply not a command. The failure is silent and inverted: dash reports
# "not found" for both halves, `!` turns the second into success, and every card takes the
# no-photo branch. Every fixture then seeds correctly, reads correctly and shows a monogram, so
# nothing looks broken anywhere: the app is simply told nobody has a photo.
card_body() { # file  [photo]
  if [ -z "${2:-}" ] || [ ! -f "$2" ]; then cat "$1"; return; fi
  sed '/^END:VCARD/d' "$1"
  printf 'PHOTO;TYPE=JPEG:data:image/jpeg;base64,%s' "$(base64 < "$2" | tr -d '\n')" \
    | fold -w 74 | awk 'NR==1 { print; next } { print " " $0 }'
  printf 'END:VCARD\n'
}

put_contact() { # file  uid  collection  credentials  [photo]
  card_body "$1" "${5:-}" | sed 's/$/\r/' | curl -sk --user "$4" \
    -X PUT -H 'Content-Type: text/vcard; charset=utf-8' \
    --data-binary @- "$3/$2.vcf"
}

log "waiting for IMAP to accept a login for $ALICE"
i=0
until imap_cmd INBOX "NOOP" >/dev/null 2>&1; do
  i=$((i + 1))
  [ "$i" -gt 60 ] && {
    log "IMAP never became ready"
    exit 1
  }
  sleep 1
done

log "ensuring mailboxes exist"
imap_cmd INBOX "CREATE Archive" >/dev/null 2>&1 || true
imap_cmd INBOX "CREATE Projects" >/dev/null 2>&1 || true
# QResync is a dedicated, otherwise-untouched mailbox the CONDSTORE/QRESYNC delta
# test mutates in isolation (it re-flags one message and expunges another), so it
# never disturbs the count-asserted INBOX/Archive/Projects.
imap_cmd INBOX "CREATE QResync" >/dev/null 2>&1 || true
# Idle is a second dedicated mailbox the IMAP IDLE (RFC 2177) push test watches and
# flag-toggles in isolation, so its mutations never disturb the count-asserted
# mailboxes either.
imap_cmd INBOX "CREATE Idle" >/dev/null 2>&1 || true

log "clearing managed mailboxes for an idempotent re-seed"
imap_clear INBOX
imap_clear Archive
imap_clear Projects
imap_clear QResync
imap_clear Idle

# INBOX was just cleared, so appends land at deterministic sequence numbers
# (Stalwart's SEARCH does not match on a HEADER Message-ID, so we rely on append
# order rather than searching). The trailing comments are those sequence numbers.
log "appending mail fixtures to INBOX"
imap_append "$MAIL_DIR/01-plain.eml" INBOX        # seq 1
imap_append "$MAIL_DIR/02-dup-msgid-a.eml" INBOX  # seq 2
imap_append "$MAIL_DIR/02-dup-msgid-b.eml" INBOX  # seq 3
imap_append "$MAIL_DIR/03-no-msgid.eml" INBOX     # seq 4
imap_append "$MAIL_DIR/04-attachment.eml" INBOX   # seq 5
imap_append "$MAIL_DIR/05-flagged.eml" INBOX      # seq 6
imap_append "$MAIL_DIR/06-thread-root.eml" INBOX  # seq 7
imap_append "$MAIL_DIR/06-thread-reply.eml" INBOX # seq 8

log "setting flags + custom keyword on the flagged fixture (seq 6)"
imap_cmd INBOX "STORE 6 +FLAGS (\\Seen \\Flagged harness)" >/dev/null

log "copying the baseline message (seq 1) into Archive (two memberships)"
imap_cmd INBOX "COPY 1 Archive" >/dev/null

log "seeding the dedicated QResync mailbox (three messages) for the QRESYNC delta test"
imap_cmd INBOX "COPY 1:3 QResync" >/dev/null

log "seeding the dedicated Idle mailbox (one message) for the IMAP IDLE push test"
imap_cmd INBOX "COPY 1 Idle" >/dev/null

log "moving a message from INBOX into Projects (single membership)"
imap_append "$MAIL_DIR/07-moved.eml" INBOX # seq 9
if ! imap_cmd INBOX "MOVE 9 Projects" >/dev/null 2>&1; then
  imap_cmd INBOX "COPY 9 Projects" >/dev/null
  imap_cmd INBOX "STORE 9 +FLAGS (\\Deleted)" >/dev/null
  imap_cmd INBOX "EXPUNGE" >/dev/null
fi

# The HTML fixture is appended LAST, after every STORE/COPY/MOVE above has run.
# Those steps address messages by IMAP sequence number, and an APPEND earlier in
# the list would renumber the ones after it, silently re-pointing "STORE 6"
# (the flagged fixture) or "MOVE 9" at the wrong message. Landing it here means
# it shifts nothing: it takes the next free sequence number after the MOVE (or
# its COPY+EXPUNGE fallback) has already renumbered INBOX.
#
# It is the only fixture with a text/html part, so it is what exercises the
# reading path's HTML half on a REAL transport: the sanitiser, the shared reading
# document (CSP + base CSS), and (because its <img> is remote) the block-by-
# default remote-image gate and its "load remote images" opt-in. Its remote host
# is deliberately unresolvable (.invalid, RFC 2606), so the image can never load
# even if the gate is opened: the fixture proves the image is BLOCKED, and must
# not depend on the harness having internet access to do so.
log "appending the HTML fixture to INBOX (rendering path; sequence-safe, so it goes last)"
imap_append "$MAIL_DIR/08-html.eml" INBOX

# Same reasoning as the HTML fixture: appended after every sequence-numbered step, so it renumbers
# nothing. It is the only message addressed to more than one person and the only one with a Cc,
# which makes it the only fixture a REPLY-ALL can be judged against: every other one derives a
# single recipient, and a single recipient is precisely the case that stayed correct while a
# multi-recipient To was rendered with its last address loose and a one-address Cc with no pill at
# all. A composer bug the whole harness could not reproduce is one nobody finds twice.
log "appending the multi-recipient fixture to INBOX (reply-all; sequence-safe, so it goes last too)"
imap_append "$MAIL_DIR/09-reply-all.eml" INBOX

# Sequence-safe for the same reason, and the only fixture with more attachments than fit on screen.
# Twenty is not an absurd number (a quote pack or a scanned bundle reaches it), and the reading
# view stacks the attachment bar ABOVE the message in a column that does not scroll, so a long
# enough bar pushes the body off the bottom with no way to reach it. One attachment cannot show
# that, which is why 04-attachment.eml never did.
log "appending the many-attachment fixture to INBOX (reading-view overflow; sequence-safe)"
imap_append "$MAIL_DIR/10-many-attachments.eml" INBOX

log "putting calendar fixtures into the default calendar"
put_calendar "$CAL_DIR/one-off.ics" oneoff-2001
put_calendar "$CAL_DIR/recurring-weekly.ics" weekly-2002
put_calendar "$CAL_DIR/meeting-attendees.ics" meeting-2003
put_calendar "$CAL_DIR/virtual-location.ics" virtual-2004
put_calendar "$CAL_DIR/all-day.ics" allday-2005
put_calendar "$CAL_DIR/floating.ics" floating-2006

# The six fixtures above are pinned to absolute dates in early 2026 because they encode ENGINE
# semantics that must not drift (floating time, a zoneless all-day, a DST boundary, a recurrence with
# an overridden instance). They are the deterministic CI fixture, and they stay.
#
# They are also all in the past, so a grid seeded with only them opens EMPTY, and an empty grid
# teaches you nothing about a grid. The living week re-anchors a second set on the current Monday at
# every seed, and deliberately hits the rules a client can get wrong: a block too short to hold its
# own title, three events overlapping into columns, an all-day banner past its cap, a band spanning
# three days, and something in the weeks either side so a swipe lands on content.
log "putting the living week into the default calendar (anchored on this Monday)"
sh "$SEED_DIR/../seed-calendar-week.sh" "$CAL_COLLECTION" "$ALICE:$ALICE_PW"

# Contacts, into BOTH accounts' default address books. The split is deliberate and is what the
# fixtures exist to prove (docker/stalwart/seed/contacts/README.md): `shared-*` is one person at
# one address filed in two accounts and must merge into a single row that discloses the merge,
# while the two `namesake-*` cards share only a NAME and must stay separate. Seeding both into one
# account would leave the cross-account case (the headline claim) untested.
log "putting contact fixtures into alice's address book"
put_contact "$CARD_DIR/shared-alice.vcf" shared-iris "$ALICE_CARDS" "$ALICE:$ALICE_PW" "$CARD_DIR/photos/shared-iris.jpg"
put_contact "$CARD_DIR/namesake-a.vcf" namesake-a "$ALICE_CARDS" "$ALICE:$ALICE_PW" "$CARD_DIR/photos/namesake-a.jpg"
put_contact "$CARD_DIR/namesake-b.vcf" namesake-b "$ALICE_CARDS" "$ALICE:$ALICE_PW"
put_contact "$CARD_DIR/ahmed.vcf" ahmed "$ALICE_CARDS" "$ALICE:$ALICE_PW" "$CARD_DIR/photos/ahmed.jpg"
put_contact "$CARD_DIR/sofie.vcf" sofie "$ALICE_CARDS" "$ALICE:$ALICE_PW" "$CARD_DIR/photos/sofie.jpg"
put_contact "$CARD_DIR/numeric.vcf" numeric "$ALICE_CARDS" "$ALICE:$ALICE_PW"
# The one card with a PHOTO, and the one whose address matches a mail sender: both halves are
# what makes the avatar photo path reachable at all (seed/contacts/README.md).
put_contact "$CARD_DIR/bestuur.vcf" bestuur "$ALICE_CARDS" "$ALICE:$ALICE_PW" "$CARD_DIR/photos/bestuur.jpg"
put_contact "$CARD_DIR/bob.vcf" bob "$ALICE_CARDS" "$ALICE:$ALICE_PW" "$CARD_DIR/photos/bob.jpg"

log "putting the shared contact into bob's address book (proves cross-account dedup)"
put_contact "$CARD_DIR/shared-bob.vcf" shared-iris "$BOB_CARDS" "$BOB:$BOB_PW" "$CARD_DIR/photos/shared-iris.jpg"

# ---------------------------------------------------------------------------
# Meeting invitations over the real wire (docs/invitations.md).
#
# Appended AFTER every STORE/COPY/MOVE above, for the same reason 08-html.eml is: those steps address
# messages by IMAP sequence number, and an APPEND earlier in the list silently re-points "STORE 6" or
# "MOVE 9" at the wrong message.
#
# These two are the contrast the whole G6 rule turns on, and neither can be shown without the other:
#
#   invite   `text/calendar; method=REQUEST` as an alternative BODY part, no Content-Disposition at
#            all (iMIP, RFC 6047 §2.4). It must produce a card and **no** paperclip, and its
#            `METHOD:REQUEST` plus an ATTENDEE matching alice is the two-condition RSVP gate passing.
#   publish  the same media type, explicitly dispositioned as a file, `METHOD:PUBLISH`, no ATTENDEE
#            lines at all. It must produce **no** card and **keep** its chip. Before the fix, both of
#            these showed a junk `attachment-N.ics` row.
#
# A THIRD invitation lands on the weekend, and it exists to make one check able to fail. The rule is
# that the day preview opens expanded whenever the calendar was read (docs/invitations.md), but the
# Monday invitation below sits in the review/triage overlap, so its preview would be open under the
# OLD rule too ("open only when the count is non-zero"). A test that can only see a conflicted day
# passes either way and proves nothing. The weekend is the day the living week deliberately leaves
# empty, so this one is the only fixture that tells the two rules apart.
#
# The invitation is generated rather than a static fixture because it has to agree with the living
# week: its UID and its start match the NEEDS-ACTION hold seed-calendar-week.sh puts on Monday
# mid-morning, so the card's conflict count excludes its own event (the first exclusion in
# docs/invitations.md) and its preview lands in the review/triage overlap with something to show. A
# fixture pinned to 2026 would render a correct card over an empty day, which is exactly the "the grid
# opens empty and teaches you nothing" problem the living week exists to solve.
#
# The anonymized per-sender MIME shapes (Outlook's base64 + iso-8859-1, Gmail's duplicate chip) belong
# to the unit suite, not here: this proves the transport, not the parser.
INVITE_MONDAY=$(date -u -d "-$(($(date -u +%u) - 1)) days" +%Y%m%d)
# Derived from Monday, never from a second offset off today: on a Monday `%u - 2` is -1, and the
# string that builds is `--1 days`, which GNU date reads as MINUS one day, so the seeded PUBLISH
# fixture got a DTEND the day BEFORE its DTSTART, one day in seven.
INVITE_TUESDAY=$(date -u -d "$INVITE_MONDAY +1 days" +%Y%m%d)
# Saturday, derived off Monday for the same reason Tuesday is. seed-calendar-week.sh fills day
# offsets 0..4 only, so this one is genuinely empty, which is the entire point of the fixture.
INVITE_SATURDAY=$(date -u -d "$INVITE_MONDAY +5 days" +%Y%m%d)
INVITE_UID="week-invited@test.local"
INVITE_FREE_UID="week-invited-free@test.local"

log "appending the iMIP meeting invitation to INBOX (body part, no disposition — must show no chip)"
tmp=$(mktemp)
{
  printf 'From: Bob Tester <bob@test.local>\n'
  printf 'To: Alice Tester <alice@test.local>\n'
  printf 'Subject: Quarterly planning\n'
  printf 'Message-ID: <invite-%s@test.local>\n' "$INVITE_MONDAY"
  # Dated after every other fixture so the invitation is the top row of the inbox: this is the
  # message a developer opens to look at the card, and hunting for it in the middle of the list is
  # friction with no purpose.
  printf 'Date: Wed, 21 Jan 2026 08:00:00 +0000\n'
  printf 'MIME-Version: 1.0\n'
  printf 'Content-Type: multipart/alternative; boundary="harness-imip"\n\n'
  printf -- '--harness-imip\n'
  printf 'Content-Type: text/plain; charset=utf-8\n\n'
  printf 'You are invited to Quarterly planning. Please let me know if you can make it.\n\n'
  # No Content-Disposition, no filename: this is a body part, a sibling of the text above.
  printf -- '--harness-imip\n'
  printf 'Content-Type: text/calendar; charset=utf-8; method=REQUEST\n'
  printf 'Content-Transfer-Encoding: 8bit\n\n'
  printf 'BEGIN:VCALENDAR\n'
  printf 'VERSION:2.0\n'
  printf 'PRODID:-//Allodia//Harness iMIP//EN\n'
  printf 'CALSCALE:GREGORIAN\n'
  # The METHOD inside the body is the authoritative one, not the Content-Type parameter above.
  printf 'METHOD:REQUEST\n'
  printf 'BEGIN:VTIMEZONE\n'
  printf 'TZID:Europe/Amsterdam\n'
  printf 'BEGIN:DAYLIGHT\n'
  printf 'TZOFFSETFROM:+0100\n'
  printf 'TZOFFSETTO:+0200\n'
  printf 'TZNAME:CEST\n'
  printf 'DTSTART:19700329T020000\n'
  printf 'RRULE:FREQ=YEARLY;BYMONTH=3;BYDAY=-1SU\n'
  printf 'END:DAYLIGHT\n'
  printf 'BEGIN:STANDARD\n'
  printf 'TZOFFSETFROM:+0200\n'
  printf 'TZOFFSETTO:+0100\n'
  printf 'TZNAME:CET\n'
  printf 'DTSTART:19701025T030000\n'
  printf 'RRULE:FREQ=YEARLY;BYMONTH=10;BYDAY=-1SU\n'
  printf 'END:STANDARD\n'
  printf 'END:VTIMEZONE\n'
  printf 'BEGIN:VEVENT\n'
  printf 'UID:%s\n' "$INVITE_UID"
  printf 'DTSTAMP:20260101T000000Z\n'
  printf 'DTSTART;TZID=Europe/Amsterdam:%sT103000\n' "$INVITE_MONDAY"
  printf 'DTEND;TZID=Europe/Amsterdam:%sT113000\n' "$INVITE_MONDAY"
  printf 'SUMMARY:Quarterly planning\n'
  printf 'LOCATION:Room 4 / Meet link in the notes\n'
  printf 'DESCRIPTION:Budget, headcount and the roadmap for next quarter.\n'
  printf 'ORGANIZER;CN=Bob Tester:mailto:bob@test.local\n'
  printf 'ATTENDEE;CN=Bob Tester;ROLE=CHAIR;PARTSTAT=ACCEPTED;RSVP=FALSE:mailto:bob@test.local\n'
  printf 'ATTENDEE;CN=Alice Tester;ROLE=REQ-PARTICIPANT;PARTSTAT=NEEDS-ACTION;RSVP=TRUE:mailto:alice@test.local\n'
  printf 'ATTENDEE;CN=Carol External;ROLE=REQ-PARTICIPANT;PARTSTAT=ACCEPTED;RSVP=TRUE:mailto:carol@example.com\n'
  printf 'X-MICROSOFT-CDO-BUSYSTATUS:TENTATIVE\n'
  printf 'X-MICROSOFT-CDO-INTENDEDSTATUS:BUSY\n'
  printf 'END:VEVENT\n'
  printf 'END:VCALENDAR\n\n'
  printf -- '--harness-imip--\n'
} >"$tmp"
imap_append "$tmp" INBOX
rm -f "$tmp"

log "appending a published .ics as a real attachment (PUBLISH, no attendees — no card, chip stays)"
tmp=$(mktemp)
{
  printf 'From: Residents Association <notices@example.com>\n'
  printf 'To: Alice Tester <alice@test.local>\n'
  printf 'Subject: Annual general meeting\n'
  printf 'Message-ID: <publish-0010@test.local>\n'
  printf 'Date: Wed, 21 Jan 2026 07:00:00 +0000\n'
  printf 'MIME-Version: 1.0\n'
  printf 'Content-Type: multipart/mixed; boundary="harness-publish"\n\n'
  printf -- '--harness-publish\n'
  printf 'Content-Type: text/plain; charset=utf-8\n\n'
  printf 'The AGM date is attached. No reply needed.\n\n'
  printf -- '--harness-publish\n'
  printf 'Content-Type: text/calendar; charset=utf-8; method=PUBLISH; name="agm.ics"\n'
  printf 'Content-Disposition: attachment; filename="agm.ics"\n\n'
  printf 'BEGIN:VCALENDAR\n'
  printf 'VERSION:2.0\n'
  printf 'PRODID:-//Allodia//Harness Publish//EN\n'
  printf 'CALSCALE:GREGORIAN\n'
  printf 'METHOD:PUBLISH\n'
  printf 'BEGIN:VEVENT\n'
  printf 'UID:agm-0010@test.local\n'
  printf 'DTSTAMP:20260101T000000Z\n'
  printf 'DTSTART;VALUE=DATE:%s\n' "$INVITE_MONDAY"
  # DTEND is exclusive: a one-day event ends on the NEXT day.
  printf 'DTEND;VALUE=DATE:%s\n' "$INVITE_TUESDAY"
  printf 'SUMMARY:Annual general meeting\n'
  printf 'END:VEVENT\n'
  printf 'END:VCALENDAR\n\n'
  printf -- '--harness-publish--\n'
} >"$tmp"
imap_append "$tmp" INBOX
rm -f "$tmp"

log "appending an iMIP invitation on the EMPTY weekend day (the zero-conflict preview fixture)"
tmp=$(mktemp)
{
  printf 'From: Bob Tester <bob@test.local>\n'
  printf 'To: Alice Tester <alice@test.local>\n'
  printf 'Subject: Weekend walk\n'
  printf 'Message-ID: <invite-free-%s@test.local>\n' "$INVITE_SATURDAY"
  # Between the PUBLISH fixture and the Monday invitation, so neither of those moves off the top of
  # the inbox: a developer opening the app to look at a card should still land on the same row.
  printf 'Date: Wed, 21 Jan 2026 07:30:00 +0000\n'
  printf 'MIME-Version: 1.0\n'
  printf 'Content-Type: multipart/alternative; boundary="harness-imip-free"\n\n'
  printf -- '--harness-imip-free\n'
  printf 'Content-Type: text/plain; charset=utf-8\n\n'
  printf 'Fancy a walk on Saturday morning? Nothing else is in the diary.\n\n'
  printf -- '--harness-imip-free\n'
  printf 'Content-Type: text/calendar; charset=utf-8; method=REQUEST\n'
  printf 'Content-Transfer-Encoding: 8bit\n\n'
  printf 'BEGIN:VCALENDAR\n'
  printf 'VERSION:2.0\n'
  printf 'PRODID:-//Allodia//Harness iMIP//EN\n'
  printf 'CALSCALE:GREGORIAN\n'
  printf 'METHOD:REQUEST\n'
  printf 'BEGIN:VEVENT\n'
  printf 'UID:%s\n' "$INVITE_FREE_UID"
  printf 'DTSTAMP:20260101T000000Z\n'
  # Floating-free mid-morning: far from anything a developer is likely to scratch onto the weekend
  # by hand while testing the event editor, which would otherwise turn the zero into a one.
  printf 'DTSTART;TZID=Europe/Amsterdam:%sT110000\n' "$INVITE_SATURDAY"
  printf 'DTEND;TZID=Europe/Amsterdam:%sT120000\n' "$INVITE_SATURDAY"
  printf 'SUMMARY:Weekend walk\n'
  printf 'LOCATION:Amsterdamse Bos\n'
  printf 'DESCRIPTION:An hour in the woods.\n'
  printf 'ORGANIZER;CN=Bob Tester:mailto:bob@test.local\n'
  printf 'ATTENDEE;CN=Bob Tester;ROLE=CHAIR;PARTSTAT=ACCEPTED;RSVP=FALSE:mailto:bob@test.local\n'
  printf 'ATTENDEE;CN=Alice Tester;ROLE=REQ-PARTICIPANT;PARTSTAT=NEEDS-ACTION;RSVP=TRUE:mailto:alice@test.local\n'
  printf 'END:VEVENT\n'
  printf 'END:VCALENDAR\n\n'
  printf -- '--harness-imip-free--\n'
} >"$tmp"
imap_append "$tmp" INBOX
rm -f "$tmp"

# ---------------------------------------------------------------------------
# Optional bulk seed (dev only; OFF in CI).
#
# SEED_BULK=1 seeds SEED_BULK_COUNT (default 60) extra, varied messages into
# dedicated dev-only mailboxes (Lists / Newsletters / Bulk / DeepThread), so a
# developer debugging the app sees a fuller, real-feeling mailbox: read/unread,
# flagged, HTML newsletters, attachments, and a deep thread. It is fully
# index-derived (no randomness, no `date` arithmetic), pure printf/curl, and
# never touches the count-asserted INBOX/Archive/Projects/QResync/Idle fixtures,
# so CI's dataset (SEED_BULK unset ⇒ 0) stays byte-for-byte the fixture above.
# ---------------------------------------------------------------------------
if [ "${SEED_BULK:-0}" = "1" ]; then
  BULK_COUNT="${SEED_BULK_COUNT:-60}"
  log "bulk seed: $BULK_COUNT messages into dev-only mailboxes (Lists/Newsletters/Bulk/DeepThread)"

  for mbox in Lists Newsletters Bulk DeepThread; do
    imap_cmd INBOX "CREATE $mbox" >/dev/null 2>&1 || true
    imap_clear "$mbox"
  done

  # Fixed sender + date pools, cycled by index, deterministic, no randomness.
  bulk_sender() {
    case $(( $1 % 8 )) in
      0) echo "newsletter@updates.test" ;;
      1) echo "team@projects.test" ;;
      2) echo "no-reply@lists.test" ;;
      3) echo "billing@shop.test" ;;
      4) echo "digest@news.test" ;;
      5) echo "bob@test.local" ;;
      6) echo "notifications@social.test" ;;
      *) echo "support@service.test" ;;
    esac
  }
  # UTC (+0000) dates only: the engine's JMAP adapter expects `sentAt` as a UTC instant, so a
  # non-zero offset (which Stalwart would surface as +01:00, not Z) fails the sync. Matches the
  # fixed fixtures above.
  bulk_date() {
    case $(( $1 % 6 )) in
      0) echo "Mon, 06 Jan 2025 09:15:00 +0000" ;;
      1) echo "Tue, 14 Jan 2025 13:42:00 +0000" ;;
      2) echo "Wed, 22 Jan 2025 18:03:00 +0000" ;;
      3) echo "Thu, 06 Feb 2025 07:58:00 +0000" ;;
      4) echo "Fri, 21 Feb 2025 21:30:00 +0000" ;;
      *) echo "Sat, 08 Mar 2025 11:11:00 +0000" ;;
    esac
  }

  # Every message is written LF-only; imap_append CRLF-ifies it on upload.
  i=1
  while [ "$i" -le "$BULK_COUNT" ]; do
    sender=$(bulk_sender "$i")
    date=$(bulk_date "$i")
    msgid="bulk-$i@test.local"
    tmp=$(mktemp)
    if [ $(( i % 5 )) -eq 0 ]; then
      # HTML newsletter (multipart/alternative) → Newsletters.
      mbox="Newsletters"
      {
        printf 'From: %s\n' "$sender"
        printf 'To: alice@test.local\n'
        printf 'Subject: Newsletter #%s - what is new\n' "$i"
        printf 'Message-ID: <%s>\n' "$msgid"
        printf 'Date: %s\n' "$date"
        printf 'MIME-Version: 1.0\n'
        printf 'Content-Type: multipart/alternative; boundary="b-%s"\n\n' "$i"
        printf -- '--b-%s\n' "$i"
        printf 'Content-Type: text/plain; charset=utf-8\n\n'
        printf 'Issue %s of the test newsletter (plain-text part).\n\n' "$i"
        printf -- '--b-%s\n' "$i"
        printf 'Content-Type: text/html; charset=utf-8\n\n'
        printf '<html><body><h1>Newsletter #%s</h1><p>HTML part for rendering tests.</p></body></html>\n\n' "$i"
        printf -- '--b-%s--\n' "$i"
      } >"$tmp"
    elif [ $(( i % 7 )) -eq 0 ]; then
      # Small attachment (multipart/mixed) → Bulk.
      mbox="Bulk"
      {
        printf 'From: %s\n' "$sender"
        printf 'To: alice@test.local\n'
        printf 'Subject: Report #%s\n' "$i"
        printf 'Message-ID: <%s>\n' "$msgid"
        printf 'Date: %s\n' "$date"
        printf 'MIME-Version: 1.0\n'
        printf 'Content-Type: multipart/mixed; boundary="m-%s"\n\n' "$i"
        printf -- '--m-%s\n' "$i"
        printf 'Content-Type: text/plain; charset=utf-8\n\n'
        printf 'Message %s with a small CSV attachment.\n\n' "$i"
        printf -- '--m-%s\n' "$i"
        printf 'Content-Type: text/csv; name="data-%s.csv"\n' "$i"
        printf 'Content-Disposition: attachment; filename="data-%s.csv"\n' "$i"
        printf 'Content-Transfer-Encoding: base64\n\n'
        printf 'aWQsdmFsdWUKMSxhbHBoYQoyLGJldGEK\n\n'
        printf -- '--m-%s--\n' "$i"
      } >"$tmp"
    else
      # Plain text → Lists.
      mbox="Lists"
      {
        printf 'From: %s\n' "$sender"
        printf 'To: alice@test.local\n'
        printf 'Subject: Message %s about the mailing list\n' "$i"
        printf 'Message-ID: <%s>\n' "$msgid"
        printf 'Date: %s\n' "$date"
        printf 'Content-Type: text/plain; charset=utf-8\n\n'
        printf 'Body of bulk message %s. Lorem ipsum for the list view.\n' "$i"
      } >"$tmp"
    fi
    imap_append "$tmp" "$mbox" || true
    rm -f "$tmp"
    i=$((i + 1))
  done

  # Represent read/unread + flagged states: every 2nd \Seen, every 3rd \Flagged, per folder
  # (STORE on an absent sequence is tolerated, so this is safe for the smaller folders).
  for mbox in Lists Newsletters Bulk; do
    imap_cmd "$mbox" "STORE 2,4,6,8,10 +FLAGS (\\Seen)" >/dev/null 2>&1 || true
    imap_cmd "$mbox" "STORE 3,6,9 +FLAGS (\\Flagged)" >/dev/null 2>&1 || true
  done

  # A deep, References-chained conversation in DeepThread (root + 8 replies).
  log "bulk seed: building a deep conversation in DeepThread"
  root_id="deep-root@test.local"
  tmp=$(mktemp)
  {
    printf 'From: %s\n' "$(bulk_sender 0)"
    printf 'To: alice@test.local\n'
    printf 'Subject: Deep conversation\n'
    printf 'Message-ID: <%s>\n' "$root_id"
    printf 'Date: %s\n' "$(bulk_date 0)"
    printf 'Content-Type: text/plain; charset=utf-8\n\n'
    printf 'The root message of a deep thread.\n'
  } >"$tmp"
  imap_append "$tmp" DeepThread || true
  rm -f "$tmp"
  refs="<$root_id>"
  prev="$root_id"
  j=1
  while [ "$j" -le 8 ]; do
    mid="deep-$j@test.local"
    tmp=$(mktemp)
    {
      printf 'From: %s\n' "$(bulk_sender "$j")"
      printf 'To: alice@test.local\n'
      printf 'Subject: Re: Deep conversation\n'
      printf 'Message-ID: <%s>\n' "$mid"
      printf 'Date: %s\n' "$(bulk_date "$j")"
      printf 'In-Reply-To: <%s>\n' "$prev"
      printf 'References: %s\n' "$refs"
      printf 'Content-Type: text/plain; charset=utf-8\n\n'
      printf 'Reply number %s in the deep thread.\n' "$j"
    } >"$tmp"
    imap_append "$tmp" DeepThread || true
    rm -f "$tmp"
    refs="$refs <$mid>"
    prev="$mid"
    j=$((j + 1))
  done
  log "bulk seed complete"
fi

log "content seed complete"
