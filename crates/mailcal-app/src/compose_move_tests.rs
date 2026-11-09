//! A composer moved to another view of it, the inline one popped out into a window, opens on the
//! composition it had and holding exactly what it held (`docs/reading-window.md`).
//!
//! A child of [`super`] (the rich-submit tests), over [`super::mail_fake`]'s provider.

use engine_api::{AccountId, MessageIdHeader, ProviderKey};
use mailcal_composer::{Block, ComposerDocument, Paragraph};

use super::mail_fake::{ThreadProvider, app_and_logs, original_message};
use crate::{
    ComposeKind, ComposeOrigin, ComposerState, CompositionId, DraftStatus, DraftsIntent, Intent,
    MessageRef, MovedComposer, MovedSignature, protocol::StagedAttachment,
};

const DOCUMENT: &str = r#"{"blocks":[{"Paragraph":{"content":[{"Text":{"text":"Half a reply","bold":false,"italic":false,"underline":false}}]}}],"attachments":[]}"#;

fn composition(name: &str) -> CompositionId {
    CompositionId::new(name).expect("a composition id")
}

fn original() -> MessageRef {
    MessageRef::from_parts("acct-1", "m1".to_owned()).expect("a message reference")
}

fn staging_dir(name: &str) -> String {
    let dir = std::env::temp_dir().join(format!("mailcal-move-staging-{name}"));
    let _ = std::fs::remove_dir_all(&dir);
    dir.to_string_lossy().into_owned()
}

fn save(composition: &CompositionId, subject: &str) -> Intent {
    Intent::Drafts(DraftsIntent::Save {
        composition: composition.clone(),
        from: None,
        to: "reply@remote.test".to_owned(),
        cc: String::new(),
        bcc: String::new(),
        subject: subject.to_owned(),
        document: ComposerDocument {
            blocks: vec![Block::Paragraph(Paragraph::new(Vec::new()))],
            attachments: Vec::new(),
        },
        blobs: Vec::new(),
        then_close: false,
    })
}

fn state() -> ComposerState {
    ComposerState {
        from: Some(AccountId::try_from("acct-2").expect("an account id")),
        to: "reply@remote.test, colleague@remote.test".to_owned(),
        cc: "boss@remote.test".to_owned(),
        bcc: "me@local.test".to_owned(),
        subject: "Re: Quarterly report (revised)".to_owned(),
        document: DOCUMENT.to_owned(),
        attachments: vec![StagedAttachment {
            path: "/tmp/composer/figures.pdf".to_owned(),
            file_name: "figures.pdf".to_owned(),
            media_type: "application/pdf".to_owned(),
        }],
        signature: MovedSignature::Chosen("sig-work".to_owned()),
        written: true,
    }
}

fn moved(state: ComposerState) -> ComposeOrigin {
    ComposeOrigin::Moved {
        state: Box::new(state),
    }
}

/// **The contract.** The moved composer saves over the copy the composer it left saved, and keeps
/// the conversation that composer joined. A move that opened a composition of its own would leave
/// the first copy in Drafts beside the second, and a reply would turn into a new message.
#[tokio::test]
async fn a_moved_composer_saves_over_what_the_one_it_left_saved() {
    let (app, logs) = app_and_logs(ThreadProvider::with(vec![original_message("m1")]));
    app.dispatch(Intent::RefreshMail).await;
    let reply = composition("reply");
    app.open_composer(
        reply.clone(),
        ComposeOrigin::Reply {
            message: original(),
            all: false,
        },
        None,
        &staging_dir("reply"),
    )
    .await
    .expect("the reply opens");
    app.dispatch(save(&reply, "Re: Quarterly report")).await;

    app.open_composer(reply.clone(), moved(state()), None, &staging_dir("moved"))
        .await
        .expect("the composer moves");
    assert_eq!(
        app.draft_status(&reply),
        DraftStatus::Saved,
        "the move is not a new composer: what its save came to is still this composer's"
    );
    app.dispatch(save(&reply, "Re: Quarterly report (revised)"))
        .await;

    let puts = logs.draft_puts.lock().unwrap();
    assert_eq!(puts.len(), 2);
    assert_eq!(
        puts[1].1,
        Some(ProviderKey::new("draft-1").unwrap()),
        "the moved composer's save must name what the one it left stored"
    );
    assert_eq!(
        puts[1].0.in_reply_to.as_ref().map(MessageIdHeader::as_str),
        Some("parent@remote"),
        "the move keeps the conversation the reply joined when it opened"
    );
    assert_eq!(
        puts[0].0.message_id, puts[1].0.message_id,
        "one composition, one Message-ID"
    );
}

