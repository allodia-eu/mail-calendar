---
name: inspect-mail-store
description: Read a client's engine store (the mailcal.sqlite database) read-only to see what the engine actually persisted: thread ids, message headers, folder membership, row counts. Use when the UI and your mental model disagree, to decide whether a bug is in the sync/engine layer or in the view-model/client, before changing any code. Wraps scripts/dev/store.sh; works against the harness store or a personal one, on Windows/macOS/Linux/Android/simulator.
---

# inspect-mail-store: ask the store, not the UI

The clients render a projection of what the engine wrote to `mailcal.sqlite`. When the screen
looks wrong, the first question is **which layer is lying**: did the engine persist bad data, or is
the view-model rendering good data badly? The store answers that in seconds, and it answers it
without a debugger, without a rebuild, and without trusting the code you're about to suspect.

Use it **before** you start reading Rust. A wrong `thread_id` on disk means no amount of
view-model archaeology will help.

## Commands (all via `scripts/dev/store.sh`)

| Command | What it does |
|---|---|
| `scripts/dev/store.sh path` | Resolve + print the database path for this client. |
| `scripts/dev/store.sh tables` | List the tables. |
| `scripts/dev/store.sh schema <table>` | Columns of one table. |
| `scripts/dev/store.sh sql "<SELECT …>"` | Run a read-only query. Refuses anything but `SELECT`/`WITH`/`PRAGMA`/`EXPLAIN`. |
| `scripts/dev/store.sh threads [subject-substring]` | Thread grouping report: per message its `thread_id`, own `Message-ID`, and referenced ids. |

Flags: `--platform windows\|macos\|linux\|android\|iphone\|ipad` (default: the host) and
`--store real\|dev\|dev-imap` (default: `real`). Each dev account gets an isolated store, so
`--store dev` is the harness mailbox and never your own mail.

## It is read-only, by construction

Every command copies the store aside first (the main file **plus `-wal` and `-shm`**, because a
running app keeps recent commits in the write-ahead log and the main file alone can be hours
stale), then opens the copy `mode=ro` and deletes it on exit. You can run it against a live app.
Never point `sqlite3` at the real file yourself: a stray write, or even an incidental WAL
checkpoint, corrupts the app's view of its own store.

## Privacy

`--store real` is the developer's actual mailbox. `threads` prints headers only (subject,
`Message-ID`, `thread_id`), never bodies. Ad-hoc `sql` prints whatever you ask for, so **read the
output before pasting it into an issue, a PR, or a commit message**. When a bug reproduces on the
harness, prefer `--store dev` and share freely. See `mail-harness` to seed one.

## The layout worth knowing

| Table | Holds |
|---|---|
| `object` | One row per synced object; `payload` is the JSON-serialized `Message`/`Event` (envelope, headers, flags). |
| `mail_index` | The projected mail row: `thread_id`, `date_utc`, `has_attachment`, keyed by `(scope_key, provider_key)`. |
| `membership` | Which mailbox/folder each object belongs to. |
| `sync_scope` | Per-scope sync cursors. |
| `pending_op` | The durable outbox (queued sends/flag changes). |

A message's identity is `(scope_key, provider_key)`; `scope_key` embeds the account and folder.

## Worked example: "why is this conversation split into two rows?"

A notification thread from a forge is the shape this happens on, because every comment carries a
`References` chain back to one discussion. The transcript below is illustrative (the ids are made
up, not a capture of anyone's mailbox), but the report's shape and the reasoning after it are real.

```
scripts/dev/store.sh threads "build failed"
```

```
9 message(s) in 3 thread(s):
    7  …/comments/1000001@forge.example
    1  …/comments/1000002@forge.example
    1  …/comments/1000003@forge.example

SPLIT: these referenced ids appear in more than one thread -
the derivation pass never saw those messages together (engine-sync::threading).
  …/discussions/2001@forge.example  ->  3 threads
```

That settles it without reading a line of view-model code. The clients group a list row by
`(account, thread_id)` (`mailcal-viewmodel::view_rows::build_threaded`), so messages with different
`thread_id`s **cannot** render as one conversation. The bug is upstream, in the engine's thread
derivation: the `SPLIT` section names the referenced id that should have united them.

The general move: find a field the UI depends on, read it from the store, and see whether the UI is
faithfully rendering it. If it is, stop looking at the client.

## When to reach for it

- A row renders as a single message when it should be a conversation (or vice-versa) → `threads`.
- A message shows in the wrong folder, or in none → query `membership`.
- "Refresh re-downloads everything" → look at `sync_scope` cursors.
- A row's date/attachment flag/subject looks wrong → compare `mail_index` against `object.payload`.
- Anything where you're about to add a `log::info!` to find out what the core stored. Just read it.

## Gotchas

- **No `sqlite3.exe` on Windows.** Neither Windows nor Git for Windows ships one. The script falls
  back to Python's bundled `sqlite3` module automatically; nothing to install.
- **Android needs a debuggable build.** The store lives inside the app sandbox and is read via
  `adb exec-out run-as eu.allodia.mailcal`. A release build refuses.
- **A dev store only exists once that account has run.** `--store dev` and `--store dev-imap` are
  separate databases, created on the first boot of `MAILCAL_DEV_ACCOUNT=stalwart` /
  `stalwart-imap` respectively; the script says so rather than printing a path that isn't there.
- **The store is not the client's state.** UI-only state (inline thread expansion, scroll position,
  selection) lives in the client and won't be here.

## Related

- `mail-harness`: seed a deterministic mailbox so the store you're reading is shareable.
- `debug-app`: boot a client and read its `app.log`; pair it with this when the log says the core
  built N rows and the screen shows something else.
