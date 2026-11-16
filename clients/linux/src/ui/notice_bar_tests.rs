//! The banner, drawn from notices the core would send. GTK, so it runs inside the one widget test
//! that holds the display (`mailbox_tests`).

use std::collections::HashMap;

use adw::prelude::*;
use mailcal_bindings::{
    InvitationResponse, Notice, NoticeAction, NoticeKind, NoticeScreens, NoticeSeverity, NoticeText,
};

use super::NoticeBar;
use crate::{
    l10n,
    ui::{
        AppInput, PrimaryView,
        mailbox::tests::{glib_records, rendered_labels},
        notices::NoticeInput,
    },
};

fn text(key: &str, address: &str) -> NoticeText {
    NoticeText {
        key: key.to_owned(),
        args: HashMap::from([("address".to_owned(), address.to_owned())]),
    }
}

fn reauth(id: &str, kind: NoticeKind, screens: NoticeScreens, key: &str) -> Notice {
    Notice {
        id: id.to_owned(),
        kind,
        title: text(key, "alice@example.org"),
        detail: None,
        severity: NoticeSeverity::Blocked,
        screens,
        account: Some("alice".to_owned()),
        mark: None,
        action: Some(NoticeAction::SignInAgain),
        action_label: Some(text("notice_action_sign_in_again", "")),
        dismissible: false,
        remedy_failed: false,
        second_action: None,
        second_action_label: None,
        running: false,
    }
}

fn skipped() -> Notice {
    Notice {
        id: "skipped".to_owned(),
        kind: NoticeKind::AccountsSkipped { count: 1 },
        title: text("notice_accounts_skipped", ""),
        detail: None,
        severity: NoticeSeverity::Degraded,
        screens: NoticeScreens::Mail,
        account: None,
        mark: None,
        action: None,
        action_label: None,
        dismissible: true,
        remedy_failed: false,
        second_action: None,
        second_action_label: None,
        running: false,
    }
}

fn pressed(receiver: &relm4::Receiver<AppInput>) -> Option<NoticeInput> {
    match receiver.recv_sync() {
        Some(AppInput::Notice(input)) => Some(input),
        _ => None,
    }
}

pub(crate) fn one_banner_per_screen_and_the_rest_behind_issues() {
    let (sender, receiver) = relm4::channel();
    let mut bar = NoticeBar::new(&sender);
    let mail = reauth(
        "mail",
        NoticeKind::MailReauth,
        NoticeScreens::Mail,
        "notice_mail_reauth",
    );
    let calendar = reauth(
        "calendar",
        NoticeKind::CalendarReauth,
        NoticeScreens::Calendar,
        "notice_calendar_reauth",
    );
    let notices = [mail, calendar];

    bar.render(&notices, PrimaryView::Mail);
    assert!(bar.root.is_visible());
    assert_eq!(
        bar.banner.title(),
        l10n::notice_mail_reauth("alice@example.org")
    );
    assert_eq!(
        bar.banner.button_label().as_deref(),
        Some(l10n::notice_action_sign_in_again())
    );
    assert!(
        bar.root.has_css_class("blocked"),
        "Blocked takes the warning colour"
    );
    assert!(bar.issues.is_visible(), "a second notice is behind Issues");
    assert_eq!(
        bar.issues.label().as_deref(),
        Some(l10n::notices_more(1).as_str())
    );
    assert!(!bar.spinner.is_visible());
    bar.banner.emit_by_name::<()>("button-clicked", &[]);
    assert!(matches!(pressed(&receiver), Some(NoticeInput::Run(id)) if id == "mail"));

    let running = [
        notices[0].clone(),
        Notice {
            running: true,
            ..notices[1].clone()
        },
    ];
    bar.render(&running, PrimaryView::Calendar);
    assert_eq!(
        bar.banner.title(),
        l10n::notice_calendar_reauth("alice@example.org")
    );
    assert!(bar.spinner.is_visible(), "its sign-in is running");

    bar.render(&notices, PrimaryView::Contacts);
    assert!(
        !bar.root.is_visible(),
        "nothing belongs to the contacts screen"
    );

    // One that can only close offers Close, and closes it.
    let closable = [skipped()];
    bar.render(&closable, PrimaryView::Mail);
    assert!(!bar.issues.is_visible(), "one notice, no Issues button");
    assert!(!bar.root.has_css_class("blocked"));
    assert_eq!(
        bar.banner.button_label().as_deref(),
        Some(l10n::notices_dismiss())
    );
    bar.banner.emit_by_name::<()>("button-clicked", &[]);
    assert!(matches!(pressed(&receiver), Some(NoticeInput::Dismiss(id)) if id == "skipped"));

    // A failed remedy is said beneath the sentence.
    let mut failed = notices[0].clone();
    failed.remedy_failed = true;
    bar.render(&[failed], PrimaryView::Mail);
    assert_eq!(
        bar.banner.title(),
        format!(
            "{}\n{}",
            l10n::notice_mail_reauth("alice@example.org"),
            l10n::notices_remedy_failed()
        )
    );
}

