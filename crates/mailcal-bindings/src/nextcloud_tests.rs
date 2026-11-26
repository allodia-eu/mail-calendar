//! The browser half of a Nextcloud sign-in, against a stand-in server: what is asked, what the app
//! password is named, and what a grant carries. A sign-in that connects an account is the live
//! suite's (`tests/live/live_nextcloud_sign_in.rs`).

use super::{NextcloudGrant, client_name, password_name, test_grant};
use crate::{
    NextcloudAddressStart, NextcloudSignInPoll, Platform,
    nextcloud_mock::{boot, mock_nextcloud},
};

pub(crate) const STATUS: &str =
    r#"{"installed":true,"maintenance":false,"version":"35.0.2.2","productname":"Nextcloud"}"#;
pub(crate) const STARTED: &str = r#"{"poll":{"token":"poll-token","endpoint":"{base}\/login\/v2\/poll"},"login":"{base}\/login\/v2\/flow\/login-token"}"#;
const GRANTED: &str = r#"{"server":"{base}","loginName":"f3a9c1","appPassword":"app-password"}"#;
const USER: &str = r#"{"ocs":{"data":{"id":"alice","email":"alice@example.org"}}}"#;

/// `Basic` for `f3a9c1:app-password`.
pub(crate) const AUTHORIZATION: &str = "authorization: Basic ZjNhOWMxOmFwcC1wYXNzd29yZA==";

pub(crate) fn has_header(request: &str, header: &str) -> bool {
    request
        .lines()
        .any(|line| line.eq_ignore_ascii_case(header))
}

#[test]
fn the_app_password_is_named_after_the_app_and_the_platform() {
    let name = client_name(Some(Platform::Ipados));
    assert_eq!(name, password_name(env!("MAILCAL_APP_NAME"), "iPadOS"));
    assert!(client_name(Some(Platform::Windows)).ends_with(" (Windows)"));
    assert!(client_name(None).ends_with(')'));
}

#[test]
fn the_name_leaves_out_an_ampersand_the_grant_page_would_escape_twice() {
    assert_eq!(
        password_name("Allodia Mail & Calendar", "iOS"),
        "Allodia Mail Calendar (iOS)"
    );
    assert_eq!(password_name("Mail&Cal", "Linux"), "Mail Cal (Linux)");
}

#[test]
fn a_server_that_says_it_is_nextcloud_offers_the_sign_in() {
    let server = mock_nextcloud(vec![("GET /status.php ", 200, STATUS)]);
    let silent = mock_nextcloud(vec![]);
    let (app, _) = boot("nextcloud-offers", Vec::new());

    assert!(app.offers_nextcloud_sign_in(format!("{}/remote.php/dav/", server.base)));
    assert!(!app.offers_nextcloud_sign_in(silent.base.clone()));
    assert!(!app.offers_nextcloud_sign_in("http://cloud.example.org/".to_owned()));
}

#[test]
fn a_server_that_shows_no_php_is_never_asked_a_php_path() {
    // Stalwart's shape: its `.well-known` answers as DAV where it is, and a request for a `.php`
    // path bans the address that sent it.
    let stalwart = mock_nextcloud(vec![
        ("GET /status.php ", 200, STATUS),
        ("PROPFIND /.well-known/caldav ", 401, ""),
    ]);
    let (app, _) = boot("nextcloud-no-php", Vec::new());

    assert!(!app.offers_nextcloud_sign_in(format!("{}/dav/cal/", stalwart.base)));
    assert!(!app.offers_nextcloud_sign_in(stalwart.base.clone()));
    assert_eq!(
        stalwart.requests_to("PROPFIND /.well-known/caldav").len(),
        1
    );
    assert!(stalwart.requests_to("GET /status.php").is_empty());
}

#[test]
fn a_redirect_that_names_no_endpoint_is_no_sign() {
    let server = mock_nextcloud(vec![
        ("PROPFIND /.well-known/caldav ", 301, ""),
        ("GET /status.php ", 200, STATUS),
    ]);
    let (app, _) = boot("nextcloud-bare-host", Vec::new());
    // A redirect without a Location sends the client nowhere, so the server shows no sign.
    assert!(!app.offers_nextcloud_sign_in(server.base.clone()));
    assert!(server.requests_to("GET /status.php").is_empty());
}

