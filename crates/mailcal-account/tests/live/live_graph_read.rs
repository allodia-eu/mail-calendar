//! Gated live check, **read-only**, that a Microsoft account is wired as the app wires it: its
//! folders bound on one client, a page of its inbox, and a token the server refuses replaced
//! once, for a mail read and a calendar read alike.
//!
//! It reads and nothing else: no send, no draft, no flag, no move, no folder change. It prints
//! classes and counts, never a token, an address, a subject or any content.
//!
//! It **skips** unless `MS_REFRESH_TOKEN`, `MS_CLIENT_ID` and `MS_TEST_ADDRESS` are set, as
//! `live_graph_contacts` does, whose header says how to set them; run it on its own:
//! `cargo test -p mailcal-account --test live -- live_graph_read:: --nocapture`.

use std::sync::Arc;

use engine_core::time::TimeZoneId;
use engine_provider::Provider;
use mailcal_account::{
    CredentialOrigin, GraphTokenSource, TokenSink, connect_graph_calendar_provider,
    connect_graph_folder, connect_graph_mail_providers,
};

use super::read_only::{CountingSink, a_page, connected, garbage, listed, step};

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn live_a_microsoft_account_reads_and_replaces_a_refused_token_once() {
    let Some(config) = super::live_graph_contacts::config() else {
        eprintln!(
            "skipping live_graph_read: MS_REFRESH_TOKEN / MS_CLIENT_ID / MS_TEST_ADDRESS unset"
        );
        return;
    };
    let account = config.account_id().expect("account id");
    let sink = Arc::new(CountingSink::default());
    let tokens = connected(
        "token source",
        GraphTokenSource::new(
            &config,
            account.clone(),
            Some(Arc::clone(&sink) as Arc<dyn TokenSink>),
            CredentialOrigin::Stored,
        ),
    );

    // The account as the app connects it: a provider per folder, on one client.
    let folders = connected(
        "connect",
        connect_graph_mail_providers(&account, Arc::clone(&tokens), None, None).await,
    );
    let inbox = connected(
        "inbox",
        connect_graph_folder(Arc::clone(&tokens), "inbox", None, None),
    );
    let page = step(
        "a page of the inbox",
        a_page(inbox.as_ref(), &account).await,
    );
    eprintln!(
        "graph: {} folder(s) bound; first chunk of the inbox {page} message(s); {} refresh(es)",
        folders.len(),
        sink.refreshes(),
    );
    assert!(!folders.is_empty(), "the account binds its folders");

    // A refused token on a mail read.
    let (token, expires_at) = garbage();
    tokens.seed_access_token(token, expires_at);
    let before = sink.refreshes();
    let listing = step(
        "folders over a refused token",
        inbox.sync_mailboxes(&account, None).await,
    );
    let mail = sink.refreshes() - before;
    eprintln!(
        "graph: folder list over a refused token: {} folder(s); {mail} replacement(s)",
        listed(&listing.update),
    );
    assert_eq!(mail, 1, "the mail read replaced the refused token once");

    // The calendar, connected as the app connects it, then over a refused token.
    let calendar = connected(
        "calendar connect",
        connect_graph_calendar_provider(&account, Arc::clone(&tokens), TimeZoneId::utc(), None)
            .await,
    );
    let calendars = step("calendars", calendar.sync_calendars(&account, None).await);
    let (token, expires_at) = garbage();
    tokens.seed_access_token(token, expires_at);
    let before = sink.refreshes();
    let again = step(
        "calendars over a refused token",
        calendar.sync_calendars(&account, None).await,
    );
    let replaced = sink.refreshes() - before;
    eprintln!(
        "graph: {} calendar(s), then {} over a refused token; {replaced} replacement(s); \
         {} refresh token rotation(s) in all, not written back",
        listed(&calendars.sync.update),
        listed(&again.sync.update),
        sink.rotations(),
    );
    assert_eq!(
        replaced, 1,
        "the calendar read replaced the refused token once"
    );
}
