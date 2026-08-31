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
roughly a second. A client therefore **debounces** the query (Android and Linux: 250 ms after
typing stops) rather than dispatching per keystroke; clearing and leaving search stay immediate.
Dispatching per keystroke stacked seven concurrent searches to type "monitor", each slowing the
others: the same query measured ~2.0–2.5 s per rebuild stacked, ~0.86 s debounced.

## Per-platform

| Platform | Newest-first order | Trash excluded by default | Scope filter (rule 4) | Back/exit restores the view | Horizon stated (rule 8) | Where |
|---|:---:|:---:|:---:|:---:|:---:|---|
| Shared core | ✅ | ✅ | ✅ (`Intent::SetSearchScope`) | ✅ | ✅ (`search_horizon`) | `crates/mailcal-app/src/snapshot_search.rs` |
| Android | ✅ | ✅ | ✅ | ✅ (incl. system back) | ✅ | `clients/android/.../SearchBar.kt` |
| macOS | ✅ | ✅ | ⬜ | ✅ | ✅ | `clients/apple/…/SearchHorizonStrip.swift` |
| iOS/iPadOS | ✅ | ✅ | ⬜ | ✅ | ✅ | `clients/apple/…/SearchHorizonStrip.swift` |
| Windows | ✅ | ✅ | ⬜ | ✅ | ✅ | `clients/windows/…/SearchHorizonLine.cs` |
| Linux | ✅ | ✅ | ✅ | ✅ | ✅ | `clients/linux/src/ui/search/bar.rs` |
| MCP (`search`, `list_messages`) | ✅ | ✅ | ✅ | n/a | ✅ (`sync_depth_months`) | `crates/mailcal-mcp/src/tools/read.rs` |

## Known gaps

- **The horizon states the sync-depth *policy*, not what has finished downloading.** While an
  account is still backfilling, "Searching the last 3 months" is what the device is *for*, and only
  becomes what it *holds* once the first pass completes. The engine could close the gap (it has the
  scope's cursor state, and `LocalCoverage::unsynced_objects` is the field for it), but its mail
  search reports every scope as complete unconditionally today
  (`store-sqlite/src/search_ops/mod.rs`, `assemble_results`), so there is nothing to read. The
  progress surface says a sync is running in the meantime (`docs/sync-progress.md`).
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
