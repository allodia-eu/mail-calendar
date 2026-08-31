//! Which pass raises which surface: the bar for a download the user awaits, the hint for one
//! they did not ask for, and silence for a poll that finds nothing.

use super::SyncProgressState;

/// Registers an account with `folders` folders on a pass and reports it downloading: the two
/// steps that admit it to the hint.
fn downloading(state: &mut SyncProgressState, id: u64, account: &str, folders: u32) {
    state.account_started(id, account, folders);
    state.downloading(id, account);
}

#[test]
fn an_awaited_pass_shows_the_bar() {
    let mut state = SyncProgressState::default();
    let (_id, _progress) = state.begin(true, true, 1);
    assert!(state.snapshot().active);
}

#[test]
fn ending_the_last_awaited_pass_hides_the_bar() {
    let mut state = SyncProgressState::default();
    let (id, _progress) = state.begin(true, true, 1);
    assert!(state.snapshot().active);
    state.end(id);
    assert!(!state.snapshot().active);
}

/// The rule the hint exists for: a pass nobody started never takes the bar's row of layout, no
/// matter how much it downloads. It says so in the status line instead.
#[test]
fn a_background_pass_never_raises_the_bar() {
    let mut state = SyncProgressState::default();
    let (id, _progress) = state.begin(false, true, 1);
    downloading(&mut state, id, "acct-1", 12);
    let snapshot = state.snapshot();
    assert!(!snapshot.active, "a background pass must not open the bar");
    assert_eq!(snapshot.accounts.len(), 1);
}

#[test]
fn a_background_pass_names_its_account_and_folder_counts_once_it_downloads() {
    let mut state = SyncProgressState::default();
    let (id, _progress) = state.begin(false, true, 12);
    state.account_started(id, "acct-1", 12);
    assert!(
        state.snapshot().accounts.is_empty(),
        "a pass that has not downloaded anything yet says nothing"
    );

    state.downloading(id, "acct-1");
    state.folder_finished(id, "acct-1");
    state.folder_finished(id, "acct-1");
    let hint = state.snapshot().accounts;
    assert_eq!(hint.len(), 1);
    assert_eq!(hint[0].account_id, "acct-1");
    assert_eq!((hint[0].folders_done, hint[0].folders_total), (2, 12));
}

/// The counterpart, and the whole reason the hint waits for a commit: an account polling on a
/// timer over already-cached mail would otherwise blink a status line every few minutes forever.
#[test]
fn a_background_pass_that_finds_nothing_says_nothing() {
    let mut state = SyncProgressState::default();
    let (id, _progress) = state.begin(false, true, 3);
    state.account_started(id, "acct-1", 3);
    state.folder_finished(id, "acct-1");
    state.folder_finished(id, "acct-1");
    state.folder_finished(id, "acct-1");
    let snapshot = state.snapshot();
    assert!(!snapshot.active);
    assert!(snapshot.accounts.is_empty());
}

/// The pass that follows the user's own archive/delete/send stays silent even though it does
/// download (the moved message is re-committed) because the row already left the list.
#[test]
fn an_unannounceable_pass_stays_silent_even_while_downloading() {
    let mut state = SyncProgressState::default();
    let (id, _progress) = state.begin(false, false, 1);
    downloading(&mut state, id, "acct-1", 1);
    let snapshot = state.snapshot();
    assert!(!snapshot.active);
    assert!(snapshot.accounts.is_empty());
}

/// An awaited download is already explained by the bar; naming it a second time in the status
/// line would say the same thing twice, in two places, about one pass.
#[test]
fn an_awaited_pass_is_never_also_in_the_hint() {
    let mut state = SyncProgressState::default();
    let (id, _progress) = state.begin(true, true, 1);
    downloading(&mut state, id, "acct-1", 4);
    let snapshot = state.snapshot();
    assert!(snapshot.active);
    assert!(snapshot.accounts.is_empty());
}

/// One background pass covers every account of a `refresh_mail` tick, so an account that finishes
/// first has to leave the hint on its own rather than lingering until the slowest one is done.
#[test]
fn an_account_leaves_the_hint_when_its_own_sync_ends() {
    let mut state = SyncProgressState::default();
    let (id, _progress) = state.begin(false, true, 8);
    downloading(&mut state, id, "acct-1", 3);
    downloading(&mut state, id, "acct-2", 5);
    assert_eq!(state.snapshot().accounts.len(), 2);

    state.account_finished(id, "acct-1");
    let hint = state.snapshot().accounts;
    assert_eq!(hint.len(), 1);
    assert_eq!(hint[0].account_id, "acct-2");
}

