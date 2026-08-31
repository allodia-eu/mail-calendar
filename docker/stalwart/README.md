# Stalwart JMAP test harness

A reproducible, deterministic [Stalwart](https://stalw.art) server in Docker,
seeded with **one shared dataset** (JMAP, IMAP, SMTP, CalDAV). In this repo it is
the real JMAP provider the product-core **JMAP account** path is tested against,
locally and in CI (`.github/workflows/ci.yml`). It is lifted from the engine's
harness of the same name; the engine's design doc (bootstrap flow, per-fixture
invariants, determinism rules) is the authoritative reference. See
[`docs/jmap.md`](../../docs/jmap.md) for how the product connects a JMAP account.

This is **test infrastructure**, not a product. It runs on loopback with
throwaway credentials and never holds real data.

## Run it

One self-bootstrapping service. It completes Stalwart v0.16's first-run setup
through the management API, creates the accounts, and seeds the dataset inside
its entrypoint, then reports healthy once seeding is done:

```sh
cd docker/stalwart
docker compose up -d --wait   # self-bootstrap + seed; healthy == ready
```

Then run the gated JMAP live test against it (from the repo root):

```sh
export STALWART_HTTP_ADDR=127.0.0.1:28080
export STALWART_ACCOUNT=alice@test.local
export STALWART_PASSWORD=harness-alice-pw
cargo test -p mailcal-account --test live_jmap -- --nocapture
```

To drive the **macOS app** against it, add a JMAP account in the setup form with
server `http://127.0.0.1:28080`, email `alice@test.local`, password
`harness-alice-pw` (JMAP is the only kind that accepts a plaintext-`http://`
server, for exactly this loopback fixture).

Reset to a clean slate, **through the script, not compose directly**:

```sh
scripts/dev/harness.sh reset          # add --keep-clients to leave the client stores alone
```

### A reset invalidates every client store, and nothing says so on its own

Stalwart mints its ids deterministically from an empty database, so the server
that comes back up hands out **the same ids for a different set of messages**:
the seed has grown since the store synced. A client that keeps its cached
bodies under those ids then opens somebody else's body for every message, with
somebody else's attachments and no invitation part in the ones that should have
one. Nothing errors and nothing looks broken: the app renders exactly what it
was handed.

It reads as a product bug, and it has been mistaken for one: a round of "the
invitation card is broken on Windows" that was a nine-day-old body cache. The
store settles it in one query:

```sh
scripts/dev/store.sh sql "SELECT m.subject, substr(b.plain,1,50) FROM message m   JOIN message_body b ON b.provider_key=m.provider_key AND b.account=m.account" --store dev
```

If those two columns disagree, the store is stale. `harness.sh reset` now clears
this host's client dev stores with the server, which is what makes "clean slate"
true on both sides; a store on a phone or simulator is out of its reach, so
reinstall the app there or clear its data. `docker compose down -v` on its own
does **not** do this: that is the whole reason to go through the script.

CI always starts fresh, so it never meets any of it.

Without `STALWART_HTTP_ADDR` set, every Stalwart-touching test **skips**, so
`cargo test --workspace` stays green with no Docker.

## Debugging a client against this harness

The `scripts/dev/*` tooling (and the `mail-harness` / `debug-app` Claude Code skills) wrap this
harness and boot any client against it, with no manual account setup:

```sh
scripts/dev/harness.sh up            # start + seed (this compose file)
scripts/dev/harness.sh up --bulk     # also seed dozens of extra varied messages (SEED_BULK)
scripts/dev/boot.sh macos            # boot a client against the harness (also iphone|ipad|android)
```

The client injects a canned harness account at boot (debug builds only), into its own engine
store, so it never touches your real accounts. Full loop (logs, screenshots, control) is in
[`../../docs/debugging.md`](../../docs/debugging.md).

**Bulk seed.** `--bulk` sets `SEED_BULK=1` (count `SEED_BULK_COUNT`, default 60), which seeds extra
index-derived messages (plain / HTML newsletters / attachments / a deep thread, with read/flagged
states) into dedicated dev-only mailboxes (`Lists`, `Newsletters`, `Bulk`, `DeepThread`). It never
touches the count-asserted CI fixtures, so CI (`SEED_BULK` unset ⇒ 0) is unaffected.

