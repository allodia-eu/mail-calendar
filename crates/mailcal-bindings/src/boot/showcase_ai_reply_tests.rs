//! Draft a reply on the showcase across the FFI (`docs/ai.md` rules 22 and 23): a fresh reply is
//! not asked about before the editor has said anything, and a file added or removed signals the
//! record exactly when it changes what the card says.

use std::{
    sync::{Arc, mpsc},
    time::{Duration, Instant},
};

use mailcal_app::ai::{AiEntitlement, BuildKind};

use crate::{
    AiIntent, ComposeOrigin, ComposerFileAttachment, ComposerReplyDraft, Intent, LogLevel,
    MailcalApp, Recipients, ReplyDraftIntent, ReplyDraftState, ReplyDraftTaskKind, ShowcaseLocale,
    SnapshotRow, Surface,
    tests::{ChannelObserver, NullLogger},
};

/// The showcase as `MAILCAL_OFFER_AI` leaves it, with what it signals.
fn staged() -> (Arc<MailcalApp>, mpsc::Receiver<Surface>) {
    let (tx, rx) = mpsc::channel();
    let app = MailcalApp::new_showcase(
        Box::new(ChannelObserver { tx }),
        Box::new(NullLogger),
        LogLevel::Info,
        "Europe/Amsterdam".to_owned(),
        ShowcaseLocale::En,
    );
    let account = app.app.account_ids()[0].as_str().to_owned();
    super::super::offer_ai(&app);
    super::stage(app.app.as_ref(), &account, &account);
    (app, rx)
}

/// A reply composer named `reply`, opened on the inbox's first message.
fn open_reply(app: &MailcalApp, name: &str) {
    let (account, key) = app
        .mailbox_list()
        .rows
        .into_iter()
        .map(|row| match row {
            SnapshotRow::Flat { row } => (row.account, row.key),
            SnapshotRow::Thread { row } => (row.account, row.latest_key),
        })
        .next()
        .expect("the inbox is seeded");
    let staging = crate::tests::temp_data_dir(name);
    app.open_composer(
        "reply".to_owned(),
        ComposeOrigin::Reply {
            account,
            key,
            all: false,
        },
        staging.to_string_lossy().into_owned(),
        None,
    )
    .expect("the reply opens");
}

fn record(app: &MailcalApp) -> Option<ComposerReplyDraft> {
    app.composer_reply_draft("reply".to_owned(), None)
}

fn draft(app: &MailcalApp) {
    app.dispatch(Intent::Ai {
        intent: AiIntent::ReplyDraft {
            intent: ReplyDraftIntent::Draft {
                composition: "reply".to_owned(),
                from: None,
                instruction: "yes".to_owned(),
                language: "en".to_owned(),
                replace: false,
            },
        },
    });
}

fn eventually<T>(mut found: impl FnMut() -> Option<T>) -> T {
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        if let Some(found) = found() {
            return found;
        }
        assert!(Instant::now() < deadline, "never came");
        std::thread::sleep(Duration::from_millis(10));
    }
}

/// How many times the record was signalled since this was last asked.
fn signalled(rx: &mpsc::Receiver<Surface>) -> usize {
    rx.try_iter()
        .filter(|surface| matches!(surface, Surface::ReplyDraft))
        .count()
}

/// Whether the card's file to attach is done.
fn attached(app: &MailcalApp) -> bool {
    let Some(ReplyDraftState::Drafted { checklist, .. }) = record(app).map(|record| record.state)
    else {
        panic!("the card");
    };
    checklist
        .iter()
        .find(|item| item.kind == ReplyDraftTaskKind::Attach)
        .expect("a file to attach")
        .done
}

fn picked(name: &str) -> ComposerFileAttachment {
    ComposerFileAttachment {
        path: format!("/picked/{name}"),
        file_name: name.to_owned(),
        media_type: String::new(),
    }
}

/// The quote and the signature a reply opens with are not the person's writing: the first press
/// drafts without a question, before the editor has reported anything.
#[test]
fn a_fresh_reply_drafts_without_asking_before_the_editor_has_said() {
    let (app, _rx) = staged();
    open_reply(&app, "fresh-reply");
    assert!(!record(&app).expect("offered").ask_before_replacing);
    draft(&app);
    eventually(|| record(&app).and_then(|record| record.editor_script));
}

