//! Tests for the redial loop: a dropped socket is redialled once and the call retried,
//! and a delegate's own failure is surfaced rather than masked as a reconnect.
//!
//! In their own `#[path]` file to keep `reconnect.rs` under the 500-line limit.

use std::{
    collections::BTreeSet,
    sync::atomic::{AtomicUsize, Ordering},
};

use engine_core::{ids::MessageIdHeader, mail::EmailAddress, sync::SyncUpdate};
use engine_provider::{ProviderError, TlsVersion};

use super::*;

/// A scripted delegate: every mail call increments a shared counter and returns either
/// `Ok` (an empty snapshot) or a classified error, so a test can assert whether the
/// wrapper retried on a fresh delegate.
struct FakeDelegate {
    calls: Arc<AtomicUsize>,
    outcome: Option<FailureClass>,
    info: ConnectionInfo,
}

impl FakeDelegate {
    fn arc(calls: Arc<AtomicUsize>, outcome: Option<FailureClass>) -> Arc<dyn Provider> {
        Self::arc_with_info(
            calls,
            outcome,
            ConnectionInfo::new(Capabilities::none().with_mail()),
        )
    }

    fn arc_with_info(
        calls: Arc<AtomicUsize>,
        outcome: Option<FailureClass>,
        info: ConnectionInfo,
    ) -> Arc<dyn Provider> {
        Arc::new(Self {
            calls,
            outcome,
            info,
        })
    }

    fn record(&self) -> ProviderResult<()> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        match self.outcome {
            None => Ok(()),
            Some(class) => Err(ProviderError::new(class, "scripted failure")),
        }
    }
}

// Adopted as `Arc<dyn Provider>`, so it has to be one. The rejecting default
// is right for these tests: they exercise the redial loop, not reporting.
#[async_trait]
impl Provider for FakeDelegate {
    fn connection_info(&self) -> ConnectionInfo {
        self.info
    }

    async fn sync_mailboxes(
        &self,
        _account: &AccountId,
        _cursor: Option<&SyncState>,
    ) -> ProviderResult<ScopeSync<Mailbox>> {
        self.record()?;
        Ok(ScopeSync::new(
            SyncUpdate::snapshot(Vec::new(), BTreeSet::new()),
            SyncState::new("cursor"),
        ))
    }

    async fn submit_email(
        &self,
        _account: &AccountId,
        _draft: &Draft,
    ) -> ProviderResult<SubmissionReceipt> {
        self.record()?;
        unreachable!("submit tests only script a failing outcome");
    }
}

fn account() -> AccountId {
    AccountId::try_from("test@example.com").expect("valid account id")
}

fn mailbox() -> MailboxId {
    MailboxId::try_from("INBOX").expect("valid mailbox id")
}

/// A `redial` that produces an always-succeeding delegate and counts how often it fires.
fn healthy_redial(redials: Arc<AtomicUsize>) -> Redial {
    let calls = Arc::new(AtomicUsize::new(0));
    Box::new(move || {
        redials.fetch_add(1, Ordering::SeqCst);
        let calls = Arc::clone(&calls);
        Box::pin(async move { Ok(FakeDelegate::arc(calls, None)) })
    })
}

/// A `redial` that reports a specific connection fact on the fresh delegate.
fn healthy_redial_with_info(redials: Arc<AtomicUsize>, info: ConnectionInfo) -> Redial {
    let calls = Arc::new(AtomicUsize::new(0));
    Box::new(move || {
        redials.fetch_add(1, Ordering::SeqCst);
        let calls = Arc::clone(&calls);
        Box::pin(async move { Ok(FakeDelegate::arc_with_info(calls, None, info)) })
    })
}

#[tokio::test]
async fn retries_once_on_a_fresh_session_after_a_retryable_drop() {
    let redials = Arc::new(AtomicUsize::new(0));
    // The adopted session fails once with a dead-socket error…
    let initial = FakeDelegate::arc(Arc::new(AtomicUsize::new(0)), Some(FailureClass::Retryable));
    let provider = ReconnectingImapProvider::adopt(
        initial,
        mailbox(),
        healthy_redial(Arc::clone(&redials)),
        AuthRenewal::Impossible,
    );

    // …and the call still succeeds, because the wrapper re-dialed and retried.
    let result = provider.sync_mailboxes(&account(), None).await;
    assert!(
        result.is_ok(),
        "expected the retry on a fresh session to succeed"
    );
    assert_eq!(
        redials.load(Ordering::SeqCst),
        1,
        "reconnected exactly once"
    );
}

#[tokio::test]
async fn connection_info_tracks_the_current_session_after_redial() {
    let redials = Arc::new(AtomicUsize::new(0));
    let initial_info = ConnectionInfo {
        tls_version: Some(TlsVersion::Tls1_2),
        ..ConnectionInfo::new(Capabilities::none().with_mail())
    };
    let redial_info = ConnectionInfo {
        tls_version: Some(TlsVersion::Tls1_3),
        ..ConnectionInfo::new(Capabilities::none().with_mail())
    };
    let initial = FakeDelegate::arc_with_info(
        Arc::new(AtomicUsize::new(0)),
        Some(FailureClass::Retryable),
        initial_info,
    );
    let provider = ReconnectingImapProvider::adopt(
        initial,
        mailbox(),
        healthy_redial_with_info(Arc::clone(&redials), redial_info),
        AuthRenewal::Impossible,
    );

    assert_eq!(
        provider.connection_info().tls_version,
        Some(TlsVersion::Tls1_2)
    );
    let result = provider.sync_mailboxes(&account(), None).await;
    assert!(result.is_ok(), "redial should recover the scripted drop");
    assert_eq!(
        provider.connection_info().tls_version,
        Some(TlsVersion::Tls1_3)
    );
}