**IMAP fidelity (`stalwart-imap`).** For full mail-action + IDLE testing, `scripts/dev/boot.sh
<platform> --account stalwart-imap` drives the harness over IMAP (implicit TLS on 12993). Since
that listener serves a **self-signed** cert (`CN=rcgen self signed cert`, SAN `localhost`),
`harness.sh up` extracts it to `docker/stalwart/tls/harness-ca.pem` (gitignored) and `boot.sh`
delivers it so the **dev-only** trust path in the core adds it as an extra anchor. Standard
verification otherwise unchanged. Dial `127.0.0.1:12993` (Android emulator `10.0.2.2:12993`) with
`server_name = localhost`. Supported on **every** platform: macOS, the iOS/iPad simulators,
Windows, and the Android emulator (see [`../../docs/debugging.md`](../../docs/debugging.md)).

## Host ports (loopback only)

| Protocol                      | Container | Host    | Transport          |
| ----------------------------- | --------- | ------- | ------------------ |
| HTTP: JMAP + CalDAV + admin   | 8080      | `28080` | plaintext          |
| SMTP                          | 25        | `12025` | plaintext          |
| SMTP submission               | 465       | `12465` | implicit TLS (self-signed) |
| SMTP submission               | 587       | `12587` | STARTTLS (self-signed), entrypoint-provisioned |
| IMAP                          | 993       | `12993` | implicit TLS (self-signed) |
| IMAP                          | 143       | `12143` | STARTTLS (self-signed), entrypoint-provisioned |

### Why these are not the engine's numbers

**This harness must not share a compose project name or a single host port with the engine repo's.**
Both repos ship a `docker/stalwart/docker-compose.yml`, and they began as copies of each other, so
both declared `name: stalwart-harness` and both bound `18080` / `11025` / `11465` / `11587` / `11993`
/ `11143`. **Compose keys a project on that name, not on the file's path**, so `docker compose up` in
either repo adopted the *same* container and the *same* named volumes and recreated it with its own
mounts.

That fails **silently**, which is what makes it expensive: the container stays healthy, every port
keeps answering, and the seeded account logs in, but the dataset is the other repo's. A fixture you
just added is simply missing, and `docker logs` narrates seed steps your `seed.sh` does not contain.

So: **`11xxx` is the engine, `12xxx` is this repo**; HTTP is `18080` there and `28080` here, and
`18081` is already the engine's SabreDAV harness (do not reuse it). The project name here is
`mailcal-core-harness`. A third harness takes `13xxx` / `38080`. The numbers appear in
[`docker-compose.yml`](docker-compose.yml), [`../../scripts/dev/lib.sh`](../../scripts/dev/lib.sh) and
the four clients' injected dev account: [`check-dev-account.sh`](../../scripts/ci/check-dev-account.sh)
fails the build if those drift apart, so changing one means changing all of them.

## Seeded accounts

Created at startup via Stalwart's management API (v0.16 has no declarative config
file, see the design doc).

| Account            | Password           | Role                          |
| ------------------ | ------------------ | ----------------------------- |
| `alice@test.local` | `harness-alice-pw` | primary (mail + calendar)     |
| `bob@test.local`   | `harness-bob-pw`   | second party / event attendee |
| `admin`            | `harness-admin-pw` | fallback admin (management)   |

## Layout

```text
docker/stalwart/
├── docker-compose.yml      # single service (image pinned by digest)
├── entrypoint.sh           # self-bootstrap via API → restart → accounts → seed
├── seed.sh                 # curl: IMAP APPEND/STORE/COPY/MOVE + CalDAV PUT
├── seed-calendar-week.sh   # the living week, re-anchored on the current Monday at every seed
└── seed/
    ├── mail/*.eml          # messages: dup/missing Message-ID, attachment, HTML+remote image, …
    └── calendar/*.ics      # events: recurring+exceptions, attendees, …
```

## The living week

The six `seed/calendar/*.ics` fixtures are pinned to absolute dates in early 2026 because they encode
**engine** semantics that must not drift: floating time, a zoneless all-day, a DST boundary, a
recurrence with an overridden instance. They are the deterministic CI fixture and they stay.

They are also all in the past, so a grid seeded with only them opens **empty**, and an empty grid
teaches you nothing about a grid. `seed-calendar-week.sh` re-anchors a second set on the **current
Monday** at every seed, chosen to hit the rules a client can actually get wrong (docs/calendar.md):

