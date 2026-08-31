//! The pairing that decides whether a user is warned before a series edit.
//!
//! The three transport shapes here are the **measured** ones, named in
//! `engine_provider::OverrideSurvival`'s own table. If one of these assertions ever disagrees
//! with that table, one of the two is out of date and the user is being told something untrue
//! about their own calendar.

use engine_api::OverrideSurvival;

use super::{SeriesEditTouches, SeriesEditWarning, series_edit_warning};

/// An edit that moves the series' start or end; what `survives_time_change` is about.
fn moves_the_series() -> SeriesEditTouches {
    SeriesEditTouches {
        timing: true,
        ..SeriesEditTouches::default()
    }
}

/// An edit that replaces or clears the repeat rule; what `survives_rule_change` is about.
fn changes_the_rule() -> SeriesEditTouches {
    SeriesEditTouches {
        rule: true,
        ..SeriesEditTouches::default()
    }
}

/// An edit that touches only a property an override may have set for itself; what
/// `clobbers_own_fields` is about. The commonest edit there is: a retitle.
fn retitles() -> SeriesEditTouches {
    SeriesEditTouches {
        fields: true,
        ..SeriesEditTouches::default()
    }
}

/// Everything at once: the worst an edit can be.
fn changes_everything() -> SeriesEditTouches {
    SeriesEditTouches {
        timing: true,
        rule: true,
        fields: true,
    }
}

/// What CalDAV and JMAP do: a series edit costs nothing.
fn keeps_everything() -> OverrideSurvival {
    OverrideSurvival::kept()
}

/// What Graph does: a move or a rule change reverts the occurrence to the pattern.
fn destroys_on_any_change() -> OverrideSurvival {
    OverrideSurvival {
        survives_time_change: false,
        survives_rule_change: false,
        clobbers_own_fields: false,
    }
}

/// What Google does: a move reverts it, a rule change does not, and a rename spreads.
fn destroys_on_move_and_renames() -> OverrideSurvival {
    OverrideSurvival {
        survives_time_change: false,
        survives_rule_change: true,
        clobbers_own_fields: true,
    }
}

#[test]
fn a_clean_series_is_never_warned_about() {
    // The half that keeps the warning worth reading. A user who has never singled out an
    // occurrence has nothing to lose, and a dialog that appears anyway is what teaches people
    // to click past the one that mattered.
    for (what, survival) in [
        ("a transport that keeps everything", keeps_everything()),
        ("one that destroys everything", destroys_on_any_change()),
        ("one that renames as well", destroys_on_move_and_renames()),
    ] {
        assert_eq!(
            series_edit_warning(Some(survival), false, changes_everything()),
            None,
            "{what}: no overrides means nothing of the user's to lose"
        );
    }
}

#[test]
fn a_transport_that_keeps_overrides_says_nothing_either() {
    assert_eq!(
        series_edit_warning(Some(keeps_everything()), true, changes_everything()),
        None
    );
}

#[test]
fn each_measured_transport_gets_the_warning_that_is_true_of_it() {
    assert_eq!(
        series_edit_warning(Some(destroys_on_any_change()), true, changes_everything()),
        Some(SeriesEditWarning::OccurrencesReset),
        "nothing is renamed here, so the warning must not claim it is"
    );
    assert_eq!(
        series_edit_warning(
            Some(destroys_on_move_and_renames()),
            true,
            changes_everything()
        ),
        Some(SeriesEditWarning::OccurrencesResetAndRenamesSpread),
        "two separate losses, and the user is owed both"
    );
}

#[test]
fn a_rename_that_spreads_on_its_own_is_its_own_warning() {
    // No transport measured so far does only this, and that is exactly why it has a variant:
    // the alternative is folding it into the reset sentence, which would tell a user their
    // moved occurrences were about to go back when nothing of the sort would happen.
    let renames_only = OverrideSurvival {
        survives_time_change: true,
        survives_rule_change: true,
        clobbers_own_fields: true,
    };

    assert_eq!(
        series_edit_warning(Some(renames_only), true, changes_everything()),
        Some(SeriesEditWarning::RenamesSpread)
    );
}

#[test]
fn an_account_that_cannot_write_calendars_is_asked_nothing() {
    // No capability means no writes at all, so there is no series edit to warn about.
    assert_eq!(series_edit_warning(None, true, changes_everything()), None);
}

#[test]
fn an_edit_that_cannot_cause_the_loss_does_not_announce_it() {
    // Measured on a real Outlook account: a series holding one moved occurrence, retitled
    // through our own editor. The occurrence kept its time; Graph reverts an override when
    // the MASTER moves or its rule changes, and a retitle does neither. The warning said
    // otherwise, which is the shape that teaches people to click past the real one.
    assert_eq!(
        series_edit_warning(Some(destroys_on_any_change()), true, retitles()),
        None,
        "Graph leaves an override's own fields alone, so a retitle costs the user nothing"
    );
}

#[test]
fn each_flag_is_owed_only_by_the_change_it_describes() {
    // Google: a move reverts an occurrence, a rule change does not, a rename spreads. Three
    // different edits, three different answers, and the old pairing gave all three the same
    // one, because it never asked what the edit was.
    let google = destroys_on_move_and_renames();
    assert_eq!(
        series_edit_warning(Some(google), true, moves_the_series()),
        Some(SeriesEditWarning::OccurrencesReset),
        "the move is undone, nothing is renamed, so the sentence must not say so"
    );
    assert_eq!(
        series_edit_warning(Some(google), true, changes_the_rule()),
        None,
        "Google keeps overrides through a rule change, so this edit owes nothing"
    );
    assert_eq!(
        series_edit_warning(Some(google), true, retitles()),
        Some(SeriesEditWarning::RenamesSpread),
        "the rename spreads; no time goes back, so the sentence must not claim one does"
    );
}

#[test]
fn a_rule_change_alone_is_enough_where_the_rule_is_what_destroys_them() {
    // Graph is the only transport that loses overrides to a rule change, and this is the case
    // that separates the two reset flags: same account, same series, timing untouched.
    assert_eq!(
        series_edit_warning(Some(destroys_on_any_change()), true, changes_the_rule()),
        Some(SeriesEditWarning::OccurrencesReset)
    );
}

#[test]
fn an_edit_that_changes_nothing_is_never_warned_about() {
    for (what, survival) in [
        ("a transport that keeps everything", keeps_everything()),
        ("one that destroys everything", destroys_on_any_change()),
        ("one that renames as well", destroys_on_move_and_renames()),
    ] {
        assert_eq!(
            series_edit_warning(Some(survival), true, SeriesEditTouches::default()),
            None,
            "{what}: an edit that touches nothing can cost nothing"
        );
    }
}
