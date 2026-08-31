//! What the app does with the calendar server's own report on the reply it promised to send;
//! RFC 6638 §3.2.9, arriving on the RSVP write receipt.
//!
//! These go through the real `Intent`, because the decision is a *dispatch*: the client sends a
//! choice, and whether a question comes back depends on what the server said while storing it.
//!
//! # The one that must never regress
//!
//! Two servers disagree about this parameter, and the disagreement is the whole design.
//! Stalwart delivers replies correctly and writes **no** status at all; Soverin writes `5.2`
//! and delivers nothing. So **silence carries no information**, and reading it as either answer
//! is a bug in one direction or the other:
//!
//! - silence as *failure* → every Stalwart user is asked to email an organiser who already has the
//!   reply, and answering "yes" sends a duplicate;
//! - a reported failure as *success* → "You accepted" is the only thing a Soverin user ever sees,
//!   while the organiser was never told.
//!
//! Both directions are pinned below.

use std::sync::{Arc, Mutex};

use engine_provider::ReplyDelivery;
use fakes::{InvitationFake, MESSAGE_KEY, invitation_app, invitation_app_with_prefs};

use super::{Intent, InvitationResponse, MessageRef, Surface};

#[allow(clippy::duplicate_mod)]
#[path = "tests_fakes.rs"]
mod fakes;

/// The message the invitation arrived in, named the way a client names it.
fn invite() -> MessageRef {
    MessageRef {
        account: engine_api::AccountId::try_from("acct-a").unwrap(),
        key: engine_api::ProviderKey::new(MESSAGE_KEY).unwrap(),
    }
}

/// A scratch preferences path, cleaned first so a rerun starts from no stored choice.
fn scratch(name: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("mailcal-reply-delivery-{name}"));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir.join("preferences.toml")
}

/// Stores this account's standing answer to the reply-fallback question.
fn remember(prefs: &std::path::Path, choice: mailcal_account::ReplyFallback) {
    let mut stored = mailcal_account::load_preferences(prefs);
    stored.set_reply_fallback("acct-a", choice);
    mailcal_account::save_preferences(prefs, &stored).unwrap();
}

/// Boots an app whose server reports `delivery`, syncs, and accepts the invitation.
async fn accept_with(
    delivery: ReplyDelivery,
) -> (super::App<InvitationFake>, Arc<Mutex<Vec<Surface>>>) {
    accept_with_prefs(delivery, None).await
}

/// The same, over a preferences file: for the account choices that are only readable there.
async fn accept_with_prefs(
    delivery: ReplyDelivery,
    prefs: Option<std::path::PathBuf>,
) -> (super::App<InvitationFake>, Arc<Mutex<Vec<Surface>>>) {
    let surfaces = Arc::new(Mutex::new(Vec::new()));
    let app = invitation_app_with_prefs(
        InvitationFake::new().reporting_delivery(delivery),
        &surfaces,
        prefs,
    );
    app.dispatch(Intent::RefreshMail).await;
    app.dispatch(Intent::RefreshCalendar).await;
    app.dispatch(Intent::RespondToInvitation {
        message: invite(),
        response: InvitationResponse::Accept,
        comment: None,
        notify_organizer: true,
        reply_subject: None,
    })
    .await;
    (app, surfaces)
}

#[tokio::test]
async fn a_server_that_reports_nothing_raises_no_question() {
    // Stalwart's shape, and the overwhelmingly common one. It delivered the reply and said
    // nothing about it; treating that silence as a failure would ask every user of a working
    // server to send a duplicate.
    let (app, _) = accept_with(ReplyDelivery::NotReported).await;
    assert_eq!(app.reply_prompt(), None);
}

#[tokio::test]
async fn a_server_that_reports_success_raises_no_question() {
    let (app, _) = accept_with(ReplyDelivery::Delivered {
        status: "1.1".to_owned(),
    })
    .await;
    assert_eq!(app.reply_prompt(), None);
}

#[tokio::test]
async fn a_reported_failure_becomes_a_question_for_the_user() {
    // Soverin's shape. The answer is stored either way: this is only about whether anyone
    // was *told*: so the prompt is the sole thing that stops the failure being silent.
    let (app, _) = accept_with(ReplyDelivery::Failed {
        status: "5.2".to_owned(),
    })
    .await;
    let prompt = app.reply_prompt().expect("a failure must raise a prompt");
    assert_eq!(prompt.status_code, "5.2");
    assert_eq!(prompt.response, InvitationResponse::Accept);
}

