use std::collections::HashMap;

use mailcal_bindings::{
    AccountProvider, Notice, NoticeAction, NoticeIntent, NoticeKind, NoticeScreens, NoticeSeverity,
    NoticeText, UseProblem,
};

use super::{
    Announcements, SignInFlow, about, belongs, beneath, core_remedy, glyph, marks, more, offers,
    prominent, screen_of, sign_in_flow, words,
};
use crate::{
    l10n,
    ui::{PrimaryView, icons},
};

fn text(key: &str, address: Option<&str>) -> NoticeText {
    NoticeText {
        key: key.to_owned(),
        args: address
            .map(|address| HashMap::from([("address".to_owned(), address.to_owned())]))
            .unwrap_or_default(),
    }
}

/// An unreachable account's notice, or the whole app's when `account` is `None`.
fn notice(
    id: &str,
    screens: NoticeScreens,
    severity: NoticeSeverity,
    account: Option<&str>,
) -> Notice {
    Notice {
        id: id.to_owned(),
        kind: NoticeKind::Unreachable,
        title: text("notice_unreachable", Some(account.unwrap_or("everyone"))),
        detail: None,
        severity,
        screens,
        account: account.map(str::to_owned),
        mark: account.map(|_| text("notice_mark_unreachable", None)),
        action: Some(NoticeAction::Retry),
        action_label: Some(text("notice_action_retry", None)),
        dismissible: false,
        remedy_failed: false,
    }
}

#[test]
fn each_destination_is_the_screen_of_its_name() {
    assert_eq!(screen_of(PrimaryView::Mail), NoticeScreens::Mail);
    assert_eq!(screen_of(PrimaryView::Calendar), NoticeScreens::Calendar);
    assert_eq!(screen_of(PrimaryView::Contacts), NoticeScreens::Contacts);
}

#[test]
fn the_banner_holds_the_first_notice_that_belongs_to_the_screen_in_the_cores_order() {
    let notices = [
        notice(
            "calendar",
            NoticeScreens::Calendar,
            NoticeSeverity::Blocked,
            None,
        ),
        notice("mail", NoticeScreens::Mail, NoticeSeverity::Degraded, None),
        notice(
            "everywhere",
            NoticeScreens::Everywhere,
            NoticeSeverity::Informational,
            None,
        ),
    ];
    let shown = |screen| prominent(&notices, screen).map(|notice| notice.id.as_str());

    assert_eq!(shown(NoticeScreens::Calendar), Some("calendar"));
    // Not the Blocked one: it belongs to another screen. Never ranked again here.
    assert_eq!(shown(NoticeScreens::Mail), Some("mail"));
    // A notice for every screen is drawn where nothing of the screen's own comes first.
    assert_eq!(shown(NoticeScreens::Contacts), Some("everywhere"));
    assert!(belongs(&notices[2], NoticeScreens::Mail));
    assert!(!belongs(&notices[0], NoticeScreens::Mail));
}

#[test]
fn the_issues_button_counts_every_other_notice_and_only_beside_a_banner() {
    let notices = [
        notice(
            "calendar",
            NoticeScreens::Calendar,
            NoticeSeverity::Degraded,
            Some("a"),
        ),
        notice(
            "mail",
            NoticeScreens::Mail,
            NoticeSeverity::Degraded,
            Some("a"),
        ),
    ];
    assert_eq!(more(&notices, NoticeScreens::Mail), 1);
    assert_eq!(more(&notices[..1], NoticeScreens::Calendar), 0);
    // No banner on the contacts screen, so no button: the account's row carries both.
    assert_eq!(more(&notices, NoticeScreens::Contacts), 0);
}

#[test]
fn an_account_is_marked_with_its_first_notice_or_how_many_it_has() {
    let notices = [
        notice(
            "app",
            NoticeScreens::Everywhere,
            NoticeSeverity::Informational,
            None,
        ),
        notice(
            "one",
            NoticeScreens::Mail,
            NoticeSeverity::Blocked,
            Some("a"),
        ),
        notice(
            "only",
            NoticeScreens::Mail,
            NoticeSeverity::Degraded,
            Some("b"),
        ),
        notice(
            "two",
            NoticeScreens::Calendar,
            NoticeSeverity::Degraded,
            Some("a"),
        ),
    ];
    let marks = marks(&notices);

    assert_eq!(
        marks.len(),
        2,
        "a notice about the whole app marks no account"
    );
    let a = &marks["a"];
    assert_eq!(
        a.severity,
        NoticeSeverity::Blocked,
        "the first notice's glyph"
    );
    assert_eq!(a.label, l10n::notices_account_count(2));
    assert_eq!(
        a.detail,
        format!("{}\n{}", words(&notices[1].title), words(&notices[3].title)),
        "a screen reader hears every sentence, in the core's order"
    );
    let b = &marks["b"];
    assert_eq!(
        b.label,
        l10n::notice_mark_unreachable(),
        "a few words, no address"
    );
}

