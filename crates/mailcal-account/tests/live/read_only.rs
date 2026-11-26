//! What the read-only live suites share: a token sink that counts refreshes and rotations, a way
//! to fail a step that says only the failure's class, and a garbage token to seed.
//!
//! Nothing here prints a token, an address, a subject or any content: a real account's server
//! text can echo any of them, so a failed step names its class and nothing else.

use std::sync::atomic::{AtomicUsize, Ordering};

use engine_core::{
    ids::AccountId,
    sync::{SyncObject, SyncUpdate, SyncWindow},
    time::CalendarDate,
};
use engine_provider::{Provider, ProviderResult};
use futures::StreamExt;
use mailcal_account::TokenSink;
use mailcal_oauth::GrantedScopes;
use time::{Duration, OffsetDateTime};

/// Counts the account's refreshes (each names the scopes it granted) and the refresh tokens the
/// server rotated, which the suites report and never write back.
#[derive(Debug, Default)]
pub(super) struct CountingSink {
    refreshes: AtomicUsize,
    rotations: AtomicUsize,
}

impl CountingSink {
    pub(super) fn refreshes(&self) -> usize {
        self.refreshes.load(Ordering::SeqCst)
    }

    pub(super) fn rotations(&self) -> usize {
        self.rotations.load(Ordering::SeqCst)
    }
}

#[async_trait::async_trait]
impl TokenSink for CountingSink {
    async fn refresh_token_rotated(&self, _account: &AccountId, _new_refresh_token: &str) {
        self.rotations.fetch_add(1, Ordering::SeqCst);
    }

    async fn scopes_granted(&self, _account: &AccountId, _granted: &GrantedScopes) {
        self.refreshes.fetch_add(1, Ordering::SeqCst);
    }
}

/// `result`'s value, or a failure naming `step` and the class it failed as.
pub(super) fn step<T>(step: &str, result: ProviderResult<T>) -> T {
    match result {
        Ok(value) => value,
        Err(err) => panic!("{step}: failed as {:?}", err.class()),
    }
}

/// `result`'s value, or a failure naming `step` alone: an account error's text may carry the
/// account's address.
pub(super) fn connected<T, E>(step: &str, result: Result<T, E>) -> T {
    result.unwrap_or_else(|_| panic!("{step}: failed"))
}

/// A token no server issued, valid for an hour as far as the source knows.
pub(super) fn garbage() -> (String, OffsetDateTime) {
    (
        "not-a-token-any-server-issued".to_owned(),
        OffsetDateTime::now_utc() + Duration::hours(1),
    )
}

/// A week back, the window a read-only pass reads.
fn a_week_back() -> CalendarDate {
    let date = (OffsetDateTime::now_utc() - Duration::days(7)).date();
    CalendarDate::new(date.year(), u8::from(date.month()), date.day()).expect("a valid date")
}

/// How many objects a listing carried.
pub(super) fn listed<T: SyncObject>(update: &SyncUpdate<T>) -> usize {
    match update {
        SyncUpdate::Snapshot { objects, .. } => objects.len(),
        SyncUpdate::Delta { changed, .. } => changed.len(),
    }
}

/// The messages in a week of `provider`'s mail, its first chunk alone.
pub(super) async fn a_page(provider: &dyn Provider, account: &AccountId) -> ProviderResult<usize> {
    let window = SyncWindow::since(a_week_back()).into();
    let mut stream = provider.stream_email(account, None, window, 50, 50);
    Ok(match stream.next().await {
        Some(chunk) => chunk?.changed.len(),
        None => 0,
    })
}
