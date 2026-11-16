//! The refreshing Graph calendar provider, over a token source that never touches the network.

use super::{
    super::token_source::test_support::{
        assert_calendar_answers_reach_the_delegate, dead_grant_token_endpoint, mock_token_endpoint,
        source_at,
    },
    *,
};

/// This wrapper forwards each verb by hand, and one it leaves out answers with the trait's
/// default: an invitation answer refused as unsupported, although Graph answers it.
#[tokio::test]
async fn invitation_answers_are_forwarded_to_graph() {
    let (endpoint, _hits) = dead_grant_token_endpoint();
    let provider = RefreshingGraphCalendarProvider::new(
        source_at(endpoint, None),
        rolling_window().unwrap(),
        TimeZoneId::utc(),
        tls_with(&[]).unwrap(),
        MailboxPrincipal::Me,
    );
    assert_calendar_answers_reach_the_delegate(&provider).await;
}

#[test]
fn serves_every_calendar_with_enforced_writes_and_a_scope_each() {
    let (endpoint, _hits) = mock_token_endpoint(vec![]);
    let source = source_at(endpoint, None);
    let account = AccountId::try_from("alice@example.com@graph.microsoft.com").unwrap();
    let calendar = CalendarId::try_from("cal-123").unwrap();
    let provider = RefreshingGraphCalendarProvider::new(
        source,
        rolling_window().unwrap(),
        TimeZoneId::utc(),
        tls_with(&[]).unwrap(),
        MailboxPrincipal::Me,
    );
    // Calendar capability with a server-enforced write guard, and no mail capability.
    let info = provider.connection_info();
    assert!(info.capabilities.calendars());
    assert!(!info.capabilities.mail());
    assert_eq!(
        info.capabilities.calendar_write_guard(),
        Some(WriteGuard::Enforced),
    );
    // Reported before any delegate exists, and it has to be: a host that read `false` here
    // would conclude the answer needs an iMIP message of its own and send one *beside* the
    // one Graph already sent, so the organiser gets the reply twice.
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
fn the_fetch_window_brackets_today() {
    let today = OffsetDateTime::now_utc().date();
    let window = rolling_window().unwrap();
    // Start is before today and end after it, so a freshly connected provider covers the
    // current week the grid opens on.
    assert_eq!(
        window.start,
        calendar_date(today - Duration::days(WINDOW_DAYS_BACK)).unwrap(),
    );
    assert_eq!(
        window.end,
        calendar_date(today + Duration::days(WINDOW_DAYS_AHEAD)).unwrap(),
    );
}

#[test]
fn it_answers_for_a_shared_store_by_address_before_its_first_call() {
    // Settings asks before anything has synced: a wrapper that waited for its delegate would read
    // as a sign-in that opens no shared calendar.
    let (endpoint, _hits) = mock_token_endpoint(vec![]);
    let provider = RefreshingGraphCalendarProvider::new(
        source_at(endpoint, None),
        rolling_window().unwrap(),
        TimeZoneId::utc(),
        tls_with(&[]).unwrap(),
        MailboxPrincipal::Me,
    );
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
    let provider = RefreshingGraphCalendarProvider::new(
        source_at(endpoint, None),
        rolling_window().unwrap(),
        TimeZoneId::utc(),
        tls_with(&[]).unwrap(),
        MailboxPrincipal::Me,
    );
    let err = provider
        .resolve_shared_account("rooms@example.org")
        .await
        .unwrap_err();
    assert_eq!(
        err.class(),
        engine_core::error::FailureClass::Authentication,
        "{err}"
    );
}