#[tokio::test]
async fn does_not_reconnect_on_a_non_retryable_error() {
    let redials = Arc::new(AtomicUsize::new(0));
    let initial = FakeDelegate::arc(
        Arc::new(AtomicUsize::new(0)),
        Some(FailureClass::Authentication),
    );
    let provider = ReconnectingImapProvider::adopt(
        initial,
        mailbox(),
        healthy_redial(Arc::clone(&redials)),
        AuthRenewal::Impossible,
    );

    let result = provider.sync_mailboxes(&account(), None).await;
    assert!(result.is_err(), "an auth error is not a transport drop");
    assert_eq!(
        redials.load(Ordering::SeqCst),
        0,
        "must not re-dial on auth failure"
    );
}

#[tokio::test]
async fn a_send_is_never_blind_retried() {
    let redials = Arc::new(AtomicUsize::new(0));
    let submits = Arc::new(AtomicUsize::new(0));
    let initial = FakeDelegate::arc(Arc::clone(&submits), Some(FailureClass::Retryable));
    let provider = ReconnectingImapProvider::adopt(
        initial,
        mailbox(),
        healthy_redial(Arc::clone(&redials)),
        AuthRenewal::Impossible,
    );

    let draft = Draft::new(
        MessageIdHeader::new("m@example.com").expect("valid message id"),
        EmailAddress::new("from@example.com"),
        vec![EmailAddress::new("to@example.com")],
        "Subject",
        "Body",
    );
    let result = provider.submit_email(&account(), &draft).await;
    assert!(result.is_err(), "a retryable send surfaces the error");
    assert_eq!(
        submits.load(Ordering::SeqCst),
        1,
        "the send was attempted exactly once"
    );
    assert_eq!(
        redials.load(Ordering::SeqCst),
        0,
        "submit invalidates but does not re-dial"
    );
}

#[tokio::test]
async fn an_oauth_account_re_dials_once_on_an_authentication_failure() {
    // The mid-session expiry an OAuth account meets routinely: the engine deliberately does
    // not refresh a token itself, so the only thing that recovers a session whose token aged
    // out is this re-dial, which mints a fresh one. Without it the user sees "sign in again"
    // for an account whose grant is perfectly valid.
    let redials = Arc::new(AtomicUsize::new(0));
    let initial = FakeDelegate::arc(
        Arc::new(AtomicUsize::new(0)),
        Some(FailureClass::Authentication),
    );
    let provider = ReconnectingImapProvider::adopt(
        initial,
        mailbox(),
        healthy_redial(Arc::clone(&redials)),
        AuthRenewal::MintsAFreshToken,
    );

    let result = provider.sync_mailboxes(&account(), None).await;
    assert!(
        result.is_ok(),
        "the re-dialled session should serve the call"
    );
    assert_eq!(
        redials.load(Ordering::SeqCst),
        1,
        "exactly once: a second attempt would present the same fresh token"
    );
}

#[tokio::test]
async fn a_password_account_never_re_dials_on_an_authentication_failure() {
    // The same failure, the other credential shape. A re-dial would send the same password
    // back to the server that just refused it, which is not a retry but a second refusal, at a
    // provider that may be counting them toward a lockout.
    let redials = Arc::new(AtomicUsize::new(0));
    let initial = FakeDelegate::arc(
        Arc::new(AtomicUsize::new(0)),
        Some(FailureClass::Authentication),
    );
    let provider = ReconnectingImapProvider::adopt(
        initial,
        mailbox(),
        healthy_redial(Arc::clone(&redials)),
        AuthRenewal::Impossible,
    );

    assert!(provider.sync_mailboxes(&account(), None).await.is_err());
    assert_eq!(redials.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn an_oauth_send_is_still_never_blind_retried() {
    // Renewal widens which *failures* are worth a re-dial, never which *operations* may be
    // repeated: a submission whose outcome is unknown must not be sent twice, whatever
    // refused it (`docs/sending.md`).
    let redials = Arc::new(AtomicUsize::new(0));
    let submits = Arc::new(AtomicUsize::new(0));
    let initial = FakeDelegate::arc(Arc::clone(&submits), Some(FailureClass::Authentication));
    let provider = ReconnectingImapProvider::adopt(
        initial,
        mailbox(),
        healthy_redial(Arc::clone(&redials)),
        AuthRenewal::MintsAFreshToken,
    );

    let draft = Draft::new(
        MessageIdHeader::new("m@example.com").expect("valid message id"),
        EmailAddress::new("from@example.com"),
        vec![EmailAddress::new("to@example.com")],
        "Subject",
        "Body",
    );
    assert!(provider.submit_email(&account(), &draft).await.is_err());
    assert_eq!(
        submits.load(Ordering::SeqCst),
        1,
        "the message must reach the server at most once"
    );
}
