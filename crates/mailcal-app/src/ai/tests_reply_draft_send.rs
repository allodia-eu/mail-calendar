//! A drafted reply at Send (`docs/ai.md` rules 24 and 25): the question asked once, from the
//! function the card is drawn from, and the mark a drafted reply carries: its `Message-ID` in
//! this device's log and the keyword asked for on its Sent copy. And the other half of the mark:
//! what the Sent listing says is a drafted reply is never shown to a draft as the person's words.
//!
//! A child of the rich-submit tests, for [`super::mail_fake`]. Every address is `@test.local` or a
//! fixture's `.test`; nothing is sent anywhere.

use std::sync::Arc;

use engine_api::{Draft, EmailAddress, MessageIdHeader, UtcDateTime};
use engine_core::{
    ids::{MailboxId, MessageId},
    mail::{Keyword, Message},
    membership::Memberships,
};
use mailcal_account::drafted_reply_keyword;
use mailcal_composer::{Block, ComposerDocument, LeadText, Paragraph};

use super::{
    mail_fake::{ProviderLogs, ThreadProvider, original_message},
    reply_draft_support::{
        ScriptedRelay, assistant, drafted, drafted_into, open_reply, report, run, start,
    },
};
use crate::{App, CompositionId, Intent, ai::AiQueued};

// What the mark does on a stored draft, beside what it does on a send.
#[path = "tests_drafted_draft.rs"]
mod drafted_draft;

fn send(composition: &CompositionId) -> Intent {
    Intent::SubmitRichMail {
        from: None,
        to: "reply@test.local".to_owned(),
        cc: String::new(),
        bcc: String::new(),
        subject: "Re: Quarterly report".to_owned(),
        document: ComposerDocument {
            blocks: vec![Block::Paragraph(Paragraph::new(Vec::new()))],
            attachments: Vec::new(),
        },
        blobs: Vec::new(),
        composition: Some(composition.clone()),
    }
}

