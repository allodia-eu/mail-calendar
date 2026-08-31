#!/bin/sh
# The living week: calendar fixtures anchored on TODAY, so the grid always opens on a week with
# something in it.
#
# The six fixtures in seed/calendar/ are pinned to absolute dates in early 2026 because they encode
# *engine* semantics that must not drift: floating time, a zoneless all-day, a DST boundary, a
# recurrence with an overridden instance. They are the deterministic CI fixture and they stay exactly
# as they are.
#
# But they are all in the past, and a grid seeded only with them opens **empty**. That is not a
# theoretical complaint: it cost an afternoon. Every UI change to the calendar had to be checked by
# swiping seventeen weeks backwards to reach March, or not checked at all, so the emulator, which is
# the only calendar most of us will ever debug against, showed a blank grid and taught us nothing.
#
# So this generates a second set, re-anchored on the current Monday at every seed. It is not trying to
# be a realistic diary. It is trying to hit every rule in docs/calendar.md that a client can get wrong,
# on the week the grid opens on:
#
#   - a 15-minute event          : a block too short to hold its own title, at most zooms (§4)
#   - three overlapping events   : the core's column packing, side by side (§1)
#   - back-to-back events        : the boundary between two blocks
#   - an event spanning the day  : 09:00-18:00, taller than the viewport
#   - a single all-day           : a zoneless date, exclusive end (§1). One day wide, not two.
#   - a three-day offsite        : a band spanning columns, and hidden on EVERY day it covers (§4)
#   - five all-day events on one day: more lanes than the banner shows, so the "+N" cap and its
#                                      per-column counts are actually exercised (§4)
#   - events in the weeks either side: so a swipe lands on something, and the page cache is real
#   - an event this afternoon    : so the now line has company
#
# Times are Europe/Amsterdam with an embedded VTIMEZONE, like the pinned fixtures: a harness whose
# events are all floating would never catch a zone-conversion bug.
set -eu

CAL_COLLECTION="$1" # the CalDAV collection URI
AUTH="$2"           # user:password

# This week's Monday, in UTC. `%u` is 1 (Mon) .. 7 (Sun).
MONDAY=$(date -u -d "-$(($(date -u +%u) - 1)) days" +%Y-%m-%d)

day() { date -u -d "$MONDAY +$1 days" +%Y%m%d; }

VTIMEZONE='BEGIN:VTIMEZONE
TZID:Europe/Amsterdam
BEGIN:DAYLIGHT
TZOFFSETFROM:+0100
TZOFFSETTO:+0200
TZNAME:CEST
DTSTART:19700329T020000
RRULE:FREQ=YEARLY;BYMONTH=3;BYDAY=-1SU
END:DAYLIGHT
BEGIN:STANDARD
TZOFFSETFROM:+0200
TZOFFSETTO:+0100
TZNAME:CET
DTSTART:19701025T030000
RRULE:FREQ=YEARLY;BYMONTH=10;BYDAY=-1SU
END:STANDARD
END:VTIMEZONE'

put() { # uid  body
  printf '%s' "$2" | sed 's/$/\r/' | curl -sk --user "$AUTH" \
    -X PUT -H 'Content-Type: text/calendar; charset=utf-8' \
    --data-binary @- "$CAL_COLLECTION/$1.ics" >/dev/null
}

timed() { # uid  summary  day-offset  HHMM-start  HHMM-end
  d=$(day "$3")
  put "$1" "BEGIN:VCALENDAR
VERSION:2.0
PRODID:-//Allodia//Harness Living Week//EN
CALSCALE:GREGORIAN
$VTIMEZONE
BEGIN:VEVENT
UID:$1@test.local
DTSTAMP:20260101T000000Z
DTSTART;TZID=Europe/Amsterdam:${d}T$400
DTEND;TZID=Europe/Amsterdam:${d}T$500
SUMMARY:$2
END:VEVENT
END:VCALENDAR"
}

allday() { # uid  summary  day-offset  span-days
  start=$(day "$3")
  end=$(day "$(($3 + $4))") # DTEND is EXCLUSIVE: a one-day event ends on the NEXT day (§1)
  put "$1" "BEGIN:VCALENDAR
VERSION:2.0
PRODID:-//Allodia//Harness Living Week//EN
CALSCALE:GREGORIAN
BEGIN:VEVENT
UID:$1@test.local
DTSTAMP:20260101T000000Z
DTSTART;VALUE=DATE:$start
DTEND;VALUE=DATE:$end
SUMMARY:$2
END:VEVENT
END:VCALENDAR"
}

# --- Monday: a dense morning, and the overlap the core has to pack into columns ----------------
timed week-standup      "Standup"                    0 0900 0915  # 15 min: too short for a title
timed week-review       "Design review"              0 1000 1100
timed week-onetoone     "1:1 with Sam"               0 1000 1030  # overlaps the review...
timed week-triage       "Bug triage"                 0 1015 1115  # ...and so does this: three columns
timed week-lunch        "Lunch"                      0 1200 1300
timed week-retro        "Retro"                      0 1300 1400  # back-to-back with lunch

# --- Tuesday: the all-day banner, past its cap -------------------------------------------------
# Five lanes where the banner shows three, so the "+N" chip and its PER-COLUMN counts are real.
allday week-holiday     "Public holiday"             1 1
allday week-oncall      "On call"                    1 1
allday week-birthday    "Ada's birthday"             1 1
allday week-deadline    "Tax deadline"               1 1
allday week-leave       "Sam on leave"               1 1
timed week-1500         "Customer call"              1 1500 1600

