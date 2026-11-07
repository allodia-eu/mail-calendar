//! The banner, drawn from notices the core would send. GTK, so it runs inside the one widget test
//! that holds the display (`mailbox_tests`).

use std::collections::HashMap;

use adw::prelude::*;
use mailcal_bindings::{
    Notice, NoticeAction, NoticeKind, NoticeScreens, NoticeSeverity, NoticeText,
};

use super::NoticeBar;
use crate::{
    l10n,
    ui::{AppInput, PrimaryView, notices::NoticeInput},
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

    bar.render(&notices, PrimaryView::Mail, None);
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

    bar.render(&notices, PrimaryView::Calendar, Some("calendar"));
    assert_eq!(
        bar.banner.title(),
        l10n::notice_calendar_reauth("alice@example.org")
    );
    assert!(bar.spinner.is_visible(), "its sign-in is running");

    bar.render(&notices, PrimaryView::Contacts, None);
    assert!(
        !bar.root.is_visible(),
        "nothing belongs to the contacts screen"
    );

    // One that can only close offers Close, and closes it.
    let closable = [skipped()];
    bar.render(&closable, PrimaryView::Mail, None);
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
    bar.render(&[failed], PrimaryView::Mail, None);
    assert_eq!(
        bar.banner.title(),
        format!(
            "{}\n{}",
            l10n::notice_mail_reauth("alice@example.org"),
            l10n::notices_remedy_failed()
        )
    );
}