#[tokio::test]
async fn the_question_names_the_meeting_and_who_would_be_emailed() {
    // A person consenting to send mail as themselves is entitled to see the recipient, and
    // `mailto:` is protocol punctuation rather than something to show them.
    let (app, _) = accept_with(ReplyDelivery::Failed {
        status: "5.2".to_owned(),
    })
    .await;
    let prompt = app.reply_prompt().expect("prompt");
    assert!(!prompt.summary.is_empty(), "the meeting must be named");
    assert!(
        !prompt.organizer.starts_with("mailto:"),
        "the address reaches the UI unprefixed, got {}",
        prompt.organizer
    );
    assert!(prompt.organizer.contains('@'), "{}", prompt.organizer);
}

#[tokio::test]
async fn raising_the_question_signals_the_surface() {
    // A modal the core cannot open is one a client has to poll for.
    let (_app, surfaces) = accept_with(ReplyDelivery::Failed {
        status: "5.2".to_owned(),
    })
    .await;
    assert!(
        surfaces.lock().unwrap().contains(&Surface::InvitationReply),
        "the reply surface must change when a prompt is raised"
    );
}

#[tokio::test]
async fn an_unrecognized_status_class_is_not_treated_as_a_failure() {
    // RFC 5546 §3.6 defines no 4.x. Guessing "failed" would email an organiser who may well
    // already have the reply; the token is logged instead, which is where a support
    // conversation about an unusual server starts.
    let (app, _) = accept_with(ReplyDelivery::Unrecognized {
        status: "4.0".to_owned(),
    })
    .await;
    assert_eq!(app.reply_prompt(), None);
}

#[tokio::test]
async fn the_answer_is_still_stored_when_the_reply_could_not_be_delivered() {
    // The failure is about *telling the organiser*, not about the RSVP. Failing the whole
    // action would tell the user their answer did not happen when it did, and invite them to
    // press the button again, which would answer twice.
    let provider = InvitationFake::new().reporting_delivery(ReplyDelivery::Failed {
        status: "5.2".to_owned(),
    });
    let rsvps = provider.rsvps();
    let surfaces = Arc::new(Mutex::new(Vec::new()));
    let app = invitation_app(provider, &surfaces);
    app.dispatch(Intent::RefreshMail).await;
    app.dispatch(Intent::RefreshCalendar).await;
    app.dispatch(Intent::RespondToInvitation {
        message: invite(),
        response: InvitationResponse::Accept,
        comment: None,
        notify_organizer: true,
        reply_subject: None,
    })
    .await;
    assert_eq!(
        rsvps.lock().unwrap().len(),
        1,
        "the RSVP must still reach the calendar"
    );
    assert!(app.reply_prompt().is_some());
}

#[tokio::test]
async fn answering_the_question_clears_it_and_signals_the_surface() {
    let (app, surfaces) = accept_with(ReplyDelivery::Failed {
        status: "5.2".to_owned(),
    })
    .await;
    assert!(app.reply_prompt().is_some());
    surfaces.lock().unwrap().clear();

    app.dispatch(Intent::AnswerReplyPrompt {
        send: false,
        remember: false,
        reply_subject: None,
    })
    .await;

    assert_eq!(app.reply_prompt(), None, "the question must be gone");
    assert!(
        surfaces.lock().unwrap().contains(&Surface::InvitationReply),
        "dismissing must close the modal too"
    );
}

#[tokio::test]
async fn a_second_answer_to_the_same_question_sends_nothing() {
    // Two taps on a modal that has not closed yet, or a client dispatching on both press and
    // release. The organiser must not receive the reply twice: the second arriving with no
    // way for them to tell it from a change of mind.
    let provider = InvitationFake::new().reporting_delivery(ReplyDelivery::Failed {
        status: "5.2".to_owned(),
    });
    let sends = provider.sends();
    let surfaces = Arc::new(Mutex::new(Vec::new()));
    let app = invitation_app(provider, &surfaces);
    app.dispatch(Intent::RefreshMail).await;
    app.dispatch(Intent::RefreshCalendar).await;
    app.dispatch(Intent::RespondToInvitation {
        message: invite(),
        response: InvitationResponse::Accept,
        comment: None,
        notify_organizer: true,
        reply_subject: None,
    })
    .await;
    assert!(app.reply_prompt().is_some());

    for _ in 0..2 {
        app.dispatch(Intent::AnswerReplyPrompt {
            send: true,
            remember: false,
            reply_subject: None,
        })
        .await;
    }

    assert_eq!(
        sends.lock().unwrap().len(),
        1,
        "the organizer must be emailed exactly once"
    );
}

#[tokio::test]
async fn a_remembered_never_is_honoured_without_asking() {
    let prefs = scratch("never");
    remember(&prefs, mailcal_account::ReplyFallback::Never);

    let (app, _) = accept_with_prefs(
        ReplyDelivery::Failed {
            status: "5.2".to_owned(),
        },
        Some(prefs),
    )
    .await;

    assert_eq!(
        app.reply_prompt(),
        None,
        "an account that said never must not be asked again"
    );
}

