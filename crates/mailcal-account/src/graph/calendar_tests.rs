//! The Graph calendar provider, over a token source that never touches the network.

use engine_core::error::FailureClass;
use engine_provider::{SharedAccounts, WriteGuard};
use provider_graph::MailboxPrincipal;

use super::{
    super::{
        client_connecting,
        token_source::test_support::{
            an_invitation, assert_calendar_reaches_the_delegate, dead_grant_token_endpoint,
            mock_token_endpoint, source_at,
        },
    },
    *,
};
use crate::http_fake;

/// A calendar provider for the signed-in user's own calendars, asking `endpoint` for tokens.
fn provider(endpoint: String) -> ReconnectingGraphCalendarProvider {
    let client = account_client(
        source_at(endpoint, None),
        MailboxPrincipal::Me,
        tls_with(&[]).unwrap(),
    );
    ReconnectingGraphCalendarProvider::new(client, TimeZoneId::utc()).unwrap()
}

/// This wrapper forwards each verb by hand, and one it leaves out answers with the trait's
/// default: a calendar fetch or an invitation answer refused as unsupported, although Graph
/// answers both.
#[tokio::test]
async fn calendar_calls_are_forwarded_to_graph() {
    let (endpoint, _hits) = dead_grant_token_endpoint();
    let provider = provider(endpoint);
    assert_calendar_reaches_the_delegate(&provider).await;
}

#[test]
fn serves_every_calendar_with_enforced_writes_and_a_scope_each() {
    let (endpoint, _hits) = mock_token_endpoint(vec![]);
    let account = AccountId::try_from("alice@example.com@graph.microsoft.com").unwrap();
    let calendar = CalendarId::try_from("cal-123").unwrap();
    let provider = provider(endpoint);
    // Calendar capability with a server-enforced write guard, and no mail capability.
    let info = provider.connection_info();
    assert!(info.capabilities.calendars());
    assert!(!info.capabilities.mail());
    assert_eq!(
        info.capabilities.calendar_write_guard(),
        Some(WriteGuard::Enforced),
    );
    // Reported before any request, and it has to be: a host that read `false` here would
    // conclude the answer needs an iMIP message of its own and send one *beside* the one Graph
    // already sent, so the organiser gets the reply twice.
    assert!(info.capabilities.calendar_scheduling());
    // Each calendar's events have a scope of their own; the calendar-list scope is
    // account-wide.
    assert_eq!(
        provider.event_scope(&account, &calendar),
        SyncScope::GraphCalendar {
            account: account.clone(),
            calendar,
        },
    );
    assert_eq!(
        provider.calendar_scope(&account),
        SyncScope::GraphCalendarList { account },
    );
}

#[test]
fn a_403_calendar_probe_is_classified_as_access_denied() {
    // A `403`/`ErrorAccessDenied` means the token lacks the calendar scope → re-consent.
    let denied = ProviderError::retryable(
        "Graph HTTP 403 (code Some(\"ErrorAccessDenied\")): access is denied",
    );
    assert!(is_calendar_access_denied(&denied));
    // A transient `500` is not a re-consent case; it should badge nothing and retry.
    let transient = ProviderError::retryable("Graph HTTP 500 (code ErrorInternalServerError)");
    assert!(!is_calendar_access_denied(&transient));
}

#[test]
fn it_answers_for_a_shared_store_by_address_before_its_first_call() {
    // Settings asks before anything has synced: a wrapper that waited for its delegate would read
    // as a sign-in that opens no shared calendar.
    let (endpoint, _hits) = mock_token_endpoint(vec![]);
    let provider = provider(endpoint);
    assert_eq!(
        provider.connection_info().capabilities.shared_accounts(),
        SharedAccounts::ByAddress
    );
}

/// Checking an address is the delegate's: the trait's default refuses it outright, which Settings
/// would say as "no calendar there".
#[tokio::test]
async fn an_address_is_checked_by_the_delegate() {
    let (endpoint, _hits) = dead_grant_token_endpoint();
    let provider = provider(endpoint);
    let err = provider
        .resolve_shared_account("rooms@example.org")
        .await
        .unwrap_err();
    assert_eq!(err.class(), FailureClass::Authentication, "{err}");
}

/// Each request asks the account's source for the token, and one Graph refused is replaced once
/// and the request sent again, on the same client.
#[tokio::test]
async fn each_request_presents_the_current_token_and_a_refused_one_is_replaced() {
    let (endpoint, minted) = mock_token_endpoint(vec![http_fake::minted(1), http_fake::minted(2)]);
    let (base, received) = http_fake::serve(http_fake::refused_then(r#"{"value":[]}"#));
    let client = client_connecting(
        source_at(endpoint, None),
        MailboxPrincipal::Me,
        tls_with(&[]).unwrap(),
        move |source, tls, retry, agent| {
            GraphClient::with_base_and_source(source, &base, tls, retry, agent)
        },
    );
    let provider = ReconnectingGraphCalendarProvider::new(client, TimeZoneId::utc()).unwrap();
    let account = AccountId::try_from("alice@example.com@graph.microsoft.com").unwrap();

    provider.sync_calendars(&account, None).await.unwrap();
    provider.sync_calendars(&account, None).await.unwrap();

    http_fake::assert_replaced_once(&received, &minted);
}

/// An answer to an invitation may have been applied before the connection died, so it is sent
/// once; the dead client is still replaced for the next request.
#[tokio::test]
async fn a_write_on_a_dead_connection_is_sent_once_and_the_client_replaced() {
    let (endpoint, _) = mock_token_endpoint(vec![http_fake::minted(1)]);
    let (base, received) = http_fake::serve(vec![http_fake::Reply::HangUp]);
    let client = client_connecting(
        source_at(endpoint, None),
        MailboxPrincipal::Me,
        tls_with(&[]).unwrap(),
        move |source, tls, retry, agent| {
            GraphClient::with_base_and_source(source, &base, tls, retry, agent)
        },
    );
    let provider = ReconnectingGraphCalendarProvider::new(client, TimeZoneId::utc()).unwrap();
    let account = AccountId::try_from("alice@example.com@graph.microsoft.com").unwrap();
    let (event, rsvp) = an_invitation();

    provider
        .rsvp_event(&account, &event, &rsvp)
        .await
        .unwrap_err();

    assert_eq!(received.lock().unwrap().len(), 1, "sent once");
    assert_eq!(provider.delegate.generation(), 1, "a new client");
}
