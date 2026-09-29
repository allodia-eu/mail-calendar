//! What one **account** owns in the preferences file: its signature slots, its remembered reply
//! choice, and its aliases; plus what removing the account leaves behind, which is the half that
//! has to be nothing.
//!
//! Split from `tests.rs`, which covers the file itself (round trip, defaults, and what an older
//! file loads as), to keep both under the 500-line limit.

use super::*;
use crate::signatures::{SignatureId, SignatureSlot};

#[test]
fn an_older_preferences_file_without_signature_assignments_defaults_to_empty() {
    // A file written before signatures existed still loads; the map defaults to empty, so every
    // account starts with no signature rather than the load erroring.
    let prefs: Preferences = toml::from_str("display_timezone = \"Europe/Amsterdam\"").unwrap();
    assert!(prefs.signature_assignments.is_empty());
    assert!(prefs.account_signature("me@x.test").is_empty());
}

#[test]
fn an_older_file_without_a_reply_fallback_asks_rather_than_assuming() {
    // The direction this must default in: an upgrade has to *ask* before emailing an organiser
    // on the user's behalf. Defaulting to `Always` would mean every existing install silently
    // gained permission to send mail it had never been asked about.
    let prefs: Preferences = toml::from_str("display_timezone = \"Europe/Amsterdam\"").unwrap();
    assert!(prefs.invitation_reply_fallback.is_empty());
    assert_eq!(prefs.reply_fallback("me@x.test"), ReplyFallback::Ask);
}

#[test]
fn a_remembered_reply_choice_round_trips_per_account() {
    let mut prefs = Preferences::default();
    prefs.set_reply_fallback("a@x.test", ReplyFallback::Always);
    prefs.set_reply_fallback("b@x.test", ReplyFallback::Never);
    let reloaded: Preferences = toml::from_str(&toml::to_string(&prefs).unwrap()).unwrap();
    assert_eq!(reloaded.reply_fallback("a@x.test"), ReplyFallback::Always);
    assert_eq!(reloaded.reply_fallback("b@x.test"), ReplyFallback::Never);
    // An account nobody answered for is still asked: the choice is per server, not global.
    assert_eq!(reloaded.reply_fallback("c@x.test"), ReplyFallback::Ask);
}

#[test]
fn setting_a_reply_choice_back_to_ask_drops_the_entry() {
    let mut prefs = Preferences::default();
    prefs.set_reply_fallback("a@x.test", ReplyFallback::Always);
    prefs.set_reply_fallback("a@x.test", ReplyFallback::Ask);
    assert!(prefs.invitation_reply_fallback.is_empty());
}

#[test]
fn removing_an_account_forgets_its_permission_to_send_replies() {
    // `Always` is standing permission to send mail as the user. A re-added id must not inherit
    // it, having never been asked on this account.
    let mut prefs = Preferences::default();
    prefs.set_reply_fallback("a@x.test", ReplyFallback::Always);
    assert!(prefs.remove_reply_fallback("a@x.test"));
    assert_eq!(prefs.reply_fallback("a@x.test"), ReplyFallback::Ask);
    assert!(!prefs.remove_reply_fallback("a@x.test"));
}

#[test]
fn clearing_both_slots_drops_the_accounts_assignment_entry() {
    // An account the user opened the picker for and then set back to None must not leave an
    // empty table behind: the file would grow a row per account merely looked at.
    let mut prefs = Preferences::default();
    let signature = SignatureId::new("kK3-x_9").unwrap();
    prefs.set_account_signature(
        "me@x.test",
        SignatureSlot::NewMessage,
        Some(signature.clone()),
    );
    assert_eq!(
        prefs.account_signature("me@x.test").new_message.as_ref(),
        Some(&signature)
    );

    prefs.set_account_signature("me@x.test", SignatureSlot::NewMessage, None);
    assert!(prefs.signature_assignments.is_empty());
}

#[test]
fn deleting_a_signature_clears_every_account_slot_that_pointed_at_it() {
    // A dangling assignment means "no signature" in effect, but leaves an id in the file naming
    // something that no longer exists: so a delete sweeps every slot, across accounts.
    let mut prefs = Preferences::default();
    let doomed = SignatureId::new("doomed").unwrap();
    let kept = SignatureId::new("kept").unwrap();
    prefs.set_account_signature("a@x.test", SignatureSlot::NewMessage, Some(doomed.clone()));
    prefs.set_account_signature("a@x.test", SignatureSlot::ReplyForward, Some(kept.clone()));
    prefs.set_account_signature(
        "b@x.test",
        SignatureSlot::ReplyForward,
        Some(doomed.clone()),
    );

    assert!(prefs.forget_signature(&doomed));
    assert_eq!(prefs.account_signature("a@x.test").new_message, None);
    assert_eq!(
        prefs.account_signature("a@x.test").reply_forward.as_ref(),
        Some(&kept)
    );
    // b@x.test had nothing left, so its entry went with it.
    assert!(!prefs.signature_assignments.contains_key("b@x.test"));
    // A second sweep finds nothing to do.
    assert!(!prefs.forget_signature(&doomed));
}

