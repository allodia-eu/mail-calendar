//! The calendar's view of the account's token source: the same account, the same refresh, and
//! only the issuer's origins.

use std::sync::{Arc, atomic::Ordering};

use engine_provider::{
    Credential, CredentialProtocol, CredentialRequest, CredentialSource, Origin,
};

use super::IssuerBound;
use crate::graph::test_support::{ratcheting_token_endpoint, source_at};

fn bound(tokens: &Arc<crate::GraphTokenSource>, listed: &[&str]) -> IssuerBound {
    IssuerBound {
        tokens: Arc::clone(tokens),
        issuer: "https://auth.example.net".to_owned(),
        protected_resources: listed.iter().map(|&entry| entry.to_owned()).collect(),
    }
}

fn token(credential: &Credential) -> &str {
    match credential {
        Credential::Bearer { token, .. } => token.expose(),
        _ => panic!("expected a token credential"),
    }
}

/// A token the calendar saw refused is the one the mailbox stops presenting too: both ask the
/// account's one source, so one refresh serves both and no second refresh replays the first.
#[tokio::test]
async fn the_calendar_and_the_mailbox_share_the_accounts_source_and_its_refresh() {
    let (endpoint, hits) = ratcheting_token_endpoint("initial-refresh");
    let tokens = source_at(endpoint, None);
    let calendar = bound(&tokens, &[]);
    let dav = Origin::new("https", "dav.example.net", 443);
    let imap = Origin::new("imaps", "imap.example.net", 993);
    let over_http = CredentialRequest::new(CredentialProtocol::Http, &dav);
    let over_imap = CredentialRequest::new(CredentialProtocol::Imap, &imap);

    assert_eq!(calendar.subject(), tokens.subject());
    let refused = calendar.credential(&over_http).await.unwrap();
    assert_eq!(token(&refused), "AT-1");
    let replaced = calendar
        .replace(&refused, &over_http)
        .await
        .unwrap()
        .expect("a replacement");
    assert_eq!(token(&replaced), "AT-2");

    let mailbox = tokens.credential(&over_imap).await.unwrap();
    assert_eq!(
        token(&mailbox),
        "AT-2",
        "the mailbox presents the replacement"
    );
    assert_eq!(hits.load(Ordering::SeqCst), 2, "one sign-in, one refresh");
}

#[test]
fn a_listed_plaintext_loopback_server_is_permitted_on_its_own_port_only() {
    let (endpoint, _) = ratcheting_token_endpoint("initial-refresh");
    let tokens = source_at(endpoint, None);
    let calendar = bound(&tokens, &["http://127.0.0.1:28080", "https://[::1]/dav"]);

    assert!(calendar.permits(&Origin::new("http", "127.0.0.1", 28080)));
    assert!(!calendar.permits(&Origin::new("http", "127.0.0.1", 28081)));
    assert!(
        calendar.permits(&Origin::new("https", "::1", 443)),
        "an IPv6 host is matched with the brackets its URL carries"
    );
}

/// A calendar's config prints its source, so the source names the account by its log handle and
/// never by its id, which is an address, nor by its issuer's list.
#[test]
fn a_calendars_config_prints_no_address_and_no_issuer() {
    let account =
        engine_core::ids::AccountId::try_from("alice@example.net@imap.example.net").unwrap();
    let (endpoint, _) = ratcheting_token_endpoint("initial-refresh");
    let tokens = crate::graph::test_support::source_for(account.clone(), endpoint, None);
    let calendar = Arc::new(bound(&tokens, &["https://calendar.example.org"]));
    let config = provider_caldav::CalDavConfig::from_credential_source(
        "https://dav.example.com",
        calendar,
        crate::user_agent(),
    );

    let printed = format!("{config:?}");
    assert!(
        printed.contains(&crate::account_log_handle(account.as_str())),
        "{printed}"
    );
    for private in ["alice", "auth.example.net", "calendar.example.org"] {
        assert!(!printed.contains(private), "{private} in {printed}");
    }
}
