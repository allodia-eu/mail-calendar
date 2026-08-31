//! Tests for the **send-from account**: the composer's From dropdown (`Intent`'s `from`) and the
//! persisted default-send-account fallback. A rich send now names the account it goes out from,
//! which decides *both* the `From:` identity and the outbox (provider) it is submitted through;
//! so these assert the draft's `from` **and** which account's provider recorded the submission.
//!
//! A child of [`super`] (the rich reply/forward tests), reusing its `ThreadProvider`,
//! `original_message`, `reply_document`, and `dispatch_until` fixtures; split into its own file to
//! keep each test module under the 500-line limit.

use std::sync::{Arc, Mutex};

use engine_api::{AccountId, Draft, EmailAddress, Engine, MessageIdHeader, TimeZoneId};
use engine_core::mail::Message;

use super::{SilentObserver, ThreadProvider, dispatch_until, original_message, reply_document};
use crate::{Account, App, Intent, MessageRef, SendStatus, Telemetry, TimeZoneInit};

/// The submission logs of a two-account app, one per account.
struct Outboxes {
    first: Arc<Mutex<Vec<Draft>>>,
    second: Arc<Mutex<Vec<Draft>>>,
}

impl Outboxes {
    /// The single draft `acct-1` submitted, panicking if it didn't submit exactly one.
    fn only_first(&self) -> Draft {
        Self::only(&self.first, "acct-1")
    }

    /// The single draft `acct-2` submitted, panicking if it didn't submit exactly one.
    fn only_second(&self) -> Draft {
        Self::only(&self.second, "acct-2")
    }

    fn only(log: &Arc<Mutex<Vec<Draft>>>, label: &str) -> Draft {
        let drafts = log.lock().unwrap();
        assert_eq!(drafts.len(), 1, "expected one submission on {label}");
        drafts[0].clone()
    }

    fn counts(&self) -> (usize, usize) {
        (
            self.first.lock().unwrap().len(),
            self.second.lock().unwrap().len(),
        )
    }
}

/// Builds a **two-account** app; `acct-1` (`me@allodia.local`) seeded with `messages`, and
/// `acct-2` (`other@allodia.local`) with an empty mailbox; each over its own `ThreadProvider`,
/// so a test can tell which account's outbox a send went through. Starts in the unified
/// all-inboxes view (no selected account), which is where the default-send-account fallback runs.
fn two_account_app(messages: Vec<Message>) -> (Arc<App<ThreadProvider>>, Outboxes) {
    let first = ThreadProvider::with(messages);
    let second = ThreadProvider::with(Vec::new());
    let outboxes = Outboxes {
        first: first.submissions(),
        second: second.submissions(),
    };
    let app = App::new(
        Engine::open_in_memory().unwrap(),
        vec![
            Account {
                id: AccountId::try_from("acct-1").unwrap(),
                providers: vec![first],
                calendar_providers: Vec::new(),
                contact_providers: Vec::new(),
                identity: EmailAddress::new("me@allodia.local"),
            },
            Account {
                id: AccountId::try_from("acct-2").unwrap(),
                providers: vec![second],
                calendar_providers: Vec::new(),
                contact_providers: Vec::new(),
                identity: EmailAddress::new("other@allodia.local"),
            },
        ],
        TimeZoneInit {
            device_zone: TimeZoneId::utc(),
            prefs_path: None,
        },
        None,
        std::sync::Arc::new(SilentObserver),
        Telemetry::off(None),
    );
    (Arc::new(app), outboxes)
}

/// A new rich message naming `from`, with the recipients a test never varies.
fn new_mail(from: Option<&str>) -> Intent {
    let (document, blobs) = reply_document();
    Intent::SubmitRichMail {
        from: from.map(|id| AccountId::try_from(id).unwrap()),
        to: "you@remote.test".to_owned(),
        cc: String::new(),
        bcc: String::new(),
        subject: "Hello".to_owned(),
        document,
        blobs,
    }
}

#[tokio::test(start_paused = true)]
async fn an_explicit_from_picks_both_the_identity_and_the_sending_account() {
    let (app, outboxes) = two_account_app(Vec::new());

    // The unified view is showing, so without a `from` this would derive `acct-1` (the first
    // account). Naming `acct-2` must send as (and through) that account instead.
    let _task = dispatch_until(&app, new_mail(Some("acct-2")), SendStatus::Sent).await;

    assert_eq!(outboxes.counts(), (0, 1), "acct-2's outbox must carry it");
    assert_eq!(outboxes.only_second().from.email, "other@allodia.local");
}

#[tokio::test(start_paused = true)]
async fn without_a_from_a_new_message_uses_the_persisted_default_send_account() {
    let (app, outboxes) = two_account_app(Vec::new());
    // In the unified all-inboxes view (no selected account) the app-level default decides.
    app.set_default_send_account(Some("acct-2".to_owned()))
        .await;

    let _task = dispatch_until(&app, new_mail(None), SendStatus::Sent).await;

    assert_eq!(outboxes.counts(), (0, 1));
    assert_eq!(outboxes.only_second().from.email, "other@allodia.local");
}