/// Sends `composition` and answers the draft that went to the provider.
async fn sent(
    app: &Arc<App<ThreadProvider>>,
    logs: &ProviderLogs,
    composition: &CompositionId,
) -> Draft {
    let before = logs.submissions.lock().unwrap().len();
    let task = tokio::spawn({
        let app = Arc::clone(app);
        let intent = send(composition);
        async move { app.dispatch(intent).await }
    });
    while logs.submissions.lock().unwrap().len() == before {
        tokio::task::yield_now().await;
    }
    task.abort();
    logs.submissions.lock().unwrap().last().cloned().unwrap()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn send_asks_once_while_the_checklist_has_open_items() {
    let relay = ScriptedRelay::answering(Ok(drafted()));
    let (app, _) = assistant(ThreadProvider::with(vec![original_message("m1")]), &relay).await;
    let reply = open_reply(&app, "reply").await;
    assert_eq!(
        app.reply_draft_send_question(&reply, &|_| 0),
        None,
        "nothing drafted"
    );
    let serial = drafted_into(&app, &reply).await;

    assert_eq!(app.reply_draft_send_question(&reply, &|_| 0), Some(4));
    let card = super::reply_draft_support::view(&app, &reply);
    assert_eq!(
        card.send_question,
        Some(4),
        "the card and Send read one function"
    );
    // Every item done: nothing to ask.
    app.take_reply_draft_report(
        &reply,
        report(serial, LeadText::Written, true, vec![], 2),
        0,
    );
    app.tick_reply_draft_item(&reply, 3, true);
    assert_eq!(app.reply_draft_send_question(&reply, &|_| 1), None);
    // An undo opens a gap again, and Send would ask; answered either way, it never asks again.
    assert_eq!(app.reply_draft_send_question(&reply, &|_| 0), Some(1));
    app.answer_reply_draft_send_question(&reply);
    assert_eq!(app.reply_draft_send_question(&reply, &|_| 0), None);
    app.take_reply_draft_report(
        &reply,
        report(serial, LeadText::Draft, true, vec![0, 1], 3),
        0,
    );
    assert_eq!(app.reply_draft_send_question(&reply, &|_| 0), None);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_drafted_reply_is_logged_and_asks_for_the_keyword_on_its_sent_copy() {
    let relay = ScriptedRelay::answering(Ok(drafted()));
    let (app, logs) = assistant(ThreadProvider::with(vec![original_message("m1")]), &relay).await;

    let plain = open_reply(&app, "plain").await;
    let mine = sent(&app, &logs, &plain).await;
    assert!(mine.sent_copy_keywords.is_empty());
    assert!(!app.is_drafted_reply(&mine.message_id));

    // Drafted, then undone: what is sent may still carry its words, so it is marked all the same.
    let reply = open_reply(&app, "reply").await;
    let serial = drafted_into(&app, &reply).await;
    app.take_reply_draft_report(
        &reply,
        report(serial, LeadText::Written, false, vec![], 2),
        0,
    );
    let drafted = sent(&app, &logs, &reply).await;
    assert!(
        drafted
            .sent_copy_keywords
            .contains(&drafted_reply_keyword())
    );
    assert!(app.is_drafted_reply(&drafted.message_id));
}

fn sent_message(key: &str, id: &str, day: u8, keyword: bool) -> Message {
    let mut message = Message::new(
        MessageId::try_from(key).unwrap(),
        Memberships::of_one(MailboxId::try_from("sent").unwrap()),
    );
    message.envelope.from = vec![EmailAddress::new("me@allodia.local")];
    message.envelope.to = vec![EmailAddress::new("sender@remote.test")];
    message.envelope.message_id = vec![MessageIdHeader::new(id).unwrap()];
    message.sent_at = Some(UtcDateTime::new(2026, 10, day, 9, 0, 0).unwrap());
    if keyword {
        message
            .keywords
            .insert(Keyword::new(mailcal_account::DRAFTED_REPLY_KEYWORD).unwrap());
    }
    message
}

fn plain(text: &str) -> Vec<u8> {
    format!("Content-Type: text/plain\r\n\r\n{text}\r\n").into_bytes()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_draft_reads_the_persons_own_words_and_leaves_out_every_drafted_reply() {
    let provider = ThreadProvider::with(vec![
        original_message("m1"),
        sent_message("s1", "s1@allodia.local", 3, true),
        sent_message("s2", "s2@allodia.local", 2, false),
        sent_message("s3", "s3@allodia.local", 4, false),
    ])
    .with_sent_folder()
    .serving("s1", &plain("Words a draft wrote for me last time."))
    .serving(
        "s2",
        &plain("Words I wrote myself, plainly and in my own voice."),
    )
    .serving("s3", &plain("Words this device logged as drafted."));
    let relay = ScriptedRelay::answering(Ok(drafted()));
    let (app, _) = assistant(provider, &relay).await;
    app.note_drafted_reply("acct-1", MessageIdHeader::new("s3@allodia.local").unwrap());
    let listed = |key: &str| sent_message(key, &format!("{key}@allodia.local"), 1, key == "s1");
    assert!(
        app.is_drafted_message(&listed("s1")),
        "by the keyword on its listing"
    );
    assert!(
        app.is_drafted_message(&listed("s3")),
        "by this device's log"
    );
    assert!(!app.is_drafted_message(&listed("s2")));

    let reply = open_reply(&app, "reply").await;
    app.take_reply_draft_report(&reply, report(0, LeadText::Empty, false, vec![], 0), 0);
    assert_eq!(app.draft_reply(&reply, start(false)), Some(AiQueued::Run));
    run(&app).await;

    let seen = relay.seen.lock().unwrap();
    let material = &seen[0].messages[1].content;
    assert!(material.contains("Words I wrote myself"));
    assert!(!material.contains("a draft wrote"));
    assert!(!material.contains("logged as drafted"));
}