#[test]
fn a_file_signals_the_record_only_when_it_changes_the_card() {
    let (app, rx) = staged();
    open_reply(&app, "reply-files");
    let report = |serial: u64, lead: &str, inserted: bool| {
        let message = format!(
            r#"{{"id":0,"request":{{"reply_draft":{{"serial":{serial},"lead":"{lead}","inserted":{inserted},"left":[],"edits":0}}}}}}"#
        );
        assert!(app.composer_reply_draft_report("reply".to_owned(), message));
    };
    let add = |name: &str| {
        app.add_composer_files("reply".to_owned(), vec![picked(name)])
            .expect("added")
    };

    // Before a draft is in, a file is no item's answer.
    signalled(&rx);
    add("before.pdf");
    assert_eq!(signalled(&rx), 0);

    draft(&app);
    let script = eventually(|| record(&app).and_then(|record| record.editor_script));
    report(script.serial, "draft", true);
    assert!(!attached(&app));
    signalled(&rx);

    let first = add("drawings.pdf");
    assert_eq!(signalled(&rx), 1, "the item is done now");
    assert!(attached(&app));
    let second = add("invoice.pdf");
    assert_eq!(signalled(&rx), 0, "the card reads as it did");

    let id = |files: &crate::ComposerFiles, name: &str| {
        files
            .tiles
            .iter()
            .find(|tile| tile.file_name == name)
            .expect("listed")
            .id
            .clone()
    };
    app.remove_composer_file("reply".to_owned(), id(&second, "invoice.pdf"))
        .expect("removed");
    assert_eq!(signalled(&rx), 0, "one file still answers the item");
    app.remove_composer_file("reply".to_owned(), id(&first, "drawings.pdf"))
        .expect("removed");
    assert_eq!(signalled(&rx), 1, "the item is open again");
    assert!(!attached(&app));
}

/// The showcase as [`staged`] leaves it, in a production build whose entitlement carries the
/// assistant.
fn staged_in_production() -> Arc<MailcalApp> {
    let (app, _rx) = staged();
    let host = crate::ai_host::BindingsAiHost::offline(
        app.runtime.handle().clone(),
        Arc::clone(&app.credential_store),
        super::transport,
    );
    app.app
        .install_ai_host(Arc::new(host), BuildKind::Production);
    app.app.set_ai_entitlement(AiEntitlement::Granted);
    app
}

fn submit(app: &MailcalApp) -> Result<(), crate::MailcalError> {
    app.submit_composition(
        "reply".to_owned(),
        Recipients {
            to: "someone@test.local".to_owned(),
            cc: String::new(),
            bcc: String::new(),
        },
        "Re: Order".to_owned(),
        DOCUMENT.to_owned(),
        None,
    )
}

const DOCUMENT: &str = r#"{"blocks":[{"Paragraph":{"content":[{"Text":{"text":"Hello","bold":false,"italic":false,"underline":false}}]}}],"attachments":[]}"#;

/// An entitlement withdrawn while a drafted reply with open items is open takes the question at
/// Send with it: no client draws it while the record is `None`, so no send is held for it
/// (`docs/ai.md` rules 4 and 24).
#[test]
fn a_withdrawn_entitlement_holds_no_send_for_the_checklist() {
    let app = staged_in_production();
    open_reply(&app, "withdrawn-reply");
    draft(&app);
    let script = eventually(|| record(&app).and_then(|record| record.editor_script));
    let report = format!(
        r#"{{"id":0,"request":{{"reply_draft":{{"serial":{},"lead":"draft","inserted":true,"left":[0],"edits":0}}}}}}"#,
        script.serial
    );
    assert!(app.composer_reply_draft_report("reply".to_owned(), report));
    assert!(
        record(&app).is_some_and(|record| record.send_question.is_some()),
        "the checklist has open items"
    );
    assert!(submit(&app).is_err(), "asked first while offered");

    app.app.set_ai_entitlement(AiEntitlement::NotGranted);
    assert_eq!(record(&app), None);
    submit(&app).expect("the send is accepted");
}