/// The moved composer opens holding what the one it left held: the fields, the editor's document
/// as the editor wrote it, the files and the signature choice, with nothing seeded over them.
#[tokio::test]
async fn a_moved_composer_opens_holding_what_it_was_given() {
    let (app, _logs) = app_and_logs(ThreadProvider::with(vec![original_message("m1")]));
    app.dispatch(Intent::RefreshMail).await;
    let reply = composition("reply-all");
    app.open_composer(
        reply.clone(),
        ComposeOrigin::Reply {
            message: original(),
            all: true,
        },
        None,
        &staging_dir("reply-all"),
    )
    .await
    .expect("the reply opens");

    let given = state();
    let opening = app
        .open_composer(reply, moved(given.clone()), None, &staging_dir("moved-all"))
        .await
        .expect("the composer moves");

    assert_eq!(opening.composition, "reply-all");
    assert_eq!(
        opening.kind,
        ComposeKind::ReplyAll,
        "still the reply it was"
    );
    assert_eq!(opening.account, "acct-2", "the From the user picked");
    assert_eq!(opening.to, given.to);
    assert_eq!(opening.cc, given.cc);
    assert_eq!(opening.bcc, given.bcc);
    assert_eq!(opening.subject, given.subject);
    assert_eq!(opening.attachments, given.attachments);
    assert!(opening.body_html.is_empty() && opening.body_text.is_empty());
    assert_eq!(
        opening.quote, None,
        "the quote is in the document, not seeded again"
    );
    assert!(!opening.seeds_signature, "the signature is in the document");
    assert_eq!(
        opening.moved,
        Some(MovedComposer {
            document: DOCUMENT.to_owned(),
            written: true,
            signature: MovedSignature::Chosen("sig-work".to_owned()),
        })
    );
}

/// A composer that follows the default sender moves still following it, on the account its
/// composition is written from.
#[tokio::test]
async fn a_moved_composer_with_no_from_keeps_its_compositions_account() {
    let (app, _logs) = app_and_logs(ThreadProvider::with(vec![original_message("m1")]));
    app.dispatch(Intent::RefreshMail).await;
    let new = composition("new");
    app.open_composer(
        new.clone(),
        ComposeOrigin::New { account: None },
        None,
        &staging_dir("new"),
    )
    .await
    .expect("a new message opens");

    let opening = app
        .open_composer(
            new,
            moved(ComposerState {
                from: None,
                ..state()
            }),
            None,
            &staging_dir("moved-new"),
        )
        .await
        .expect("the composer moves");

    assert_eq!(opening.kind, ComposeKind::New);
    assert_eq!(opening.account, "acct-1");
}

/// Moving a composer the core does not know is refused: its saves would land beside a draft the
/// composer it claims to continue saved, not over it.
#[tokio::test]
async fn moving_a_composer_that_is_not_open_is_refused() {
    let (app, _logs) = app_and_logs(ThreadProvider::with(vec![original_message("m1")]));

    let refused = app
        .open_composer(
            composition("nobody"),
            moved(state()),
            None,
            &staging_dir("nobody"),
        )
        .await;

    assert!(refused.is_err());
}

/// A document that is not one is refused, and the composition it would have opened on is left as
/// it was: a composer opened on an empty body would save it over the message.
#[tokio::test]
async fn a_moved_composer_whose_document_is_not_one_is_refused() {
    let (app, logs) = app_and_logs(ThreadProvider::with(vec![original_message("m1")]));
    app.dispatch(Intent::RefreshMail).await;
    let new = composition("broken");
    app.open_composer(
        new.clone(),
        ComposeOrigin::New { account: None },
        None,
        &staging_dir("broken"),
    )
    .await
    .expect("a new message opens");
    app.dispatch(save(&new, "Kept")).await;

    let refused = app
        .open_composer(
            new.clone(),
            moved(ComposerState {
                document: "<p>not a document</p>".to_owned(),
                ..state()
            }),
            None,
            &staging_dir("broken-moved"),
        )
        .await;

    assert!(refused.is_err());
    assert!(
        app.draft_is_stored(&new),
        "the composition still holds its draft"
    );
    assert_eq!(logs.draft_puts.lock().unwrap().len(), 1);
}
