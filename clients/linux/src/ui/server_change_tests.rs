//! The server-change dialog's copy and what an allow comes to.

use mailcal_bindings::{ServerChange, ServerChangeAnswer};

use super::{Review, ReviewFrom, allowed, body_markup, failed, title};
use crate::{
    l10n,
    ui::{account_settings::AccountsInput, settings::notice::Notice},
};

fn change(calendar: bool, contacts: bool) -> ServerChange {
    ServerChange {
        previous_site: "icloud.com".to_owned(),
        proposed_site: "example.net".to_owned(),
        calendar,
        contacts,
    }
}

fn review(change: ServerChange) -> Review {
    Review {
        address: "alice@example.org".to_owned(),
        change,
    }
}

/// The text a person reads, once Pango has taken the markup apart; it fails on markup it cannot
/// parse, which an alert dialog would show as an empty body.
fn read(markup: &str) -> String {
    let (_, text, _) = gtk::pango::parse_markup(markup, '\0').expect("the body is valid markup");
    text.to_string()
}

#[test]
fn the_dialog_names_what_moved_both_or_the_one_that_did() {
    let address = "alice@example.org";
    for (change, heading, body, unsure) in [
        (
            change(true, false),
            l10n::server_changed_title_calendar(),
            l10n::server_changed_body_calendar(address),
            l10n::server_changed_unsure_calendar(),
        ),
        (
            change(false, true),
            l10n::server_changed_title_contacts(),
            l10n::server_changed_body_contacts(address),
            l10n::server_changed_unsure_contacts(),
        ),
        (
            change(true, true),
            l10n::server_changed_title_both(),
            l10n::server_changed_body_both(address),
            l10n::server_changed_unsure_both(),
        ),
    ] {
        assert_eq!(title(&change), heading);
        let shown = read(&body_markup(&review(change.clone())));
        assert_eq!(
            shown,
            format!(
                "{body}\n\n{}\n{}\n\n{}\n\n{unsure}",
                l10n::server_changed_before("icloud.com"),
                l10n::server_changed_now("example.net"),
                l10n::server_changed_advice(),
            ),
            "{change:?}"
        );
    }
}

#[test]
fn only_the_sites_are_bold_and_nothing_the_server_named_is_markup() {
    let mut moved = change(true, false);
    moved.proposed_site = "a&b<i>.example".to_owned();
    let markup = body_markup(&review(moved));

    assert!(markup.contains("<b>icloud.com</b>"));
    assert!(markup.contains("<b>a&amp;b&lt;i&gt;.example</b>"));
    assert_eq!(markup.matches("<b>").count(), 2);
    assert!(read(&markup).contains(&l10n::server_changed_now("a&b<i>.example")));
}

#[test]
fn a_failed_allow_is_said_where_it_was_asked_and_a_good_one_redraws() {
    let account = "alice@example.org@imap.example.org";
    let shown = || review(change(true, false));
    assert!(matches!(
        allowed(
            Some(ServerChangeAnswer::Moved),
            account.to_owned(),
            shown(),
            ReviewFrom::Outage
        ),
        AccountsInput::Changed(None)
    ));
    assert!(matches!(
        allowed(None, account.to_owned(), shown(), ReviewFrom::Settings),
        AccountsInput::Refused(notice) if notice == failed()
    ));
    assert!(matches!(
        allowed(None, account.to_owned(), shown(), ReviewFrom::Outage),
        AccountsInput::ServerChangeFailed(id) if id == account
    ));
    assert!(matches!(
        failed(),
        Notice::Error { detail, .. } if detail == l10n::server_changed_failed()
    ));
}

#[test]
fn a_question_replaced_while_shown_is_asked_again_as_it_stands_and_nothing_fails() {
    let account = "alice@example.org@imap.example.org";
    let mut now = change(true, true);
    now.proposed_site = "example.org".to_owned();
    let answer = ServerChangeAnswer::Replaced {
        change: now.clone(),
    };

    let next = allowed(
        Some(answer),
        account.to_owned(),
        review(change(true, false)),
        ReviewFrom::Outage,
    );

    assert!(matches!(
        next,
        AccountsInput::ServerChangeReplaced { account: id, review: asked, from: ReviewFrom::Outage }
            if id == account && asked == review(now)
    ));
}
