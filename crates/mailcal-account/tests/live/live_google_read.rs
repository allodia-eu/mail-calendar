//! Gated live check, **read-only**, that a Google account is wired as the app wires it: its mail
//! read over IMAP with Gmail's two halves bound under one credential subject, and a token the
//! server refuses replaced once, for an IMAP dial, an API read and a calendar read alike.
//!
//! It reads and nothing else: no send, no draft, no flag, no move, no label. It prints classes
//! and counts, never a token, an address, a subject or any content.
//!
//! It **skips** unless `GOOGLE_REFRESH_TOKEN`, `GOOGLE_CLIENT_ID` and `GOOGLE_TEST_ADDRESS` are
//! set, as `live_google` does, whose header says how to set them; run it on its own:
//! `cargo test -p mailcal-account --test live -- live_google_read:: --nocapture`.

use std::sync::Arc;

use engine_provider::Provider;
use mailcal_account::{
    CredentialOrigin, GmailKeywordNaming, GmailMailRoute, GraphTokenSource, TokenSink,
    connect_google_calendar_provider, connect_google_folder, connect_google_mail_providers,
    gmail_mail_route, google_token_source,
};

use super::read_only::{CountingSink, a_page, connected, garbage, listed, step};

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn live_a_google_account_reads_over_imap_and_replaces_a_refused_token_once() {
    let Some(config) = super::live_google::config() else {
        eprintln!("skipping live_google_read: GOOGLE_REFRESH_TOKEN/GOOGLE_CLIENT_ID unset");
        return;
    };
    let account = config.account_id().expect("account id");
    let sink = Arc::new(CountingSink::default());
    let source = |sink: &Arc<CountingSink>| -> Arc<GraphTokenSource> {
        connected(
            "token source",
            google_token_source(
                &config,
                account.clone(),
                Some(Arc::clone(sink) as Arc<dyn TokenSink>),
                CredentialOrigin::Stored,
            ),
        )
    };
    let naming = GmailKeywordNaming::new(false);

    // The account as the app connects it, reading its labels and a page of its mail.
    let tokens = source(&sink);
    let providers = connected(
        "connect",
        connect_google_mail_providers(
            Arc::clone(&tokens),
            None,
            &config.identity(),
            naming.clone(),
        )
        .await,
    );
    let provider = providers.first().expect("one account-global mail provider");
    let labels = step("labels", provider.sync_mailboxes(&account, None).await);
    let page = step("a page of mail", a_page(provider.as_ref(), &account).await);
    let route = gmail_mail_route(&config.email);
    let per_request = provider.connection_info().sources_per_request;
    eprintln!(
        "google: {} label(s); first chunk {page} message(s); route {route:?}; \
         {per_request} source(s) a request; {} refresh(es)",
        listed(&labels.update),
        sink.refreshes(),
    );
    assert_eq!(route, GmailMailRoute::Imap, "the mail is read over IMAP");
    assert!(
        per_request > 1,
        "the reads went through the IMAP half bound to the API half"
    );

    // A refused token on a new IMAP dial: a second sign-in of the account dials anew.
    let again = source(&sink);
    let (token, expires_at) = garbage();
    again.seed_access_token(token, expires_at);
    let before = sink.refreshes();
    let provider = connected(
        "connect again",
        connect_google_folder(Arc::clone(&again), None, &config.identity(), naming),
    );
    let page = step(
        "a page over a refused token",
        a_page(provider.as_ref(), &account).await,
    );
    let imap = sink.refreshes() - before;
    eprintln!(
        "google: IMAP dial over a refused token: first chunk {page} message(s); {imap} replacement(s)"
    );
    assert_eq!(imap, 1, "the IMAP dial replaced the refused token once");
    assert_eq!(gmail_mail_route(&config.email), GmailMailRoute::Imap);

    // A refused token on an API read.
    let (token, expires_at) = garbage();
    again.seed_access_token(token, expires_at);
    let before = sink.refreshes();
    let labels = step(
        "labels over a refused token",
        provider.sync_mailboxes(&account, None).await,
    );
    let api = sink.refreshes() - before;
    eprintln!(
        "google: API read over a refused token: {} label(s); {api} replacement(s)",
        listed(&labels.update),
    );
    assert_eq!(api, 1, "the API read replaced the refused token once");

    // The calendar, connected as the app connects it, then over a refused token.
    let calendar = connected(
        "calendar connect",
        connect_google_calendar_provider(&account, Arc::clone(&again)).await,
    );
    let calendars = step("calendars", calendar.sync_calendars(&account, None).await);
    let (token, expires_at) = garbage();
    again.seed_access_token(token, expires_at);
    let before = sink.refreshes();
    let calendars_again = step(
        "calendars over a refused token",
        calendar.sync_calendars(&account, None).await,
    );
    let replaced = sink.refreshes() - before;
    eprintln!(
        "google: {} calendar(s), then {} over a refused token; {replaced} replacement(s); \
         {} refresh token rotation(s) in all, not written back",
        listed(&calendars.sync.update),
        listed(&calendars_again.sync.update),
        sink.rotations(),
    );
    assert_eq!(
        replaced, 1,
        "the calendar read replaced the refused token once"
    );
}
