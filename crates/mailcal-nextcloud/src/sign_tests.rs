use url::Url;

use super::{answers_as_nextcloud, nextcloud_root, serves_php};
use crate::test_server;

const NEXTCLOUD: &str = r#"{"installed":true,"maintenance":false,"productname":"Nextcloud"}"#;

fn asked(requests: &[String]) -> Vec<String> {
    requests
        .iter()
        .map(|request| request.lines().next().unwrap_or_default().to_owned())
        .collect()
}

#[test]
fn only_an_endpoint_that_is_a_php_script_is_a_sign() {
    for url in [
        "https://cloud.example/remote.php/dav/",
        "https://example.org/nextcloud/remote.php/dav/calendars/alice/",
        "https://example.org/index.php/apps/calendar",
        "https://dav.example/dav.php/",
    ] {
        assert!(serves_php(&Url::parse(url).unwrap()), "{url}");
    }
    for url in [
        "https://cloud.example/",
        "https://mail.example/dav/cal/alice/",
        "https://example.org/.well-known/caldav",
        "https://example.org/php/dav/",
    ] {
        assert!(!serves_php(&Url::parse(url).unwrap()), "{url}");
    }
}

#[tokio::test]
async fn a_server_that_shows_no_php_is_never_asked_a_php_path() {
    // A Stalwart calendar: the address names its endpoint, which is no script, so nothing at all
    // is asked; a `.php` request there bans the address that sent it.
    let server = test_server::serve(vec![test_server::json(200, NEXTCLOUD)]);
    let url = format!("{}/dav/cal/alice/", server.base);
    assert!(!answers_as_nextcloud(&test_server::client(), &url).await);
    assert!(server.requests().is_empty(), "{:?}", server.requests());

    // A bare host whose `.well-known` lands on an endpoint that is no script: that one hop, and
    // no `status.php`.
    let server = test_server::serve_with(|base| {
        vec![
            test_server::response(
                "301 Moved Permanently",
                &[("Location", &format!("{base}/dav/cal"))],
                "",
            ),
            test_server::response("401 Unauthorized", &[], ""),
            test_server::json(200, NEXTCLOUD),
        ]
    });
    assert!(!answers_as_nextcloud(&test_server::client(), &server.base).await);
    let requests = asked(&server.requests());
    assert_eq!(
        requests,
        [
            "PROPFIND /.well-known/caldav HTTP/1.1",
            "PROPFIND /dav/cal HTTP/1.1"
        ],
        "never a .php path"
    );
}

#[tokio::test]
async fn an_endpoint_that_is_a_php_script_is_asked_at_its_root() {
    let server = test_server::serve(vec![test_server::json(200, NEXTCLOUD)]);
    let url = format!("{}/remote.php/dav/", server.base);
    assert!(answers_as_nextcloud(&test_server::client(), &url).await);
    assert_eq!(asked(&server.requests()), ["GET /status.php HTTP/1.1"]);
}

#[tokio::test]
async fn a_bare_host_is_asked_once_its_well_known_lands_on_nextcloud() {
    let server = test_server::serve_with(|base| {
        vec![
            test_server::response(
                "301 Moved Permanently",
                &[("Location", &format!("{base}/remote.php/dav/"))],
                "",
            ),
            test_server::response("401 Unauthorized", &[], ""),
            test_server::json(200, NEXTCLOUD),
        ]
    });
    assert!(answers_as_nextcloud(&test_server::client(), &server.base).await);
    assert_eq!(
        asked(&server.requests()),
        [
            "PROPFIND /.well-known/caldav HTTP/1.1",
            "PROPFIND /remote.php/dav/ HTTP/1.1",
            "GET /status.php HTTP/1.1",
        ]
    );
}

#[tokio::test]
async fn a_bare_host_names_the_root_its_well_known_lands_on() {
    let server = test_server::serve_with(|base| {
        vec![
            test_server::response(
                "301 Moved Permanently",
                &[("Location", &format!("{base}/nextcloud/remote.php/dav/"))],
                "",
            ),
            test_server::response("401 Unauthorized", &[], ""),
            test_server::json(200, NEXTCLOUD),
        ]
    });
    let root = nextcloud_root(&test_server::client(), &server.base).await;
    assert_eq!(
        root.map(|root| root.as_str().to_owned()),
        Some(format!("{}/nextcloud", server.base))
    );
    assert_eq!(
        asked(&server.requests()).last().map(String::as_str),
        Some("GET /nextcloud/status.php HTTP/1.1")
    );
}

#[tokio::test]
async fn a_well_known_that_answers_nothing_dav_like_is_asked_nothing_more() {
    let server = test_server::serve(vec![test_server::json(404, "")]);
    assert!(!answers_as_nextcloud(&test_server::client(), &server.base).await);
    assert_eq!(server.requests().len(), 1);
}