# --- Wednesday–Friday: a band that spans its days, and is hidden on every one of them -----------
allday week-offsite     "Offsite (3 days)"           2 3
timed week-workshop     "Workshop"                   2 1000 1200
timed week-allday-long  "Hack day"                   3 0900 1800  # taller than the viewport
timed week-demo         "Demo"                       4 1600 1630

# --- The two participation states a client has to draw differently (docs/invitations.md) ---------
# What a CalDAV/Exchange/Google auto-schedule server puts on the calendar the moment an invitation
# arrives: the event, with the invitee's own PARTSTAT. Both are needed, and neither can be faked with
# the pinned meeting-attendees.ics fixture, where alice is CHAIR;ACCEPTED.
#
#   NEEDS-ACTION → an unanswered hold: dashed border, hatched gutter, and a spoken "Awaiting your
#                  response". It lands inside Monday's review/triage overlap on purpose, so the
#                  matching invitation email (seeded in seed.sh) has a non-zero conflict count and a
#                  preview grid with something in it.
#   DECLINED     → must NOT appear on the grid, the month or the agenda at all. The core hides it
#                  (one rule, four disagreeing providers), so its absence is the assertion, and a
#                  rule whose only evidence is a unit test is a rule nothing exercises end to end.
invited() { # uid  summary  day-offset  HHMM-start  HHMM-end  partstat
  d=$(day "$3")
  put "$1" "BEGIN:VCALENDAR
VERSION:2.0
PRODID:-//Allodia//Harness Living Week//EN
CALSCALE:GREGORIAN
$VTIMEZONE
BEGIN:VEVENT
UID:$1@test.local
DTSTAMP:20260101T000000Z
DTSTART;TZID=Europe/Amsterdam:${d}T$400
DTEND;TZID=Europe/Amsterdam:${d}T$500
SUMMARY:$2
ORGANIZER;CN=Bob Tester:mailto:bob@test.local
ATTENDEE;CN=Bob Tester;ROLE=CHAIR;PARTSTAT=ACCEPTED;RSVP=FALSE:mailto:bob@test.local
ATTENDEE;CN=Alice Tester;ROLE=REQ-PARTICIPANT;PARTSTAT=$6;RSVP=TRUE:mailto:alice@test.local
ATTENDEE;CN=Carol External;ROLE=REQ-PARTICIPANT;PARTSTAT=ACCEPTED;RSVP=TRUE:mailto:carol@example.com
END:VEVENT
END:VCALENDAR"
}
invited week-invited  "Quarterly planning" 0 1030 1130 NEEDS-ACTION
invited week-declined "Vendor pitch"       0 1600 1700 DECLINED

# --- Repeating events, because a summary of a rule needs a rule (§10) ---------------------------
# The living week had none, so every repeat surface (the summary sentence, the editor, the
# "this event or all of them?" question on a save and a delete) had nothing to open against. Each
# of these is a shape the sentence states differently, and the fortnightly one is the case the whole
# structured rule exists for: a frequency word alone calls it weekly.
repeating() { # uid  summary  day-offset  HHMM-start  HHMM-end  rrule
  d=$(day "$3")
  put "$1" "BEGIN:VCALENDAR
VERSION:2.0
PRODID:-//Allodia//Harness Living Week//EN
CALSCALE:GREGORIAN
$VTIMEZONE
BEGIN:VEVENT
UID:$1@test.local
DTSTAMP:20260101T000000Z
DTSTART;TZID=Europe/Amsterdam:${d}T$400
DTEND;TZID=Europe/Amsterdam:${d}T$500
SUMMARY:$2
RRULE:$6
END:VEVENT
END:VCALENDAR"
}
repeating week-repeat-weekly    "Team sync"        0 1130 1200 "FREQ=WEEKLY;BYDAY=MO,WE"
repeating week-repeat-fortnight "Sprint planning"  1 0930 1030 "FREQ=WEEKLY;INTERVAL=2"
repeating week-repeat-monthly   "Board meeting"    2 1500 1630 "FREQ=MONTHLY;BYDAY=1WE"
repeating week-repeat-until     "Onboarding"       3 1300 1330 "FREQ=DAILY;COUNT=10"

# --- Today, this afternoon: so the now line has company ------------------------------------------
TODAY_OFFSET=$(($(date -u +%u) - 1))
timed week-today-pm     "Afternoon sync"             "$TODAY_OFFSET" 1400 1500

# --- The weeks either side, so a swipe lands on something rather than a blank grid ---------------
prev() { date -u -d "$MONDAY -7 days +$1 days" +%Y%m%d; }
next() { date -u -d "$MONDAY +7 days +$1 days" +%Y%m%d; }
put_at() { # uid summary yyyymmdd HHMM HHMM
  put "$1" "BEGIN:VCALENDAR
VERSION:2.0
PRODID:-//Allodia//Harness Living Week//EN
CALSCALE:GREGORIAN
$VTIMEZONE
BEGIN:VEVENT
UID:$1@test.local
DTSTAMP:20260101T000000Z
DTSTART;TZID=Europe/Amsterdam:$3T$400
DTEND;TZID=Europe/Amsterdam:$3T$500
SUMMARY:$2
END:VEVENT
END:VCALENDAR"
}
put_at week-prev-1 "Last week's planning" "$(prev 1)" 1100 1200
put_at week-prev-2 "Last week's demo"     "$(prev 3)" 1500 1530
put_at week-next-1 "Next week's kickoff"  "$(next 0)" 0930 1030
put_at week-next-2 "Next week's review"   "$(next 2)" 1400 1500
