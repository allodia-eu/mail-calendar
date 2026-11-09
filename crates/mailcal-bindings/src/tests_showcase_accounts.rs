//! The showcase's **Settings → Accounts**: that both seeded accounts are listed, each used for
//! what its seed binds, that the account-less boot lists none, and the server change a launch hook
//! can pause the first one for.
//!
//! Every client draws that category from `accounts_snapshot` alone (`docs/accounts.md` rule 11),
//! so a showcase that registered nothing photographs an empty category, and the client suites
//! that open an account's page could not run on the showcase at all.

use std::sync::mpsc;

use super::*;
use crate::tests::{ChannelObserver, NullLogger};

fn state(entry: &AccountEntry, capability: AccountCapability) -> CapabilityState {
    entry
        .uses
        .iter()
        .find(|used| used.capability == capability)
        .map_or(CapabilityState::Off, |used| used.state)
}

#[test]
fn showcase_lists_both_seeded_accounts_used_for_what_their_seeds_bind() {
    let (tx, _rx) = mpsc::channel();
    let app = MailcalApp::new_showcase(
        Box::new(ChannelObserver { tx }),
        Box::new(NullLogger),
        LogLevel::Info,
        "Europe/Amsterdam".to_owned(),
        ShowcaseLocale::En,
    );

    let listed = app.accounts_snapshot().accounts;
    // The same accounts, in the same order, as the mail settings the page draws beneath its own
    // rows: an entry whose id the mail settings do not know would be a page without its mail.
    let mail: Vec<String> = app
        .sync_settings()
        .accounts
        .into_iter()
        .map(|account| account.account_id)
        .collect();
    let ids: Vec<&str> = listed.iter().map(|entry| entry.id.as_str()).collect();
    assert_eq!(ids, mail, "Settings → Accounts lists both seeded accounts");

    let (primary, secondary) = (&listed[0], &listed[1]);
    for entry in &listed {
        assert_eq!(
            entry.address, entry.id,
            "a showcase account is read by its address"
        );
        assert!(
            app.registry.endpoints(&entry.id).is_none(),
            "{}: no servers to edit, so nothing on its page dials",
            entry.address
        );
    }
    for capability in [
        AccountCapability::Mail,
        AccountCapability::Calendar,
        AccountCapability::Contacts,
    ] {
        assert_eq!(
            state(primary, capability),
            CapabilityState::On,
            "the primary seed binds mail, a calendar and contacts: {capability:?}"
        );
    }
    assert_eq!(
        state(secondary, AccountCapability::Mail),
        CapabilityState::On
    );
    assert_eq!(
        state(secondary, AccountCapability::Contacts),
        CapabilityState::On
    );
    assert_eq!(
        state(secondary, AccountCapability::Calendar),
        CapabilityState::Off,
        "the second seed binds no calendar, so its page does not claim one"
    );
}

#[test]
fn showcase_first_run_lists_no_account() {
    // An empty snapshot is what sends a client back to first-run setup, which is the screen this
    // boot exists to photograph.
    let (tx, _rx) = mpsc::channel();
    let app = MailcalApp::new_showcase_first_run(
        Box::new(ChannelObserver { tx }),
        Box::new(NullLogger),
        LogLevel::Info,
        "Europe/Amsterdam".to_owned(),
        ShowcaseLocale::En,
    );
    assert!(app.accounts_snapshot().accounts.is_empty());
}

/// What `MAILCAL_SHOWCASE_SERVER_CHANGE` puts in front of the UI suites: the primary account's
/// calendar paused for a server on another site, and an Allow the showcase refuses, since it has
/// no servers to change. The account stays paused, so the suites can read the refusal.
#[cfg(debug_assertions)]
#[test]
fn showcase_can_pause_its_calendar_for_a_server_change_it_will_not_allow() {
    let (tx, _rx) = mpsc::channel();
    let app = MailcalApp::new_showcase(
        Box::new(ChannelObserver { tx }),
        Box::new(NullLogger),
        LogLevel::Info,
        "Europe/Amsterdam".to_owned(),
        ShowcaseLocale::En,
    );
    let primary = app.accounts_snapshot().accounts[0].id.clone();
    crate::boot::pause_for_a_server_change(&app.app, &primary);

    let shown = app
        .account_server_change(primary.clone())
        .expect("the calendar waits on a server change")
        .change;
    assert!(shown.calendar && !shown.contacts);
    assert!(app.accept_server_change(&primary, &shown).is_err());
    assert!(app.account_server_change(primary).is_some(), "still paused");
}

