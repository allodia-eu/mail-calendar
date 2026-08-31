//! The sync-progress view-model: what a host may say about mail arriving in the background.
//!
//! Two surfaces, deliberately separate, because they answer different questions:
//!
//! - **The bar** ([`active`](SyncProgressSnapshot::active) and its counts) is for a download the
//!   user is *waiting on*; adding an account, opening an unsynced folder, an explicit refetch. It
//!   is up from the moment the pass starts, and it is the only thing allowed to take a row of
//!   layout.
//! - **The hint** ([`accounts`](SyncProgressSnapshot::accounts)) is for a pass nobody asked for, a
//!   poll tick, an `IDLE` push, a boot catch-up. It never opens a bar; it names the accounts
//!   currently pulling mail down, and how far through their folders each one is, so a status line
//!   can say so in passing.
//!
//! An account appears in the hint only once its pass has actually committed mail. A poll that
//! finds nothing says nothing; otherwise a quiet account would blink a hint on a timer forever.
//!
//! Projected from the engine's per-scope reports, which the app aggregates across the folders and
//! accounts it is syncing. Lives here, in the shared view-model, so every platform renders the
//! same shape.

/// An immutable snapshot of mail-sync progress for a host to render.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SyncProgressSnapshot {
    /// Whether a **user-awaited** download is running: a host shows the bar while this is true
    /// and hides it once the pass completes. A background pass never sets it; it reports itself
    /// through [`accounts`](Self::accounts) instead.
    pub active: bool,
    /// Messages committed (host-visible) so far across the folders of the awaited download.
    pub fetched: u64,
    /// The summed expected total across those folders, or `None` when not all of them have
    /// reported one yet (show an indeterminate bar).
    pub total: Option<u64>,
    /// The accounts whose **background** sync is downloading mail right now, in a stable order.
    /// Empty whenever nothing is arriving unasked, which is almost always. Never overlaps the
    /// bar: an awaited download is already explained by it, and saying so twice in two places
    /// is noise.
    pub accounts: Vec<AccountSyncProgress>,
}

/// One account catching up in the background, as far as a status line needs it.
///
/// Catching up has two phases, in order, and an account is in exactly one of them:
///
/// 1. **Folders**: the sync pass itself. The counts render as "3 of 12 folders"; `folders_total` is
///    the number of folders the pass set out to sync (one, for a push notification that named its
///    folder), so `folders_done` reaching it is the pass finishing, not the mail running out.
/// 2. **Bodies**; warming every synced message's body afterwards, which is the longer half on a
///    first sync and used to be entirely invisible. [`warming_bodies`](Self::warming_bodies) says
///    the account is here; [`bodies_done`](Self::bodies_done) is what is still moving.
///
/// There is no body **total**: the warm drains in batches against "what is still missing", so the
/// figure is only ever "how many so far": the same indeterminate case the bar already handles
/// with a `None` total.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AccountSyncProgress {
    /// The account, to be named from the host's own account list (which already holds the
    /// address it shows everywhere else).
    pub account_id: String,
    /// Folders whose sync has finished this pass.
    pub folders_done: u32,
    /// Folders this pass is syncing in total.
    pub folders_total: u32,
    /// Whether the account is past its folders and warming message bodies. The folder counts
    /// are then final, and a host should render `bodies_done` instead of them.
    pub warming_bodies: bool,
    /// Message bodies warmed so far, with no total to divide by.
    pub bodies_done: u32,
}
