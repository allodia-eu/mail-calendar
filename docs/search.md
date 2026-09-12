# Mail search: cross-platform contract

**Scope.** What a mail search covers, in what order it shows what it found, and how a client
offers to narrow it. Search runs entirely **in the core** (a full-text query per account against
the local store, no network), so the semantics below are the same on every client by construction;
only the scope filter's UI is per-platform.

**Principle.** Search answers *"where is that message"*. A person looking for a message knows
roughly **when** it arrived and rarely which folder it ended up in, so search looks
**everywhere by default** and shows what it finds **newest first**.

## The rules

| # | Rule | Why |
|---|---|---|
| 1 | **Results are ordered newest first**, never by relevance. The tie-break is `(account, provider key)`, so the row sequence is total and identical across rebuilds. | A relevance order interleaves this morning's mail with a three-year-old thread and reads as no order at all. The engine's ranking still decides *which* hits are candidates; it does not decide the display order. |
| 2 | **The default scope is every account and every folder, except Trash.** | Outlook's "All mailboxes". A message the user threw away is not what they are looking for, and a deleted copy sitting beside the live one is noise. |
| 3 | **Trash stays reachable**: open it and narrow to the current folder (rule 4). The exclusion is a default, not a wall. | A folder you cannot search is a folder you cannot use. |
| 4 | A client offers a **two-way scope filter**: *this scope* or *all mail*. "This scope" **mirrors the mailbox list exactly**: the selected folder, or the selected account's whole mailbox (Trash included), or, in the unified view, every account's Inbox. | The narrowing has to mean what the user sees. In the unified all-inboxes view "this folder" is the set of inboxes on screen, not any one folder. |
| 5 | **Leaving search restores the view it was opened from**: the same account and folder, unsearched. On a client with a system back gesture, that gesture leaves search rather than the app. | Search is a mode over the list, not a destination. |
| 6 | **Clearing the query resets the scope** to the default, in the core *and* in the client's control, as one action. | A narrowing the user can no longer see is a narrowing they will not think of: the next search would silently be smaller than it looks. |
| 7 | The scope is **session state, never persisted**. | It is a filter on one search, not a preference. |
| 8 | **A search states how far back it looked**, on the results surface, with a route to the sync-depth setting. The horizon is the **narrowest** sync depth among the accounts the scope covered. | Search reads the local store and nothing else, so it finds only what sync depth kept. Unqualified, an empty result claims *"there is no such message"* when it means *"not in the last three months"*; only the second is something the user can fix. One three-month account makes the whole answer three months old at best, so the narrowest wins. |
| 9 | **Results follow the list's grouping.** With conversations on, a match is shown as the conversation it belongs to: the row carries the **whole** thread, is labelled by the newest message that **matched**, and sorts on that message (rule 1, measured against the query). | Search is a mode over the list, not a second kind of list: a row that stands for its whole thread in the mailbox has to stand for its whole thread here, or selecting one and archiving it means two different things on two screens ([`list-selection.md`](list-selection.md)). Labelling and sorting on the match is what keeps the answer legible: a row summarised by an unrelated later reply reads as the wrong result. |
| 10 | **What is on screen answers the query as it now stands.** A search overtaken while it ran is discarded, never published; and the field is quiet for **250 ms** before the core is asked at all. | Searches run concurrently and finish out of order, by wide margins: a two-letter prefix matches half the mailbox and takes a second and a half, the finished query a tenth. Published unconditionally, the screen goes to whichever finished last, which is reliably the broadest and least useful one, and nothing corrects it until an unrelated rebuild lands. |

The horizon (rule 8) is `MailboxListSnapshot::search_horizon`, `None` for every list nobody
searched, so a client keys the whole line off one field. It is folded over the accounts
[`snapshot_search::searched`](../crates/mailcal-app/src/snapshot_search.rs) returns, the same list
the search itself iterates, so "which accounts does a search cover" has one answer and not two.
The core sends `AllTime` or a month count and never words: the sentence is assembled client-side
like every other string (`AGENTS.md` → Client conventions).

The ordering (rule 1) and the scope semantics (rules 2–4, 6) live in the core:
[`view_rows::build_search`](../crates/mailcal-viewmodel/src/view_rows.rs) orders,
[`snapshot_search`](../crates/mailcal-app/src/snapshot_search.rs) decides which accounts and
folders answer, and `Intent::SetSearchScope` carries the filter. A client that does nothing gets
rules 1–3 and 5–7 for free; only rule 4 needs UI.

Rule 9 needs no client code either, and for one reason worth stating: the threaded projection
already reads `AccountMessage::in_scope` as "belongs to the list being built", so a search hands
it hits marked in scope and their conversations' other members marked out of it. Grouping,
labelling and ordering then fall out of the projection the mailbox list uses, and a client
renders whichever row kind arrives, as it already does.

Rule 10 is split between the two layers, because each half can only be enforced where it is:
the core discards a superseded list (`App::search_generation`, re-checked in `rebuild_snapshot`
before it publishes), and each client debounces its own field, since only the client knows a
keystroke happened. Neither half substitutes for the other. A debounce alone still races (two
searches 250 ms apart overlap whenever one is slow), and the guard alone leaves a word's worth
of searches competing for the same store.

## Candidates vs. results

The engine ranks each account's hits by **relevance** and caps them (`SEARCH_FETCH_LIMIT`, 500 per
account). The core treats that as a **candidate set**: it applies the scope filter, merges every
account's survivors, orders them by date, and shows the newest `SEARCH_LIMIT` (100). Search results
have no "show more": `total` equals the rows returned.