/// Two background passes can own the same account at once: a poll tick and a push refresh land
/// together, and the hint counts folders, not passes.
#[test]
fn two_background_passes_on_one_account_sum_their_folders() {
    let mut state = SyncProgressState::default();
    let (poll, _a) = state.begin(false, true, 12);
    let (push, _b) = state.begin(false, true, 1);
    downloading(&mut state, poll, "acct-1", 12);
    downloading(&mut state, push, "acct-1", 1);
    state.folder_finished(poll, "acct-1");

    let hint = state.snapshot().accounts;
    assert_eq!(hint.len(), 1, "one account, however many passes cover it");
    assert_eq!((hint[0].folders_done, hint[0].folders_total), (1, 13));
}

#[test]
fn ending_a_background_pass_clears_its_hint() {
    let mut state = SyncProgressState::default();
    let (id, _progress) = state.begin(false, true, 1);
    downloading(&mut state, id, "acct-1", 1);
    assert!(!state.snapshot().accounts.is_empty());
    state.end(id);
    assert!(state.snapshot().accounts.is_empty());
}

/// The hint is read on every progress signal, so its order may not depend on how the passes
/// happened to hash: a status line that reordered its accounts mid-download would be its own bug.
#[test]
fn the_hint_is_ordered_by_account_id() {
    let mut state = SyncProgressState::default();
    let (id, _progress) = state.begin(false, true, 3);
    for account in ["acct-3", "acct-1", "acct-2"] {
        downloading(&mut state, id, account, 1);
    }
    let ids: Vec<_> = state
        .snapshot()
        .accounts
        .into_iter()
        .map(|row| row.account_id)
        .collect();
    assert_eq!(ids, ["acct-1", "acct-2", "acct-3"]);
}

/// The body warm is the second half of catching up, and it used to be entirely invisible: a
/// multi-thousand-message account spent minutes downloading with nothing on screen saying so.
#[test]
fn a_body_warm_puts_its_account_in_the_hint() {
    let mut state = SyncProgressState::default();
    assert!(state.warming("acct-1", Some(250)));
    let hint = state.snapshot().accounts;
    assert_eq!(hint.len(), 1);
    assert_eq!(hint[0].account_id, "acct-1");
    assert!(hint[0].warming_bodies);
    assert_eq!(hint[0].bodies_done, 250);
    assert_eq!(
        (hint[0].folders_done, hint[0].folders_total),
        (0, 0),
        "the folder counts are meaningless once the pass they belong to has finished"
    );
}

/// A warm that reports the same count twice must not signal: this fires every 25 bodies over
/// thousands of them, and each signal costs every client a snapshot pull.
#[test]
fn an_unchanged_warm_count_does_not_move_the_hint() {
    let mut state = SyncProgressState::default();
    assert!(state.warming("acct-1", Some(25)));
    assert!(!state.warming("acct-1", Some(25)));
    assert!(state.warming("acct-1", Some(50)));
}

#[test]
fn ending_a_warm_clears_its_account() {
    let mut state = SyncProgressState::default();
    state.warming("acct-1", Some(25));
    assert!(state.warming("acct-1", None));
    assert!(state.snapshot().accounts.is_empty());
    assert!(
        !state.warming("acct-1", None),
        "a warm that never entered the hint has nothing to clear, and must not signal"
    );
}

/// A warm left over from the previous pass must not overwrite the counts of the pass running
/// now: the folders are what that account is waiting on, and they are the more useful figure.
#[test]
fn a_running_pass_outranks_a_leftover_warm_for_the_same_account() {
    let mut state = SyncProgressState::default();
    let (id, _progress) = state.begin(false, true, 12);
    downloading(&mut state, id, "acct-1", 12);
    state.folder_finished(id, "acct-1");
    state.warming("acct-1", Some(900));

    let hint = state.snapshot().accounts;
    assert_eq!(hint.len(), 1, "one account, not one row per phase");
    assert!(!hint[0].warming_bodies);
    assert_eq!((hint[0].folders_done, hint[0].folders_total), (1, 12));
}

/// The two phases are sequential per account but concurrent across them: one account can still be
/// syncing folders while another has moved on to its bodies.
#[test]
fn two_accounts_can_be_in_different_phases_at_once() {
    let mut state = SyncProgressState::default();
    let (id, _progress) = state.begin(false, true, 3);
    downloading(&mut state, id, "acct-1", 3);
    state.warming("acct-2", Some(120));

    let hint = state.snapshot().accounts;
    assert_eq!(hint.len(), 2);
    assert!(!hint[0].warming_bodies, "acct-1 is still on its folders");
    assert!(hint[1].warming_bodies, "acct-2 has moved on to its bodies");
    assert_eq!(hint[1].bodies_done, 120);
}