/// What `MAILCAL_SHOWCASE_REAUTH` puts in front of the UI suites and the help screenshots: the
/// primary account asked to sign in again for mail and for its calendar, the two notices only a
/// narrowed Microsoft grant raises otherwise (`docs/provider-oauth.md` rule 11).
#[cfg(debug_assertions)]
#[test]
fn showcase_can_ask_its_account_to_sign_in_again_for_mail_and_calendar() {
    let (tx, _rx) = mpsc::channel();
    let app = MailcalApp::new_showcase(
        Box::new(ChannelObserver { tx }),
        Box::new(NullLogger),
        LogLevel::Info,
        "Europe/Amsterdam".to_owned(),
        ShowcaseLocale::En,
    );
    let primary = app.accounts_snapshot().accounts[0].id.clone();
    crate::boot::ask_for_reconsent(&app.app, &primary);

    let kinds: Vec<_> = app
        .notices()
        .notices
        .into_iter()
        .filter(|notice| notice.account.as_deref() == Some(primary.as_str()))
        .map(|notice| notice.kind)
        .collect();
    assert!(kinds.contains(&crate::NoticeKind::MailReauth), "{kinds:?}");
    assert!(
        kinds.contains(&crate::NoticeKind::CalendarReauth),
        "{kinds:?}"
    );
}

/// What `MAILCAL_SHOWCASE_QUESTIONS` puts in front of the UI suites: every question the core can
/// ask, each a notice with two answers (`docs/notices.md` rule 8), and a device report at launch
/// leaves the time zone question standing.
#[cfg(debug_assertions)]
#[test]
fn showcase_can_ask_every_question_as_a_notice() {
    let (tx, _rx) = mpsc::channel();
    let app = MailcalApp::new_showcase(
        Box::new(ChannelObserver { tx }),
        Box::new(NullLogger),
        LogLevel::Info,
        "Europe/Amsterdam".to_owned(),
        ShowcaseLocale::En,
    );
    let primary = app.accounts_snapshot().accounts[0].clone();
    let device = engine_api::TimeZoneId::iana("Europe/Amsterdam").expect("a zone");
    crate::boot::ask_the_questions(
        &app.app,
        &app.runtime,
        &primary.id,
        &primary.address,
        &device,
    );
    app.dispatch(crate::Intent::ReportDeviceTimeZone {
        id: "Europe/Amsterdam".to_owned(),
    });

    let notices = app.notices().notices;
    let answers = |kind: fn(&crate::NoticeKind) -> bool| {
        notices
            .iter()
            .find(|notice| kind(&notice.kind))
            .map(|notice| (notice.action, notice.second_action))
    };
    use crate::NoticeAction::{
        DontSendReply, FileSentCopy, KeepTimeZone, LeaveSentCopy, SendReply, UseDeviceTimeZone,
    };
    assert_eq!(
        answers(|kind| matches!(kind, crate::NoticeKind::SentNotFiled { .. })),
        Some((Some(FileSentCopy), Some(LeaveSentCopy)))
    );
    assert_eq!(
        answers(|kind| matches!(kind, crate::NoticeKind::ReplyNotDelivered { .. })),
        Some((Some(SendReply), Some(DontSendReply)))
    );
    assert_eq!(
        answers(|kind| matches!(kind, crate::NoticeKind::TimeZoneChanged { .. })),
        Some((Some(UseDeviceTimeZone), Some(KeepTimeZone)))
    );

    // An intent runs on the core's runtime, so the answer lands a moment after `dispatch` returns.
    app.dispatch(crate::Intent::DismissTimeZoneChange);
    app.dispatch(crate::Intent::DismissUnfiledCopy);
    let answered = |kind: &crate::NoticeKind| {
        matches!(
            kind,
            crate::NoticeKind::TimeZoneChanged { .. } | crate::NoticeKind::SentNotFiled { .. }
        )
    };
    assert!(
        crate::tests::wait_until(std::time::Duration::from_secs(5), || {
            !app.notices()
                .notices
                .iter()
                .any(|notice| answered(&notice.kind))
        }),
        "an answered question goes: {:?}",
        app.notices().notices
    );
}