#[test]
fn a_sign_in_waits_for_the_grant_and_learns_who_signed_in() {
    let server = mock_nextcloud(vec![
        ("POST /index.php/login/v2 ", 200, STARTED),
        ("POST /login/v2/poll ", 404, "[]"),
        ("POST /login/v2/poll ", 200, GRANTED),
        ("GET /ocs/v2.php/cloud/user", 200, USER),
    ]);
    let (app, _) = boot("nextcloud-flow", Vec::new());

    let start = app
        .begin_nextcloud_sign_in(format!("{}/remote.php/dav/", server.base))
        .unwrap();
    assert_eq!(
        start.login_url,
        format!("{}/login/v2/flow/login-token", server.base)
    );
    let begun = server.wait_for("POST /index.php/login/v2 ");
    assert!(
        has_header(
            &begun,
            &format!(
                "user-agent: {}",
                password_name(env!("MAILCAL_APP_NAME"), "macOS")
            )
        ),
        "{begun}"
    );
    assert_eq!(
        app.poll_nextcloud_sign_in(start.pending.clone()).unwrap(),
        NextcloudSignInPoll::Waiting
    );
    let NextcloudSignInPoll::Granted { grant } = app.poll_nextcloud_sign_in(start.pending).unwrap()
    else {
        panic!("the sign-in was not granted");
    };
    let grant = NextcloudGrant::parse(&grant).unwrap();
    assert_eq!(grant.root.as_str(), server.base);
    assert_eq!(grant.credentials.login_name, "f3a9c1");
    assert_eq!(grant.credentials.app_password.expose(), "app-password");
    assert_eq!(grant.user.as_deref(), Some("alice"));
    assert_eq!(grant.email.as_deref(), Some("alice@example.org"));
    assert!(has_header(
        &server.wait_for("GET /ocs/v2.php/cloud/user"),
        AUTHORIZATION
    ));
}

#[test]
fn a_grant_whose_user_cannot_be_learned_still_signs_in() {
    let server = mock_nextcloud(vec![
        ("POST /index.php/login/v2 ", 200, STARTED),
        ("POST /login/v2/poll ", 200, GRANTED),
    ]);
    let (app, _) = boot("nextcloud-flow-anonymous", Vec::new());

    let start = app.begin_nextcloud_sign_in(server.base.clone()).unwrap();
    let NextcloudSignInPoll::Granted { grant } = app.poll_nextcloud_sign_in(start.pending).unwrap()
    else {
        panic!("the sign-in was not granted");
    };
    let grant = NextcloudGrant::parse(&grant).unwrap();
    assert_eq!(grant.user, None);
    assert_eq!(grant.email, None);
}

#[test]
fn a_server_that_starts_no_sign_in_is_a_connect_error() {
    let server = mock_nextcloud(vec![]);
    let (app, _) = boot("nextcloud-no-flow", Vec::new());

    assert!(matches!(
        app.begin_nextcloud_sign_in(server.base.clone()),
        Err(crate::MailcalError::Connect(_))
    ));
    assert!(matches!(
        app.begin_nextcloud_sign_in("http://cloud.example.org".to_owned()),
        Err(crate::MailcalError::Config(_))
    ));
    assert!(matches!(
        app.poll_nextcloud_sign_in("not a sign-in".to_owned()),
        Err(crate::MailcalError::Config(_))
    ));
}

#[test]
fn an_abandoned_grant_is_withdrawn() {
    let server = mock_nextcloud(vec![("DELETE /ocs/v2.php/core/apppassword ", 200, "")]);
    let (app, _) = boot("nextcloud-abandon", Vec::new());

    app.abandon_nextcloud_sign_in(test_grant(
        &server.base,
        "f3a9c1",
        "app-password",
        Some("alice"),
    ));
    let request = server.wait_for("DELETE /ocs/v2.php/core/apppassword ");
    assert!(has_header(&request, AUTHORIZATION), "{request}");
    assert!(has_header(&request, "ocs-apirequest: true"), "{request}");
}

#[test]
fn a_grant_round_trips_with_every_field() {
    let grant = test_grant("https://cloud.example.org", "f3a9c1", "pw", Some("alice"));
    let parsed = NextcloudGrant::parse(&grant).unwrap();
    assert_eq!(serde_json::to_string(&parsed).unwrap(), grant);
    let login = parsed.login();
    assert_eq!(login.server, "https://cloud.example.org");
    assert_eq!(login.user.as_deref(), Some("alice"));
    assert_eq!(login.address.as_deref(), Some("alice@example.org"));
}

#[test]
fn a_server_named_as_nextcloud_is_asked_and_signed_in_at_from_its_address() {
    // Only its address book's `.well-known` leads to Nextcloud.
    let server = mock_nextcloud(vec![
        (
            "PROPFIND /.well-known/carddav ",
            301,
            "{base}/remote.php/dav/",
        ),
        ("PROPFIND /remote.php/dav/ ", 401, ""),
        ("GET /status.php ", 200, STATUS),
        ("POST /index.php/login/v2 ", 200, STARTED),
    ]);
    let (app, _) = boot("nextcloud-named", Vec::new());

    let started = app.begin_nextcloud_sign_in_at(format!(" {} ", server.base));
    let Ok(NextcloudAddressStart::Started { start }) = started else {
        panic!("no sign-in started: {started:?}");
    };
    assert_eq!(
        start.login_url,
        format!("{}/login/v2/flow/login-token", server.base)
    );
    assert_eq!(
        server.requests_to("PROPFIND /.well-known/caldav").len(),
        1,
        "the calendar's hop is asked first"
    );
    assert_eq!(server.requests_to("GET /status.php").len(), 1);
}

