---
name: mail-harness
description: Start, stop, reset, reseed, and inspect the local seeded Stalwart mail/calendar test server the app is debugged against. Use before or around driving any client so the app has a deterministic local mailbox instead of a personal account. Requires Docker. Wraps docker/stalwart via scripts/dev/harness.sh.
---

# mail-harness: the local Stalwart test server

A reproducible [Stalwart](https://stalw.art) mail/calendar server in Docker, seeded with a
deterministic dataset, running on loopback with throwaway credentials. It is the **default target
for debugging**. Use a personal mailbox only when the user explicitly asks for it, for example
because a specific real message/provider behaviour triggers the bug. It's the same harness CI uses
(`.github/workflows/ci.yml`); full detail in `docker/stalwart/README.md`.

## Commands (all via `scripts/dev/harness.sh`)

| Command | What it does |
|---|---|
| `scripts/dev/harness.sh up` | Start + seed; blocks until healthy. Idempotent. |
| `scripts/dev/harness.sh up --bulk` | Also seed dozens of extra messages (varied states/folders) for a fuller, real-feeling mailbox. |
| `scripts/dev/harness.sh status` | Health + the host-port table + seeded accounts. |
| `scripts/dev/harness.sh logs [-f]` | The server's own logs (seeding, requests). |
| `scripts/dev/harness.sh reset [--bulk]` | Wipe volumes and re-bootstrap from empty (clean slate), **and clear this host's client dev stores with it**, because the new server reuses the old ids (`--keep-clients` opts out; see below). |
| `scripts/dev/harness.sh test` | Run the gated JMAP live test against it (a fast smoke check). |
| `scripts/dev/harness.sh down` | Stop and remove the container. |

## What you get

- **Ports (loopback only):** JMAP + CalDAV + admin `http://127.0.0.1:28080`, SMTP `127.0.0.1:12025`
  (plaintext), IMAP `127.0.0.1:12993` (implicit TLS, self-signed).
  **The `12xxx`/`28080` block is this repo's; `11xxx`/`18080` is the *engine* repo's harness.** Both
  repos ship a `docker/stalwart/docker-compose.yml`, and Compose keys a project on its `name:` rather
  than on the file's path, so before they were separated, `up` in either repo silently adopted the
  other's container **and volumes** and re-seeded it. The container stays healthy and the ports keep
  answering; only the data is wrong, so a fixture you just added is simply absent. If a seed you added
  is missing, check `docker ps` for the *other* project name before you debug anything else. Full
  reasoning: [`../../../docker/stalwart/README.md`](../../../docker/stalwart/README.md) → "Why these
  are not the engine's numbers".
- **Seeded account:** `alice@test.local` / `harness-alice-pw` (mail + calendar). Also
  `bob@test.local` / `harness-bob-pw` and `admin` / `harness-admin-pw`.
- **Seed content:** threads, an attachment, flagged/read states, duplicate/missing Message-IDs,
  and calendar events (recurring + exceptions, attendees, all-day). `--bulk` adds a larger volume
  across dev-only folders (`Lists`, `Newsletters`, `Bulk`, `DeepThread`).

## Usage notes

- Bring the harness **up first**, then use the **debug-app** skill (or `scripts/dev/boot.sh`) to
  launch a client against it: the client defaults to this harness.
- Android boot tooling maps device/emulator localhost back to the host harness with `adb reverse`,
  so the dev account can use `127.0.0.1` consistently on emulators and physical devices.
- These credentials are throwaway loopback test fixtures, safe to hardcode and log; they never
  touch real data.
- **A reset invalidates every client store, and the client cannot tell.** Stalwart mints its ids
  deterministically from an empty database, so the server that comes back hands out the *same* ids
  for a *different* set of messages, and a client that kept its cached bodies then opens somebody
  else's body for every message, with somebody else's attachments and no invitation part. Nothing
  errors; the app renders what it was given, so it reads as a product bug and has been mistaken for
  one. `reset` therefore clears this host's dev stores too. Two consequences worth knowing: **close
  the app first** (a running one holds the store open, and the script warns rather than half-clearing
  it), and the next launch meets the **first-run usage-statistics screen**, which the UI suite settles
  by itself. A store on a phone or simulator is out of the script's reach: reinstall or clear its
  data there. Detail: [`docker/stalwart/README.md`](../../../docker/stalwart/README.md).
