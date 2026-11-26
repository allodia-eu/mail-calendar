use engine_api::{ConferenceService, MeetingRequest, MeetingService};
use engine_core::time::UtcDateTime;
use engine_provider::{CredentialSubject, MeetingKey};
use mailcal_oauth::DavUse;
use provider_google::GoogleClient;
use provider_graph::{GraphClient, MailboxPrincipal};

use super::{AccountMeet, AccountTeams, TalkAtConnect, talk_at_connect, talk_source};
use crate::{
    AccountSetup, Capability, ConnectionSecurity, NextcloudLogin, SetupCredential,
    build_config_toml,
    graph::token_source::test_support::{mock_token_endpoint, source_at},
    http_fake, load_str,
    tls::tls_with,
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
    // Port 1, which nothing listens on and no other test's server can be given.
    let url = "http://127.0.0.1:1";
    assert!(matches!(
        talk_at_connect(&account(url, true)).await,
        TalkAtConnect::Unknown
    ));
}

/// A Nextcloud account whose calendar, or without one its address book, signs in as `login` with
/// `password`, beside a mailbox signing in as `you@example.net` with `pw`.
fn nextcloud(calendar: bool, login: &str, password: &str) -> crate::AccountConfig {
    let dav = if calendar { "caldav" } else { "carddav" };
    load_str(&format!(
        r#"
[imap]
addr = "imap.example.net:993"
server_name = "imap.example.net"
username = "you@example.net"
password = "pw"

[{dav}]
base_url = "https://cloud.example.org/remote.php/dav/"
username = "{login}"
password = "{password}"

[nextcloud]
server = "https://cloud.example.org"
"#
    ))
    .unwrap()
}

/// Talk signs in with the app password of the DAV endpoint the account's Nextcloud sign-in
/// stored, so it is that login's subject, decided by the one DAV rule.
#[test]
fn talk_signs_in_as_the_subject_of_the_dav_login_whose_password_it_presents() {
    let subject = |config: &crate::AccountConfig| {
        talk_source(config)
            .expect("a Nextcloud sign-in made the account")
            .expect("the app password makes a source")
            .1
            .subject()
            .clone()
    };
    let account_id = |config: &crate::AccountConfig| {
        CredentialSubject::new(config.account_id().unwrap().as_str())
    };
    let dav_rule = |config: &crate::AccountConfig, calendar: bool| {
        let (endpoint, dav_use) = if calendar {
            (config.caldav_endpoint().unwrap(), DavUse::Calendars)
        } else {
            (config.carddav_endpoint().unwrap(), DavUse::Contacts)
        };
        crate::config_dav::dav_credential_source(config, endpoint, dav_use, None)
            .unwrap()
            .subject()
            .clone()
    };

    let mailbox_login = nextcloud(true, "you@example.net", "pw");
    assert_eq!(subject(&mailbox_login), account_id(&mailbox_login));

    let own_login = nextcloud(true, "alice", "app-password");
    assert_ne!(subject(&own_login), account_id(&own_login));
    assert_eq!(subject(&own_login), dav_rule(&own_login, true));

    let contacts_only = nextcloud(false, "alice", "app-password");
    assert_eq!(subject(&contacts_only), dav_rule(&contacts_only, false));
    assert_ne!(subject(&contacts_only), dav_rule(&own_login, true));
}

/// A meeting a test makes, with a key of the length a service expects.
fn request() -> MeetingRequest {
    MeetingRequest {
        key: MeetingKey::new("abcdefghijklmnopqrstuvwxyz").unwrap(),
        title: "Planning".to_owned(),
        start: UtcDateTime::new(2026, 12, 1, 9, 0, 0).unwrap(),
        end: UtcDateTime::new(2026, 12, 1, 10, 0, 0).unwrap(),
    }
}

/// Teams and Meet ask the account's source for the token on each request, and one the service
/// refused is replaced once and the request sent again.
#[tokio::test]
async fn teams_and_meet_present_the_current_token_and_replace_a_refused_one() {
    let teams = r#"{"joinWebUrl":"https://teams.example.com/l/meetup-join/1"}"#;
    let meet = r#"{"name":"spaces/abc","meetingUri":"https://meet.example.com/abc-defg-hij"}"#;
    for (service, answer) in [("teams", teams), ("meet", meet)] {
        let (endpoint, minted) =
            mock_token_endpoint(vec![http_fake::minted(1), http_fake::minted(2)]);
        let tokens = source_at(endpoint, None);
        let (base, received) = http_fake::serve(http_fake::refused_then(answer));
        let tls = tls_with(&[]).unwrap();
        let meetings: Box<dyn MeetingService> = if service == "teams" {
            Box::new(AccountTeams {
                connect: Box::new(crate::graph::connector(
                    tokens,
                    MailboxPrincipal::Me,
                    tls,
                    move |source, tls, retry, agent| {
                        GraphClient::with_base_and_source(source, &base, tls, retry, agent)
                    },
                )),
            })
        } else {
            Box::new(AccountMeet {
                connect: Box::new(crate::google::connector(
                    tokens,
                    tls,
                    move |source, tls, retry, agent| {
                        GoogleClient::with_base_and_source(source, &base, tls, retry, agent)
                    },
                )),
            })
        };

        meetings.create_meeting(&request()).await.unwrap();
        meetings.create_meeting(&request()).await.unwrap();

        http_fake::assert_replaced_once(&received, &minted);
    }
}