| fixture | what it is there to break |
|---|---|
| a 15-minute standup | a block too short to hold its own title, at most zooms (§4) |
| three overlapping events | the core's column packing: ignore `column`/`columns` and one event hides behind another (§1) |
| back-to-back lunch/retro | the boundary between two blocks |
| a 09:00–18:00 hack day | a block taller than the viewport |
| a single all-day | a zoneless date with an **exclusive** end: get it wrong and it renders two days wide (§1) |
| a three-day offsite | a band spanning columns, hidden on **every** day it covers (§4) |
| five all-day events on one day | more lanes than the banner shows, so the "+N" cap and its **per-column** counts are real (§4) |
| events in the weeks either side | a swipe lands on content, and the page cache is exercised |
| an event this afternoon | the now line has company |
| a `NEEDS-ACTION` invitation, mid-morning Monday | an **unanswered hold**: dashed, hatched, and spoken as "Awaiting your response". It sits inside the review/triage overlap on purpose, so the matching invitation email has a real clash to report ([`invitations.md`](../../docs/invitations.md)) |
| a `DECLINED` invitation, Monday afternoon | it must **not** be on the grid, the month or the agenda. The absence is the assertion, and a rule whose only evidence is a unit test is a rule nothing exercises end to end |

Two of those carry a second job, and changing them breaks a test that is nowhere near them.
`clients/windows/uitests/Attendees.Tests.ps1` opens the **`NEEDS-ACTION` invitation** ("Quarterly
planning") for its attendee roster: it is seeded with three participants whose answers deliberately
differ (Bob organising and accepted, Alice unanswered, Carol accepted), and the suite reads the
answers *against each other* rather than by name, so two attendees seeded to the same `PARTSTAT` are
load-bearing. It also opens **Lunch** as the control: an event with no `ATTENDEE` line at all, which
is what proves the roster heading stays away rather than appearing empty. Give Lunch an attendee, or
flatten those three `PARTSTAT`s to one value, and the suite stops discriminating anything.

## Meeting invitations

Two messages are appended at the **end** of `seed.sh`, and they are the contrast the "why does an
invitation sometimes have an .ics file" rule turns on ([`invitations.md`](../../docs/invitations.md)):

| message | shape | must produce |
|---|---|---|
| "Quarterly planning", from bob | `text/calendar; method=REQUEST` as an alternative **body** part, no `Content-Disposition` at all (iMIP, RFC 6047 §2.4) | an invitation **card**, and **no** paperclip |
| "Annual general meeting" | the same media type, explicitly `Content-Disposition: attachment`, `METHOD:PUBLISH`, **zero** `ATTENDEE` lines | **no** card, and its `agm.ics` chip **kept** |
| "Weekend walk", from bob | the same iMIP shape as the first, on the **weekend day the living week leaves empty** | a card whose day preview is **open over an empty grid** |

Before the fix, the first two showed a junk `attachment-N.ics` row. Together they prove the
suppression is about the *disposition*, not the media type.

The third exists so that one rule can be **tested at all**. "The day preview opens expanded whenever
the calendar was read" replaced "opens only when the count is non-zero", and the Quarterly planning
invitation sits inside the Monday overlap, so its preview is open under *both* rules. A day with no
overlap is the only fixture that tells them apart; `clients/windows/uitests/InvitationPreview.Tests.ps1`
is built on it, and any other client's test would need it too.

The invitation is **generated** rather than a static `.eml`, because its `UID` and start have to agree
with the living week's `NEEDS-ACTION` hold above: that is what makes the card's conflict count exclude
its own event and its preview grid land on a day with something in it. A fixture pinned to 2026 would
draw a correct card over an empty day: the same "opens empty and teaches you nothing" problem the
living week exists to solve. The anonymized per-sender MIME shapes (Outlook's base64 + `iso-8859-1`,
Gmail's duplicate chip) belong to the unit suite: these two prove the transport, not the parser.

⚠️ They are appended **last** for the reason `08-html.eml` is: the `STORE`/`COPY`/`MOVE` steps above
address messages by IMAP **sequence number**, so an `APPEND` earlier in the file silently re-points
them at the wrong message.

## Everything you APPEND here arrives READ

`imap_append` (and `harness.sh deliver`, and any hand-rolled `curl --upload-file`) sends no flag list,
and what lands carries `$seen`. So **the entire seeded mailbox is read**, which is fine until you need
the opposite: a test about unread mail seeded this way asserts against a mailbox that has none, and
reads as a *correct* failure of the feature rather than of the fixture. That is not hypothetical: the
Windows conversation row bolded nothing on unread mail for as long as threading existed, in a repo
that ships a mail harness, because nothing here was ever unread.

To make a seeded message unread, clear the flag after appending: `*` is the highest sequence number,
i.e. the message you just added:

```sh
curl -sk --user "alice@test.local:harness-alice-pw" \
  --url "imaps://127.0.0.1:12993/INBOX" --request 'STORE * -FLAGS (\Seen)'
```

For UI work needing unread mail, prefer the **showcase** dataset (`MAILCAL_SHOWCASE=en`), whose seed
contains both read and unread messages by construction and needs no server at all.