#[test]
fn the_account_page_lists_its_own_notices_in_order() {
    let notices = [
        notice(
            "one",
            NoticeScreens::Mail,
            NoticeSeverity::Blocked,
            Some("a"),
        ),
        notice(
            "other",
            NoticeScreens::Mail,
            NoticeSeverity::Blocked,
            Some("b"),
        ),
        notice(
            "two",
            NoticeScreens::Calendar,
            NoticeSeverity::Degraded,
            Some("a"),
        ),
    ];
    let ids: Vec<_> = about(&notices, "a")
        .iter()
        .map(|notice| notice.id.as_str())
        .collect();
    assert_eq!(ids, ["one", "two"]);
}

#[test]
fn a_failed_remedy_is_said_beneath_in_place_of_the_detail() {
    let mut outage = notice(
        "o",
        NoticeScreens::Calendar,
        NoticeSeverity::Degraded,
        Some("a"),
    );
    outage.kind = NoticeKind::CalendarOutage {
        problem: UseProblem::Unreachable,
    };
    assert_eq!(beneath(&outage), None);
    outage.detail = Some(text("notice_detail_unreachable_calendar", None));
    assert_eq!(
        beneath(&outage).as_deref(),
        Some(l10n::notice_detail_unreachable_calendar())
    );
    outage.remedy_failed = true;
    assert_eq!(
        beneath(&outage).as_deref(),
        Some(l10n::notices_remedy_failed())
    );
}

#[test]
fn each_severity_has_a_glyph_of_its_own_shape() {
    assert_eq!(glyph(NoticeSeverity::Blocked), icons::NOTICE_BLOCKED);
    assert_eq!(glyph(NoticeSeverity::Degraded), icons::NOTICE_DEGRADED);
    assert_eq!(
        glyph(NoticeSeverity::Informational),
        icons::NOTICE_INFORMATIONAL
    );
    assert_ne!(icons::NOTICE_BLOCKED, icons::NOTICE_DEGRADED);
    assert_ne!(icons::NOTICE_DEGRADED, icons::NOTICE_INFORMATIONAL);
}

#[test]
fn a_notice_is_announced_once_when_it_is_raised() {
    let mut announced = Announcements::default();
    let first = [notice(
        "a",
        NoticeScreens::Mail,
        NoticeSeverity::Blocked,
        Some("a"),
    )];
    let ids = |arrived: Vec<&Notice>| arrived.iter().map(|n| n.id.clone()).collect::<Vec<_>>();

    assert_eq!(ids(announced.arrived(&first)), ["a"]);
    assert_eq!(
        announced.arrived(&first),
        Vec::<&Notice>::new(),
        "a redraw announces nothing"
    );
    let both = [
        first[0].clone(),
        notice(
            "b",
            NoticeScreens::Mail,
            NoticeSeverity::Degraded,
            Some("b"),
        ),
    ];
    assert_eq!(ids(announced.arrived(&both)), ["b"]);
    assert_eq!(announced.arrived(&[]), Vec::<&Notice>::new());
    assert_eq!(
        ids(announced.arrived(&first)),
        ["a"],
        "one that went and came back is new"
    );
}

#[test]
fn only_a_retry_is_run_by_the_core() {
    let mut retry = notice(
        "r",
        NoticeScreens::Mail,
        NoticeSeverity::Degraded,
        Some("a"),
    );
    assert!(offers(&retry));
    assert_eq!(
        core_remedy(&retry),
        Some(NoticeIntent::Retry { id: "r".to_owned() })
    );
    for action in [
        NoticeAction::SignInAgain,
        NoticeAction::OpenAccountSettings,
        NoticeAction::SignInToAllodia,
        NoticeAction::Review,
    ] {
        retry.action = Some(action);
        assert_eq!(
            core_remedy(&retry),
            None,
            "{action:?} is a flow this client runs"
        );
    }
    retry.action = None;
    retry.action_label = None;
    assert!(!offers(&retry));
}

#[test]
fn signing_in_again_runs_the_flow_the_account_signs_in_by() {
    use SignInFlow::{AccountSignIn, Consent, Jmap, Settings};
    let expired = NoticeKind::SignInExpired;
    // A grant too narrow for mail or the calendar is widened at its provider, whatever it is.
    assert_eq!(sign_in_flow(&NoticeKind::MailReauth, None, false), Consent);
    assert_eq!(
        sign_in_flow(&NoticeKind::CalendarReauth, None, false),
        Consent
    );
    assert_eq!(
        sign_in_flow(&expired, Some(&AccountProvider::Microsoft), false),
        Consent
    );
    assert_eq!(
        sign_in_flow(&expired, Some(&AccountProvider::Google), false),
        Consent
    );
    assert_eq!(
        sign_in_flow(&expired, Some(&AccountProvider::JmapOauth), false),
        Jmap
    );
    assert_eq!(
        sign_in_flow(&expired, Some(&AccountProvider::ImapOauth), false),
        AccountSignIn
    );
    // A Nextcloud sign-in made it, so signing in again is that sign-in.
    assert_eq!(
        sign_in_flow(&expired, Some(&AccountProvider::Password), true),
        AccountSignIn
    );
    assert_eq!(
        sign_in_flow(&expired, Some(&AccountProvider::Password), false),
        Settings
    );
    assert_eq!(
        sign_in_flow(&expired, Some(&AccountProvider::Jmap), false),
        Settings
    );
    assert_eq!(sign_in_flow(&expired, None, false), Settings);
}
