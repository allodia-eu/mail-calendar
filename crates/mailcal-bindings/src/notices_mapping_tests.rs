//! The table of what each connectivity fact becomes (`docs/notices.md`, "What each fact becomes").

use super::{only, pull, quiet};
use crate::{
    ConnectivitySnapshot, NoticeAction, NoticeKind, NoticeScreens, NoticeSeverity, UseOutage,
    UseProblem, notices::NoticeBook,
};

fn ids(accounts: &[&str]) -> Vec<String> {
    super::ids(accounts)
}

/// One row of the mapping table: the facts, then what they must become.
type Case = (
    ConnectivitySnapshot,
    NoticeKind,
    NoticeSeverity,
    NoticeScreens,
    Option<&'static str>,
    Option<NoticeAction>,
);

#[test]
fn each_fact_maps_to_its_kind_severity_screens_and_action() {
    use NoticeScreens::{Calendar, Contacts, Everywhere, Mail};
    use NoticeSeverity::{Blocked, Degraded, Informational};

    let outage = |problem| {
        vec![UseOutage {
            account_id: "a".to_owned(),
            problem,
        }]
    };
    let cases: Vec<Case> = vec![
        (
            ConnectivitySnapshot {
                offline: true,
                ..quiet()
            },
            NoticeKind::Offline,
            Informational,
            Everywhere,
            None,
            None,
        ),
        (
            ConnectivitySnapshot {
                signin_expired_accounts: ids(&["a"]),
                ..quiet()
            },
            NoticeKind::SignInExpired,
            Blocked,
            Everywhere,
            Some("a"),
            Some(NoticeAction::SignInAgain),
        ),
        (
            ConnectivitySnapshot {
                mail_reauth_accounts: ids(&["a"]),
                ..quiet()
            },
            NoticeKind::MailReauth,
            Blocked,
            Mail,
            Some("a"),
            Some(NoticeAction::SignInAgain),
        ),
        (
            ConnectivitySnapshot {
                calendar_reauth_accounts: ids(&["a"]),
                ..quiet()
            },
            NoticeKind::CalendarReauth,
            Degraded,
            Calendar,
            Some("a"),
            Some(NoticeAction::SignInAgain),
        ),
        (
            ConnectivitySnapshot {
                calendar_outages: outage(UseProblem::Unreachable),
                ..quiet()
            },
            NoticeKind::CalendarOutage {
                problem: UseProblem::Unreachable,
            },
            Degraded,
            Calendar,
            Some("a"),
            Some(NoticeAction::Retry),
        ),
        (
            ConnectivitySnapshot {
                calendar_outages: outage(UseProblem::NeedsOwnSignIn),
                ..quiet()
            },
            NoticeKind::CalendarOutage {
                problem: UseProblem::NeedsOwnSignIn,
            },
            Degraded,
            Calendar,
            Some("a"),
            Some(NoticeAction::OpenAccountSettings),
        ),
        (
            ConnectivitySnapshot {
                calendar_outages: outage(UseProblem::ServerChanged),
                ..quiet()
            },
            NoticeKind::CalendarOutage {
                problem: UseProblem::ServerChanged,
            },
            Degraded,
            Calendar,
            Some("a"),
            Some(NoticeAction::Review),
        ),
        (
            ConnectivitySnapshot {
                contacts_outages: outage(UseProblem::SignInRefused),
                ..quiet()
            },
            NoticeKind::ContactsOutage {
                problem: UseProblem::SignInRefused,
            },
            Degraded,
            Contacts,
            Some("a"),
            Some(NoticeAction::OpenAccountSettings),
        ),
        (
            ConnectivitySnapshot {
                unreachable_accounts: ids(&["a"]),
                ..quiet()
            },
            NoticeKind::Unreachable,
            Degraded,
            Mail,
            Some("a"),
            Some(NoticeAction::Retry),
        ),
        (
            ConnectivitySnapshot {
                allodia_sign_in_ended: Some("me@allodia.example".to_owned()),
                ..quiet()
            },
            NoticeKind::AllodiaSignInEnded {
                address: "me@allodia.example".to_owned(),
            },
            Degraded,
            Everywhere,
            None,
            Some(NoticeAction::SignInToAllodia),
        ),
    ];
    for (connectivity, kind, severity, screens, account, action) in cases {
        let notices = pull(&mut NoticeBook::default(), &connectivity);
        let notice = only(&notices);
        assert_eq!(notice.kind, kind);
        assert_eq!(notice.severity, severity, "{kind:?}");
        assert_eq!(notice.screens, screens, "{kind:?}");
        assert_eq!(notice.account.as_deref(), account, "{kind:?}");
        assert_eq!(notice.action, action, "{kind:?}");
        assert!(!notice.dismissible, "{kind:?} is a state that holds");
        assert!(!notice.remedy_failed, "{kind:?}");
    }
}
