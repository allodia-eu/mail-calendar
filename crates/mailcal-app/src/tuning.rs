//! The paging and search window sizes; how many rows a page shows, how deep a message load
//! reads, and how wide a search casts before it narrows.
//!
//! Split out of `lib.rs` (its parent) to stay under the 500-line limit. They belong together
//! because they are one dial: every one of them trades a first-screen cost against how much of
//! the mailbox is reachable without a second read, and changing one without reading the others
//! is how a "small" tuning tweak becomes a deep-scroll regression. `lib.rs` re-exports them, so
//! every call site still says `crate::PAGE`.

/// How many search-result rows the host is shown, across every account.
pub(crate) const SEARCH_LIMIT: usize = 100;

/// How many ranked hits each account's search asks the engine for, before the scope filter
/// and the newest-first merge.
///
/// Deliberately larger than [`SEARCH_LIMIT`]: the engine ranks by **relevance** and we display
/// by **date**, so the candidate set has to be wide enough that today's match is in it even
/// when a hundred older messages score higher. It also absorbs the hits the scope filter drops
/// (an account's Trash), which would otherwise eat into the shown rows.
pub(crate) const SEARCH_FETCH_LIMIT: usize = 500;

/// How many mailbox-list rows one page holds: the initial window after any navigation, and
/// the step `Intent::ShowMore` grows it by. Sized so the first screen of a folder builds
/// and crosses the FFI fast (the rest load as the host scrolls), Outlook-style.
pub(crate) const PAGE: usize = 100;

/// The floor for the list-load window (see `App::load_window`): boot reads at most this many of
/// the newest rows **across the accounts in view** rather than the whole mailbox. The unified
/// inbox is one ordered read, so this is a total and not a per-account allowance, which is the
/// point, since a merged list only ever shows the newest N however many accounts feed it.
pub(crate) const MIN_LOAD_WINDOW: usize = 500;

/// The load window is this multiple of the visible row limit, so heavy threading (several
/// messages collapsing into one conversation row) still leaves enough conversations to fill
/// the visible rows, and scrolling deeper (`Intent::ShowMore`) loads proportionally more.
pub(crate) const WINDOW_ROW_FACTOR: usize = 4;

/// Above the boot floor, the load window is rounded **up** to this bucket, so a run of
/// `Intent::ShowMore` steps (each grows the row limit by one [`PAGE`]) reuses the cached load;
/// a wider window is a superset; instead of re-reading a slightly larger window every step. That
/// re-read of the whole growing window on each scroll step was the deep-scroll cost. The boot
/// window ([`MIN_LOAD_WINDOW`]) is left un-bucketed so the first screen stays small and fast.
pub(crate) const WINDOW_BUCKET: usize = 2000;