#[tokio::test(start_paused = true)]
async fn a_default_send_account_that_is_no_longer_configured_falls_back_to_the_first() {
    let (app, outboxes) = two_account_app(Vec::new());
    // The chosen account was removed after being made the default; the derived path must
    // degrade to the first configured account rather than dropping the send.
    app.set_default_send_account(Some("acct-gone".to_owned()))
        .await;

    let _task = dispatch_until(&app, new_mail(None), SendStatus::Sent).await;

    assert_eq!(outboxes.counts(), (1, 0));
    assert_eq!(outboxes.only_first().from.email, "me@allodia.local");
}

#[tokio::test(start_paused = true)]
async fn selecting_an_account_outranks_the_default_send_account() {
    let (app, outboxes) = two_account_app(Vec::new());
    // The default names acct-2, but acct-1's mailbox is the one on screen: the visible mailbox
    // scopes the choice, so it wins.
    app.set_default_send_account(Some("acct-2".to_owned()))
        .await;
    app.dispatch(Intent::SelectAccount(Some("acct-1".to_owned())))
        .await;

    let _task = dispatch_until(&app, new_mail(None), SendStatus::Sent).await;

    assert_eq!(outboxes.counts(), (1, 0));
    assert_eq!(outboxes.only_first().from.email, "me@allodia.local");
}

#[tokio::test(start_paused = true)]
async fn a_from_naming_an_unconfigured_account_fails_the_send_rather_than_substituting() {
    let (app, outboxes) = two_account_app(Vec::new());

    // The account was removed while the composer was open. Sending as `acct-1` instead would
    // silently deliver the message under a sender the user never chose, so the send must fail.
    let _task = dispatch_until(&app, new_mail(Some("acct-gone")), SendStatus::Failed).await;

    assert_eq!(app.send_status(), SendStatus::Failed);
    assert_eq!(outboxes.counts(), (0, 0), "nothing may reach an outbox");
}

#[tokio::test(start_paused = true)]
async fn a_reply_from_another_account_still_threads_off_the_original() {
    let (app, outboxes) = two_account_app(vec![original_message("m1")]);
    // Sync so the original lands in acct-1's store; the reply resolves it there.
    app.dispatch(Intent::RefreshMail).await;

    let (document, blobs) = reply_document();
    let intent = Intent::SubmitRichReply {
        // The original lives in acct-1 …
        message: MessageRef::from_parts("acct-1", "m1".to_owned()).unwrap(),
        // … but the user picked acct-2 in the From dropdown.
        from: Some(AccountId::try_from("acct-2").unwrap()),
        to: "reply@remote.test".to_owned(),
        cc: String::new(),
        bcc: String::new(),
        document,
        blobs,
    };
    let _task = dispatch_until(&app, intent, SendStatus::Sent).await;

    // It goes out from acct-2 …
    assert_eq!(outboxes.counts(), (0, 1));
    let draft = outboxes.only_second();
    assert_eq!(draft.from.email, "other@allodia.local");
    // … while the subject and threading headers still come from acct-1's original, so the
    // reply lands on the right conversation in the recipient's client.
    assert_eq!(draft.subject, "Re: Quarterly report");
    assert_eq!(
        draft.in_reply_to.as_ref().map(MessageIdHeader::as_str),
        Some("parent@remote")
    );
    let references: Vec<&str> = draft
        .references
        .iter()
        .map(MessageIdHeader::as_str)
        .collect();
    assert_eq!(references, vec!["root@remote", "parent@remote"]);
}

#[tokio::test(start_paused = true)]
async fn a_reply_without_a_from_still_sends_from_the_receiving_account() {
    let (app, outboxes) = two_account_app(vec![original_message("m1")]);
    app.dispatch(Intent::RefreshMail).await;

    let (document, blobs) = reply_document();
    let intent = Intent::SubmitRichReply {
        message: MessageRef::from_parts("acct-1", "m1".to_owned()).unwrap(),
        // No dropdown choice: the account that received the original replies (the default).
        from: None,
        to: "reply@remote.test".to_owned(),
        cc: String::new(),
        bcc: String::new(),
        document,
        blobs,
    };
    let _task = dispatch_until(&app, intent, SendStatus::Sent).await;

    assert_eq!(outboxes.counts(), (1, 0));
    assert_eq!(outboxes.only_first().from.email, "me@allodia.local");
}

#[tokio::test(start_paused = true)]
async fn a_forward_from_another_account_sends_through_that_accounts_outbox() {
    let (app, outboxes) = two_account_app(vec![original_message("m1")]);
    app.dispatch(Intent::RefreshMail).await;

    let (document, blobs) = reply_document();
    let intent = Intent::SubmitRichForward {
        message: MessageRef::from_parts("acct-1", "m1".to_owned()).unwrap(),
        from: Some(AccountId::try_from("acct-2").unwrap()),
        to: "elsewhere@remote.test".to_owned(),
        cc: String::new(),
        bcc: String::new(),
        document,
        blobs,
    };
    let _task = dispatch_until(&app, intent, SendStatus::Sent).await;

    assert_eq!(outboxes.counts(), (0, 1));
    let draft = outboxes.only_second();
    assert_eq!(draft.from.email, "other@allodia.local");
    // A forward derives its subject from the original and stays on its thread; including
    // when it is sent from a different account than the one the original arrived on.
    assert_eq!(draft.subject, "Fwd: Quarterly report");
    assert!(draft.in_reply_to.is_none());
    let references: Vec<&str> = draft
        .references
        .iter()
        .map(MessageIdHeader::as_str)
        .collect();
    assert_eq!(references, vec!["root@remote", "parent@remote"]);
}
