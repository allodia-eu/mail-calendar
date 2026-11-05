use engine_api::ConferenceService;

use super::{TalkAtConnect, talk_at_connect};
use crate::{
    AccountSetup, Capability, ConnectionSecurity, NextcloudLogin, SetupCredential,
    build_config_toml, graph::token_source::test_support::mock_token_endpoint, load_str,
};

/// A calendar account a Nextcloud sign-in at `server` made, or a plain DAV one without `login`.
fn account(server: &str, nextcloud: bool) -> crate::AccountConfig {
    let login = NextcloudLogin {
        server: server.to_owned(),
        user: Some("alice".to_owned()),
        address: None,
    };
    let credential = if nextcloud {
        SetupCredential::Nextcloud {
            app_password: "app-password".to_owned(),
            login,
        }
    } else {
        SetupCredential::Password("password".to_owned())
    };
    load_str(
        &build_config_toml(&AccountSetup {
            imap_host: String::new(),
            username: "alice".to_owned(),
            credential,
            smtp_host: None,
            caldav_base_url: Some(format!("{server}/remote.php/dav/")),
            carddav_base_url: None,
            imap_security: ConnectionSecurity::ImplicitTls,
            smtp_security: ConnectionSecurity::ImplicitTls,
            accepted_certificate: None,
            uses: Some([Capability::Calendar].into_iter().collect()),
        })
        .unwrap(),
    )
    .unwrap()
}

/// A server answering every request with `body`, and how many it was sent.
fn server(body: &str) -> (String, std::sync::Arc<std::sync::atomic::AtomicUsize>) {
    let (token_url, hits) = mock_token_endpoint(vec![body.to_owned(); 4]);
    (token_url.trim_end_matches("/token").to_owned(), hits)
}

const WITH_TALK: &str = r#"{"ocs":{"data":{"capabilities":{"spreed":{"config":{"conversations":{"can-create":true}}}}}}}"#;

#[tokio::test]
async fn a_nextcloud_server_with_talk_makes_meetings_through_the_account() {
    let (url, _) = server(WITH_TALK);
    let TalkAtConnect::Offered(talk) = talk_at_connect(&account(&url, true)).await else {
        panic!("a server with Talk offers it");
    };
    assert_eq!(talk.service(), ConferenceService::NextcloudTalk);
}

#[tokio::test]
async fn a_server_without_talk_or_an_account_it_lets_start_none_is_not_offered() {
    let (url, _) = server(r#"{"ocs":{"data":{"capabilities":{"core":{}}}}}"#);
    assert!(matches!(
        talk_at_connect(&account(&url, true)).await,
        TalkAtConnect::NotOffered
    ));
    let (url, _) = server(
        r#"{"ocs":{"data":{"capabilities":{"spreed":{"config":{"conversations":{"can-create":false}}}}}}}"#,
    );
    assert!(matches!(
        talk_at_connect(&account(&url, true)).await,
        TalkAtConnect::NotOffered
    ));
}

#[tokio::test]
async fn an_account_no_nextcloud_sign_in_made_sends_nothing() {
    let (url, hits) = server(WITH_TALK);
    assert!(matches!(
        talk_at_connect(&account(&url, false)).await,
        TalkAtConnect::NotOffered
    ));
    assert_eq!(hits.load(std::sync::atomic::Ordering::SeqCst), 0);
}

#[tokio::test]
async fn a_server_that_does_not_answer_leaves_talk_unknown() {
    let closed = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}", closed.local_addr().unwrap());
    drop(closed);
    assert!(matches!(
        talk_at_connect(&account(&url, true)).await,
        TalkAtConnect::Unknown
    ));
}