#[tokio::test]
async fn a_remembered_always_emails_the_reply_without_asking() {
    let prefs = scratch("always");
    remember(&prefs, mailcal_account::ReplyFallback::Always);

    let provider = InvitationFake::new().reporting_delivery(ReplyDelivery::Failed {
        status: "5.2".to_owned(),
    });
    let sends = provider.sends();
    let surfaces = Arc::new(Mutex::new(Vec::new()));
    let app = invitation_app_with_prefs(provider, &surfaces, Some(prefs));
    app.dispatch(Intent::RefreshMail).await;
    app.dispatch(Intent::RefreshCalendar).await;
    app.dispatch(Intent::RespondToInvitation {
        message: invite(),
        response: InvitationResponse::Accept,
        comment: None,
        notify_organizer: true,
        reply_subject: None,
    })
    .await;

    assert_eq!(app.reply_prompt(), None, "a standing yes is not a question");
    assert_eq!(
        sends.lock().unwrap().len(),
        1,
        "the reply must have gone out as mail"
    );
}

#[tokio::test]
async fn remembering_the_answer_stops_the_next_meeting_asking() {
    let prefs = scratch("remember");

    let (app, _) = accept_with_prefs(
        ReplyDelivery::Failed {
            status: "5.2".to_owned(),
        },
        Some(prefs.clone()),
    )
    .await;
    assert!(app.reply_prompt().is_some(), "the first one asks");

    app.dispatch(Intent::AnswerReplyPrompt {
        send: false,
        remember: true,
        reply_subject: None,
    })
    .await;

    assert_eq!(
        mailcal_account::load_preferences(&prefs).reply_fallback("acct-a"),
        mailcal_account::ReplyFallback::Never,
        "the choice must survive for the next meeting on this server"
    );
}

// ---------------------------------------------------------------------------------------------
// The debug-only verdict override (`MAILCAL_FAKE_REPLY_DELIVERY`), which is how a UI suite
// reaches the failure path at all: no harness server reports one. Only the parsing is tested
// here: reading the variable itself is process-global state, and a test that wrote it would
// decide the outcome of whichever other test happened to be running beside it.
// ---------------------------------------------------------------------------------------------

// Both carry the same `cfg` as the code they cover: the hook is compiled out of a release
// build on purpose, so under `cargo test --release` these would not fail, they would not
// compile. The workspace gate runs debug.
#[cfg(debug_assertions)]
#[test]
fn the_pretend_verdict_names_a_variant_rather_than_classifying_a_status() {
    // The point of the form: this hook never decides which class `5.2` belongs to. That is
    // protocol knowledge and it lives in the engine, so the caller says which variant it wants
    // and the token is carried through untouched.
    assert_eq!(
        super::invitations_fallback::parse_pretended_delivery("failed:5.2"),
        Some(ReplyDelivery::Failed {
            status: "5.2".to_owned()
        })
    );
    assert_eq!(
        super::invitations_fallback::parse_pretended_delivery("delivered:2.0"),
        Some(ReplyDelivery::Delivered {
            status: "2.0".to_owned()
        })
    );
    assert_eq!(
        super::invitations_fallback::parse_pretended_delivery("unrecognized:9.9"),
        Some(ReplyDelivery::Unrecognized {
            status: "9.9".to_owned()
        })
    );
    assert_eq!(
        super::invitations_fallback::parse_pretended_delivery("  notreported  "),
        Some(ReplyDelivery::NotReported),
        "surrounding whitespace is a shell's doing, not the caller's intent"
    );
}

#[cfg(debug_assertions)]
#[test]
fn a_pretend_verdict_that_says_nothing_useful_is_declined_rather_than_guessed() {
    // Each of these would otherwise reach a prompt (and a log line) as a blank, or as a
    // silently different verdict from the one the author typed. Refusing sends the run back to
    // the server's real answer, which is the safe direction: it under-tests rather than
    // inventing a failure that never happened.
    for raw in [
        "",
        "failed",          // no status: the prompt's log line would carry an empty token
        "failed:",         // same, spelled out
        "5.2",             // a status with no variant: the classification we refuse to do
        "Failed:5.2",      // deliberately case-sensitive; a near-miss must not half-work
        "notreported:2.0", // the variant that carries nothing, given something
        "delivered",
        "nonsense:5.2",
    ] {
        assert_eq!(
            super::invitations_fallback::parse_pretended_delivery(raw),
            None,
            "{raw:?} must not be read as a verdict"
        );
    }
}
