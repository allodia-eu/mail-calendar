//! Rule 8: each question the core holds is a notice with two answers, never closed, only
//! answered, and a new question is a new notice.

use super::{Questions, ReplyQuestion, UnfiledQuestion};
use crate::{
    ConnectivitySnapshot, InvitationResponse, Notice, NoticeAction, NoticeKind, NoticeScreens,
    NoticeSeverity,
    notices::{NoticeBook, NoticeFacts, raise},
};

fn address(account: &str) -> String {
    format!("{account}@example.org")
}

fn pull(book: &mut NoticeBook, questions: &Questions) -> Vec<Notice> {
    let connectivity = ConnectivitySnapshot::default();
    let facts = NoticeFacts {
        connectivity: &connectivity,
        skipped_at_launch: 0,
        folder_notice: None,
        provider: &|_| None,
        address: &address,
        questions,
        signing_in: None,
    };
    book.snapshot(raise(&facts)).notices
}

fn unfiled(subject: &str, retrying: bool) -> Questions {
    Questions {
        unfiled: Some(UnfiledQuestion {
            account: "anna".to_owned(),
            subject: subject.to_owned(),
            retrying,
        }),
        ..Questions::default()
    }
}

#[test]
fn a_copy_not_in_sent_offers_to_save_it_or_leave_it() {
    let notices = pull(&mut NoticeBook::default(), &unfiled("Figures", false));
    let notice = &notices[0];
    assert_eq!(
        notice.kind,
        NoticeKind::SentNotFiled {
            subject: "Figures".to_owned()
        }
    );
    assert_eq!(notice.title.key, "notice_sent_not_filed");
    assert_eq!(notice.title.args["subject"], "Figures");
    assert_eq!(notice.severity, NoticeSeverity::Degraded);
    assert_eq!(notice.screens, NoticeScreens::Mail);
    assert_eq!(notice.account.as_deref(), Some("anna"));
    assert_eq!(notice.action, Some(NoticeAction::FileSentCopy));
    assert_eq!(notice.second_action, Some(NoticeAction::LeaveSentCopy));
    assert_eq!(
        notice
            .second_action_label
            .as_ref()
            .map(|label| label.key.as_str()),
        Some("notice_action_leave_sent_copy")
    );
    assert!(!notice.dismissible, "a question is answered, not closed");
    assert!(!notice.running);
}

#[test]
fn a_save_to_sent_that_is_running_holds_the_buttons() {
    let notices = pull(&mut NoticeBook::default(), &unfiled("Figures", true));
    assert!(notices[0].running);
}

#[test]
fn a_second_question_is_a_new_notice() {
    let mut book = NoticeBook::default();
    let first = pull(&mut book, &unfiled("Figures", false))[0].id.clone();
    assert_eq!(
        pull(&mut book, &unfiled("Figures", true))[0].id,
        first,
        "the same question"
    );
    assert_ne!(pull(&mut book, &unfiled("Minutes", false))[0].id, first);
}

#[test]
fn a_reply_not_passed_on_names_the_organiser_and_offers_to_email_it() {
    let questions = Questions {
        reply: Some(ReplyQuestion {
            account: "anna".to_owned(),
            summary: "Planning".to_owned(),
            organizer: "bo@example.org".to_owned(),
            response: InvitationResponse::Tentative,
        }),
        ..Questions::default()
    };
    let notices = pull(&mut NoticeBook::default(), &questions);
    let notice = &notices[0];
    assert_eq!(notice.title.args["organizer"], "bo@example.org");
    assert_eq!(notice.title.args["summary"], "Planning");
    assert_eq!(
        notice.detail.as_ref().map(|detail| detail.key.as_str()),
        Some("notice_detail_reply_not_delivered")
    );
    assert_eq!(notice.screens, NoticeScreens::Everywhere);
    assert_eq!(notice.action, Some(NoticeAction::SendReply));
    assert_eq!(notice.second_action, Some(NoticeAction::DontSendReply));
    assert!(matches!(
        notice.kind,
        NoticeKind::ReplyNotDelivered {
            response: InvitationResponse::Tentative,
            ..
        }
    ));
    assert_eq!(
        notice.mark.as_ref().map(|mark| mark.key.as_str()),
        Some("notice_mark_reply_not_delivered")
    );
}

#[test]
fn a_changed_time_zone_is_the_whole_apps_and_offers_update_or_keep() {
    let questions = Questions {
        time_zone: Some("Europe/Lisbon".to_owned()),
        ..Questions::default()
    };
    let notices = pull(&mut NoticeBook::default(), &questions);
    let notice = &notices[0];
    assert_eq!(notice.title.args["zone"], "Europe/Lisbon");
    assert_eq!(notice.severity, NoticeSeverity::Informational);
    assert_eq!(notice.account, None);
    assert_eq!(notice.mark, None);
    assert_eq!(notice.action, Some(NoticeAction::UseDeviceTimeZone));
    assert_eq!(notice.second_action, Some(NoticeAction::KeepTimeZone));
}

#[test]
fn a_notice_that_is_not_a_question_has_one_answer() {
    let connectivity = ConnectivitySnapshot {
        unreachable_accounts: vec!["anna".to_owned()],
        ..ConnectivitySnapshot::default()
    };
    let facts = NoticeFacts {
        connectivity: &connectivity,
        skipped_at_launch: 0,
        folder_notice: None,
        provider: &|_| None,
        address: &address,
        questions: &Questions::default(),
        signing_in: None,
    };
    let notices = NoticeBook::default().snapshot(raise(&facts)).notices;
    assert_eq!(notices[0].second_action, None);
    assert_eq!(notices[0].second_action_label, None);
}