#[test]
fn removing_an_account_drops_its_signature_assignment() {
    // Mirrors `remove_account_calendars`: a re-added account starts from the defaults rather
    // than inheriting a pointer to a signature the user may have deleted meanwhile.
    let mut prefs = Preferences::default();
    let signature = SignatureId::new("kK3-x_9").unwrap();
    prefs.set_account_signature("me@x.test", SignatureSlot::NewMessage, Some(signature));
    assert!(prefs.remove_account_signature("me@x.test"));
    assert!(!prefs.remove_account_signature("me@x.test"));
    assert!(prefs.account_signature("me@x.test").is_empty());
}

#[test]
fn alias_writes_drop_blanks_and_case_insensitive_duplicates() {
    // A user typing a trailing comma, or repeating an address in a different case, must not end
    // up with an entry that can never match an iTIP ATTENDEE.
    let mut prefs = Preferences::default();
    prefs.set_account_aliases(
        "acct",
        vec![
            "  info@example.com  ".to_owned(),
            String::new(),
            "   ".to_owned(),
            "INFO@example.com".to_owned(),
            "sales@example.com".to_owned(),
        ],
    );
    assert_eq!(
        prefs.aliases_of("acct"),
        [
            "info@example.com".to_owned(),
            "sales@example.com".to_owned()
        ]
    );

    // An account nobody configured has no aliases, and clearing the list drops the row rather
    // than persisting an empty one.
    assert!(prefs.aliases_of("other").is_empty());
    prefs.set_account_aliases("acct", vec![String::new()]);
    assert!(prefs.aliases_of("acct").is_empty());
    assert!(
        !prefs.account_aliases.contains_key("acct"),
        "an empty list must not leave a row behind"
    );
}

#[test]
fn removing_an_account_drops_its_aliases() {
    // The alias set decides which iTIP ATTENDEE line is "me" (docs/invitations.md), so one that
    // outlived its account would not merely linger: an id re-added later would inherit it and
    // could read somebody else's invitation as an RSVP it owes.
    let mut prefs = Preferences::default();
    prefs.set_account_aliases("me@x.test", vec!["info@x.test".to_owned()]);
    assert!(prefs.remove_account_aliases("me@x.test"));
    assert!(prefs.aliases_of("me@x.test").is_empty());
    assert!(!prefs.remove_account_aliases("me@x.test"));
}

#[test]
fn a_writing_style_is_assigned_cleared_and_forgotten_without_dangling() {
    let mut prefs = Preferences::default();
    let work = crate::WritingStyleId::new("work").unwrap();
    let home = crate::WritingStyleId::new("home").unwrap();
    prefs.set_account_writing_style("a@x.test", Some(work.clone()));
    prefs.set_account_writing_style("b@x.test", Some(work.clone()));
    prefs.set_account_writing_style("c@x.test", Some(home.clone()));
    assert_eq!(prefs.writing_style_of("a@x.test"), Some(&work));

    // Forgetting a style clears it from every account that drafted in it, and only those.
    assert!(prefs.forget_writing_style(&work));
    assert!(!prefs.forget_writing_style(&work));
    assert_eq!(prefs.writing_style_of("a@x.test"), None);
    assert_eq!(prefs.writing_style_of("c@x.test"), Some(&home));

    prefs.set_account_writing_style("c@x.test", None);
    assert!(prefs.ai.writing_styles.is_empty());
}

#[test]
fn removing_an_account_drops_its_writing_style() {
    let mut prefs = Preferences::default();
    prefs.set_account_writing_style("a@x.test", crate::WritingStyleId::new("work"));
    assert!(prefs.remove_account_writing_style("a@x.test"));
    assert!(!prefs.remove_account_writing_style("a@x.test"));
}

/// A file written before the AI preferences existed reads as the strictest mode, no styles and
/// no endpoint.
#[test]
fn an_older_file_has_no_ai_preferences_and_the_strictest_mode() {
    let prefs: Preferences = toml::from_str("display_timezone = \"Europe/Amsterdam\"").unwrap();
    assert_eq!(
        prefs.jurisdiction_mode,
        mailcal_jurisdiction::Mode::EuNative
    );
    assert!(prefs.ai.writing_styles.is_empty());
    assert!(prefs.ai.endpoint.is_none());
}
