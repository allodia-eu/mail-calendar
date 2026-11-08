//! Every word the core names is in the catalog with the arguments it is given, and every
//! `notice_` message in the catalog is one the core names. The catalog's own validator holds the
//! other locales to English.

use std::collections::{BTreeMap, BTreeSet};

use super::{action_label, detail, mark, title};
use crate::{
    FolderAction, FolderProblem, InvitationResponse, NoticeAction, NoticeKind, NoticeText,
    UseProblem,
};

const CATALOG: &str = include_str!("../../../messages/en.json");

fn catalog() -> BTreeMap<String, String> {
    serde_json::from_str::<BTreeMap<String, serde_json::Value>>(CATALOG)
        .expect("the catalog is JSON")
        .into_iter()
        .filter_map(|(key, value)| value.as_str().map(|text| (key, text.to_owned())))
        .collect()
}

fn placeholders(template: &str) -> BTreeSet<String> {
    template
        .split('{')
        .skip(1)
        .filter_map(|rest| rest.split_once('}').map(|(name, _)| name.to_owned()))
        .collect()
}

/// One of every kind, and both refusals a folder change can meet.
fn every_kind() -> Vec<NoticeKind> {
    let folder = |problem| NoticeKind::FolderChangeRefused {
        folder: "Receipts".to_owned(),
        action: FolderAction::Rename,
        problem,
    };
    let problems = [
        UseProblem::Unreachable,
        UseProblem::SignInRefused,
        UseProblem::NeedsOwnSignIn,
        UseProblem::ServerChanged,
        UseProblem::NoneOnServer,
    ];
    let outages = problems.into_iter().flat_map(|problem| {
        [
            NoticeKind::CalendarOutage { problem },
            NoticeKind::ContactsOutage { problem },
        ]
    });
    outages
        .chain([
            NoticeKind::SignInExpired,
            NoticeKind::MailReauth,
            NoticeKind::CalendarReauth,
            NoticeKind::Unreachable,
            NoticeKind::Offline,
            NoticeKind::AllodiaSignInEnded {
                address: "me@allodia.example".to_owned(),
            },
            NoticeKind::AccountsSkipped { count: 2 },
            folder(FolderProblem::ChangedElsewhere),
            folder(FolderProblem::Refused),
            NoticeKind::SentNotFiled {
                subject: "Quarterly figures".to_owned(),
            },
            NoticeKind::ReplyNotDelivered {
                summary: "Planning".to_owned(),
                organizer: "bo@example.org".to_owned(),
                response: InvitationResponse::Accept,
            },
            NoticeKind::TimeZoneChanged {
                zone: "Europe/Lisbon".to_owned(),
            },
        ])
        .collect()
}

fn every_text() -> Vec<NoticeText> {
    let actions = [
        NoticeAction::SignInAgain,
        NoticeAction::OpenAccountSettings,
        NoticeAction::SignInToAllodia,
        NoticeAction::Retry,
        NoticeAction::Review,
        NoticeAction::FileSentCopy,
        NoticeAction::LeaveSentCopy,
        NoticeAction::SendReply,
        NoticeAction::DontSendReply,
        NoticeAction::UseDeviceTimeZone,
        NoticeAction::KeepTimeZone,
    ];
    let kinds = every_kind();
    kinds
        .iter()
        .map(|kind| title(kind, "anna@example.org"))
        .chain(kinds.iter().filter_map(detail))
        .chain(kinds.iter().filter_map(mark))
        .chain(actions.into_iter().map(action_label))
        .collect()
}

#[test]
fn every_word_the_core_names_is_in_the_catalog_with_its_arguments() {
    let catalog = catalog();
    for text in every_text() {
        assert!(
            text.key.starts_with("notice_"),
            "{} is not a notice key",
            text.key
        );
        let template = catalog
            .get(&text.key)
            .unwrap_or_else(|| panic!("{} is not in the catalog", text.key));
        let given: BTreeSet<String> = text.args.keys().cloned().collect();
        assert_eq!(given, placeholders(template), "{}", text.key);
        assert!(
            text.args.values().all(|value| !value.is_empty()),
            "{}",
            text.key
        );
    }
}

#[test]
fn every_notice_message_in_the_catalog_is_one_the_core_names() {
    let named: BTreeSet<String> = every_text().into_iter().map(|text| text.key).collect();
    let orphans: Vec<String> = catalog()
        .into_keys()
        .filter(|key| key.starts_with("notice_") && !named.contains(key))
        .collect();
    assert!(orphans.is_empty(), "never named by the core: {orphans:?}");
}

#[test]
fn an_outage_and_the_reply_question_have_a_detail_and_each_problem_its_own() {
    let details: BTreeSet<String> = every_kind()
        .iter()
        .filter_map(detail)
        .map(|text| text.key)
        .collect();
    assert_eq!(
        details.len(),
        11,
        "five problems for each of two uses, and where a reply's next time is chosen"
    );
    assert!(details.contains("notice_detail_reply_not_delivered"));
    assert_eq!(detail(&NoticeKind::SignInExpired), None);
}

#[test]
fn a_mark_is_a_few_words_with_no_address_and_a_moved_server_reads_paused() {
    for kind in every_kind() {
        if let Some(words) = mark(&kind) {
            assert!(
                words.args.is_empty(),
                "{kind:?}: the row already shows the address"
            );
        }
    }
    let paused = |problem| mark(&NoticeKind::CalendarOutage { problem }).map(|words| words.key);
    assert_eq!(
        paused(UseProblem::ServerChanged).as_deref(),
        Some("notice_mark_calendar_paused")
    );
    assert_eq!(
        paused(UseProblem::Unreachable).as_deref(),
        Some("notice_mark_calendar_outage")
    );
}

#[test]
fn a_notice_names_the_account_by_its_address() {
    let text = title(&NoticeKind::SignInExpired, "anna@example.org");
    assert_eq!(
        text.args.get("address").map(String::as_str),
        Some("anna@example.org")
    );
}