/// The question a calendar server raises when it stored an answer and could not pass it on.
fn reply_question(running: bool) -> Notice {
    Notice {
        id: "reply".to_owned(),
        kind: NoticeKind::ReplyNotDelivered {
            summary: "Research & Development".to_owned(),
            organizer: "bob@example.test".to_owned(),
            response: InvitationResponse::Decline,
        },
        title: NoticeText {
            key: "notice_reply_not_delivered".to_owned(),
            args: HashMap::from([
                ("summary".to_owned(), "Research & Development".to_owned()),
                ("organizer".to_owned(), "bob@example.test".to_owned()),
            ]),
        },
        detail: None,
        severity: NoticeSeverity::Informational,
        screens: NoticeScreens::Everywhere,
        account: Some("alice".to_owned()),
        mark: None,
        action: Some(NoticeAction::SendReply),
        action_label: Some(text("notice_action_send_reply", "")),
        dismissible: false,
        remedy_failed: false,
        second_action: Some(NoticeAction::DontSendReply),
        second_action_label: Some(text("notice_action_dont_send_reply", "")),
        running,
    }
}

/// The strip's answers, in the order drawn, each with the words it shows.
fn answers(bar: &NoticeBar) -> Vec<(gtk::Button, String)> {
    let mut found = Vec::new();
    let mut child = bar.answers.first_child();
    while let Some(widget) = child {
        if let Some(button) = widget.downcast_ref::<gtk::Button>() {
            let words = rendered_labels(button.upcast_ref()).join(" ");
            found.push((button.clone(), words));
        }
        child = widget.next_sibling();
    }
    found
}

/// Whether a control component's button draws its spinner.
fn spins(button: &gtk::Button) -> bool {
    button
        .child()
        .and_then(|child| child.first_child())
        .is_some_and(|first| first.is::<adw::Spinner>())
}

pub(crate) fn a_question_offers_both_answers_and_holds_them_while_one_runs() {
    let (sender, receiver) = relm4::channel();
    let mut bar = NoticeBar::new(&sender);
    let question = reply_question(false);

    let ((), records) = glib_records(|| {
        bar.render(std::slice::from_ref(&question), PrimaryView::Calendar);
    });
    assert!(
        !records.iter().any(|line| line.contains("markup")),
        "the meeting's title is the sender's, never markup: {records:?}"
    );
    assert!(
        bar.banner.title().contains("bob@example.test"),
        "consent to send mail on someone's behalf needs the recipient"
    );
    assert_eq!(
        bar.banner.button_label(),
        None,
        "the strip draws both answers, so the banner draws neither"
    );
    let drawn = answers(&bar);
    let words: Vec<_> = drawn.iter().map(|(_, words)| words.as_str()).collect();
    assert_eq!(
        words,
        [
            l10n::notice_action_dont_send_reply(),
            l10n::notice_action_send_reply()
        ],
        "the lesser answer first"
    );
    let (second, primary) = (&drawn[0].0, &drawn[1].0);
    assert!(second.has_css_class("flat"));
    assert!(second.is_sensitive() && primary.is_sensitive());
    second.emit_clicked();
    assert!(matches!(pressed(&receiver), Some(NoticeInput::Second(id)) if id == "reply"));
    primary.emit_clicked();
    assert!(matches!(pressed(&receiver), Some(NoticeInput::Run(id)) if id == "reply"));

    // The core carries an answer out: neither can start a second, and the primary spins.
    bar.render(&[reply_question(true)], PrimaryView::Calendar);
    let drawn = answers(&bar);
    let (second, primary) = (&drawn[0].0, &drawn[1].0);
    assert!(!second.is_sensitive() && !primary.is_sensitive());
    assert!(spins(primary) && !spins(second));
    assert!(!bar.spinner.is_visible(), "the spinner is the button's");

    // A notice that is no question puts its one button back on the banner.
    bar.render(&[skipped()], PrimaryView::Mail);
    assert!(!bar.answers.is_visible());
    assert_eq!(
        bar.banner.button_label().as_deref(),
        Some(l10n::notices_dismiss())
    );
}