Two consequences worth knowing, both in Known gaps: a very broad query can push a *recent* match
out of an account's top-500 by relevance, and a query matching more than 100 messages shows only
the newest 100.

## Typing does not mean searching

A search is a full-text query per account plus a store read per hit: on a real five-account device
roughly a second. Every client therefore **debounces** the query, 250 ms after typing stops,
rather than dispatching per keystroke; clearing and leaving search stay immediate, being
navigation rather than a query. Dispatching per keystroke stacked seven concurrent searches to
type "monitor", each slowing the others: the same query measured ~2.0–2.5 s per rebuild stacked,
~0.86 s debounced.

The delay buys latency back; it does not decide what is shown. Which of the searches still in
flight may reach the screen is rule 10, and it is settled in the core.

## Where the field lives

Rules 1 to 8 are about what a search *answers*, and none of them moves with the box it is typed
into. What the placement has to respect is rule 2: the default scope is **every account and every
folder**, so a field that sits over the message list is a control whose reach is wider than the
column it is drawn on.

On a desktop it therefore belongs to the **window**, centred in its top row, which is Outlook's own
placement and the only surface whose centre is not a pane's. On a phone the list is the window, so
the field stays the platform's own list search (`.searchable`, a `SearchBar` on Android).

## Per-platform

| Platform | Newest-first order | Trash excluded by default | Scope filter (rule 4) | Back/exit restores the view | Horizon stated (rule 8) | Grouping obeyed (rule 9) | Clear (×) in the field | Debounced (rule 10) | The field sits | Where |
|---|:---:|:---:|:---:|:---:|:---:|:---:|:---:|:---:|---|---|
| Shared core | ✅ | ✅ | ✅ (`Intent::SetSearchScope`) | ✅ | ✅ (`search_horizon`) | ✅ | n/a | ✅ (`search_generation`) | n/a | `crates/mailcal-app/src/snapshot_search.rs` |
| Android | ✅ | ✅ | ✅ | ✅ (incl. system back) | ✅ | ✅ | ✅ | ✅ | over the list | `clients/android/.../SearchBar.kt` |
| macOS | ✅ | ✅ | ⬜ | ✅ | ✅ | ✅ | ✅ | ✅ | centred in the window toolbar | `clients/apple/…/Mailcal.Toolbar.swift`, `SearchHorizonStrip.swift` |
| iOS/iPadOS | ✅ | ✅ | ⬜ | ✅ | ✅ | ✅ | ✅ (`.searchable`) | ✅ | `.searchable` over the list | `clients/apple/…/SearchHorizonStrip.swift` |
| Windows | ✅ | ✅ | ⬜ | ✅ | ✅ | ✅ | ✅ (`AutoSuggestBox`) | ✅ | centred over the window's caption | `clients/windows/…/MainWindow.Search.cs`, `SearchHorizonLine.cs` |
| Linux | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ (`gtk::SearchEntry`) | ✅ | centred in the window toolbar | `clients/linux/src/ui/mail_toolbar.rs`, `ui/search/bar.rs` |
| MCP (`search`, `list_messages`) | ✅ | ✅ | ✅ | n/a | ✅ (`sync_depth_months`) | n/a | n/a | n/a | n/a | `crates/mailcal-mcp/src/tools/read.rs` |

Rule 9 does not apply to MCP, deliberately: that surface answers with **messages**, whatever the
user's list is set to. A caller pages it by offset and asks for one message's body by key, and a
conversation is not addressable that way. `query_search` passes `ViewMode::Flat` explicitly.

## Known gaps

- **The horizon states the sync-depth *policy*, not what has finished downloading.** While an
  account is still backfilling, "Searching the last 3 months" is what the device is *for*, and only
  becomes what it *holds* once the first pass completes. The engine could close the gap (it has the
  scope's cursor state, and `LocalCoverage::unsynced_objects` is the field for it), but its mail
  search reports every scope as complete unconditionally today
  (`store-sqlite/src/search_ops/mod.rs`, `assemble_results`), so there is nothing to read. The
  progress surface says a sync is running in the meantime (`docs/sync-progress.md`).
- **The field is in the window's top row everywhere but Android**, which is right: on a phone the
  list *is* the window, so the platform's own list search is the honest placement there.
- **The scope filter is on Android and Linux only.** macOS, iOS/iPadOS and Windows get the
  ordering and the Trash default from the core, but offer no way to narrow to the current folder
  yet; their search is always "all mail". Bringing them up is client wiring: the intent already
  exists.
- **A broad query can miss a recent match.** Ordering by date over a set the engine truncated by
  *relevance* means a message that ranked below an account's 500th-best can be dropped even though
  it is newer than results that are shown. Raising the cap trades latency for coverage (measured on
  a five-account device: 300 → ~0.6 s, 500 → ~0.86 s). The real fix is a **date-ordered** query in
  the engine (`search_mail` ranks by relevance only, with no sort option), which would make the
  candidate cap irrelevant, tracked as [engine issue #83](https://github.com/allodia-eu/email-calendar-sync-engine/issues/83),
  which also covers searching several accounts (or one mailbox) in a single query instead of the
  per-account loop this core runs today.
- **No pagination.** Results stop at 100 rows with no "show more", so a query matching thousands
  shows only the newest hundred and says nothing about the rest.
- **Junk/Spam is searched by default.** Deliberate (rule 2 names Trash only, matching Outlook), but
  worth revisiting if it proves noisy: Gmail excludes both.
- **Optimistically-removed messages still match.** A message just archived or deleted keeps
  appearing in results until the move lands and the store catches up; the mailbox list hides it
  immediately. Same class of staleness, opposite direction.