#[test]
fn an_address_that_shows_no_php_is_never_asked_a_php_path() {
    let stalwart = mock_nextcloud(vec![("GET /status.php ", 200, STATUS)]);
    let (app, _) = boot("nextcloud-named-no-php", Vec::new());

    assert!(matches!(
        app.begin_nextcloud_sign_in_at(stalwart.base.clone()),
        Ok(NextcloudAddressStart::Unsure { .. })
    ));
    assert!(stalwart.requests_to("GET /status.php").is_empty());
}

#[test]
fn an_address_that_shows_nothing_is_unsure_and_can_be_tried_anyway() {
    let unsure = mock_nextcloud(vec![("POST /index.php/login/v2 ", 200, STARTED)]);
    let (app, _) = boot("nextcloud-named-unsure", Vec::new());

    let host = unsure.base.trim_start_matches("http://");
    let host = host.split(':').next().unwrap().to_owned();
    assert!(matches!(
        app.begin_nextcloud_sign_in_at(unsure.base.clone()),
        Ok(NextcloudAddressStart::Unsure { host: asked }) if asked == host
    ));
    assert!(
        unsure.requests_to("POST /index.php/login/v2").is_empty(),
        "no sign-in starts before the person chooses to try"
    );
    let started = app.begin_nextcloud_sign_in_anyway(unsure.base.clone());
    let Ok(NextcloudAddressStart::Started { start }) = started else {
        panic!("no sign-in started: {started:?}");
    };
    assert_eq!(
        start.login_url,
        format!("{}/login/v2/flow/login-token", unsure.base)
    );

    // A page's URL copied from the browser is tried at its host once the page starts nothing.
    let page = format!("{}/apps/dashboard/", unsure.base);
    assert!(matches!(
        app.begin_nextcloud_sign_in_anyway(page),
        Ok(NextcloudAddressStart::Started { .. })
    ));
    let begun = unsure.requests_to("POST ");
    assert!(
        begun
            .iter()
            .any(|request| request.starts_with("POST /apps/dashboard/index.php/login/v2 ")),
        "{begun:?}"
    );

    // Tried anyway, a server that starts no sign-in is not Nextcloud.
    let silent = mock_nextcloud(vec![]);
    assert!(matches!(
        app.begin_nextcloud_sign_in_anyway(silent.base.clone()),
        Ok(NextcloudAddressStart::NotNextcloud { .. })
    ));
}

#[test]
fn an_address_whose_server_names_another_product_is_not_nextcloud() {
    const OWNCLOUD: &str = r#"{"installed":"true","maintenance":"false","productname":"ownCloud"}"#;
    let owncloud = mock_nextcloud(vec![("GET /status.php ", 200, OWNCLOUD)]);
    let (app, _) = boot("nextcloud-named-other", Vec::new());

    assert!(matches!(
        app.begin_nextcloud_sign_in_at(format!("{}/remote.php/dav/", owncloud.base)),
        Ok(NextcloudAddressStart::NotNextcloud { .. })
    ));
    assert!(owncloud.requests_to("POST /index.php/login/v2").is_empty());
}

#[test]
fn what_no_server_can_be_at_starts_nothing() {
    let (app, _) = boot("nextcloud-named-no-address", Vec::new());
    for typed in [
        "",
        "  ",
        "http://cloud.example.org",
        "https://alice:pw@cloud.example.org",
    ] {
        assert!(
            matches!(
                app.begin_nextcloud_sign_in_at(typed.to_owned()),
                Ok(NextcloudAddressStart::NotAnAddress)
            ),
            "{typed:?}"
        );
        assert!(
            matches!(
                app.begin_nextcloud_sign_in_anyway(typed.to_owned()),
                Ok(NextcloudAddressStart::NotAnAddress)
            ),
            "{typed:?}"
        );
    }
}

#[test]
fn a_nextcloud_server_in_maintenance_says_so() {
    const MAINTENANCE: &str = r#"{"installed":true,"maintenance":true,"productname":"Nextcloud"}"#;
    let server = mock_nextcloud(vec![("GET /status.php ", 200, MAINTENANCE)]);
    let (app, _) = boot("nextcloud-named-maintenance", Vec::new());

    assert!(matches!(
        app.begin_nextcloud_sign_in_at(format!("{}/remote.php/dav/", server.base)),
        Ok(NextcloudAddressStart::Maintenance { .. })
    ));
    assert!(server.requests_to("POST /index.php/login/v2").is_empty());
}
