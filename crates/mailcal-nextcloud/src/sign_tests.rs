use url::Url;

use super::{
    Named, answers_as_nextcloud, named_nextcloud_root, nextcloud_root, on_the_same_server,
    serves_php, well_known_starts,
};
use crate::{Root, test_server};

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

fn moved(to: &str) -> String {
    test_server::response("301 Moved Permanently", &[("Location", to)], "")
}

fn challenge() -> String {
    test_server::response("401 Unauthorized", &[], "")
}

async fn named(address: &str) -> Named {
    named_nextcloud_root(&test_server::client(), address).await
}

fn root(url: &str) -> Named {
    Named::Nextcloud(Root::of(url).unwrap())
}

#[tokio::test]
async fn a_server_named_as_nextcloud_is_found_through_its_contacts_well_known() {
    // Only the address book's `.well-known` is redirected.
    let server = test_server::serve_with(|base| {
        vec![
            test_server::json(404, ""),
            moved(&format!("{base}/remote.php/dav/")),
            challenge(),
            test_server::json(200, NEXTCLOUD),
        ]
    });
    assert_eq!(named(&server.base).await, root(&server.base));
    assert_eq!(
        asked(&server.requests()),
        [
            "PROPFIND /.well-known/caldav HTTP/1.1",
            "PROPFIND /.well-known/carddav HTTP/1.1",
            "PROPFIND /remote.php/dav/ HTTP/1.1",
            "GET /status.php HTTP/1.1",
        ]
    );
}

#[tokio::test]
async fn a_redirect_to_a_php_path_is_the_sign_even_where_that_path_refuses() {
    // Calendars and contacts switched off: the redirect stays, its target answers 404.
    let server = test_server::serve_with(|base| {
        vec![
            moved(&format!("{base}/remote.php/dav/")),
            test_server::json(404, ""),
            test_server::json(200, NEXTCLOUD),
        ]
    });
    assert_eq!(named(&server.base).await, root(&server.base));
    assert_eq!(
        asked(&server.requests()).last().map(String::as_str),
        Some("GET /status.php HTTP/1.1")
    );
}

#[tokio::test]
async fn an_installation_in_a_directory_is_asked_at_its_own_well_known_first() {
    let server = test_server::serve_with(|base| {
        vec![
            moved(&format!("{base}/nextcloud/remote.php/dav/")),
            challenge(),
            test_server::json(200, NEXTCLOUD),
        ]
    });
    let typed = format!("{}/nextcloud/", server.base);
    assert_eq!(
        named(&typed).await,
        root(&format!("{}/nextcloud", server.base))
    );
    assert_eq!(
        asked(&server.requests()),
        [
            "PROPFIND /nextcloud/.well-known/caldav HTTP/1.1",
            "PROPFIND /nextcloud/remote.php/dav/ HTTP/1.1",
            "GET /nextcloud/status.php HTTP/1.1",
        ]
    );
}

#[tokio::test]
async fn an_address_named_as_nextcloud_that_shows_no_php_is_never_asked_a_php_path() {
    // Nothing redirected, in the directory or at the host.
    let server = test_server::serve(vec![test_server::json(404, ""); 4]);
    let typed = format!("{}/nextcloud", server.base);
    assert_eq!(named(&typed).await, Named::Unsure);
    assert_eq!(
        asked(&server.requests()),
        [
            "PROPFIND /nextcloud/.well-known/caldav HTTP/1.1",
            "PROPFIND /nextcloud/.well-known/carddav HTTP/1.1",
            "PROPFIND /.well-known/caldav HTTP/1.1",
            "PROPFIND /.well-known/carddav HTTP/1.1",
        ]
    );

    // Stalwart's shape: both land on DAV endpoints that are no script, so this is another DAV
    // server, and no sign-in is offered that would send it a `.php` request.
    let server = test_server::serve_with(|base| {
        vec![
            moved(&format!("{base}/dav/cal")),
            challenge(),
            moved(&format!("{base}/dav/card")),
            challenge(),
            test_server::json(200, NEXTCLOUD),
        ]
    });
    assert_eq!(named(&server.base).await, Named::Other);
    assert!(
        asked(&server.requests())
            .iter()
            .all(|request| !request.contains(".php")),
        "{:?}",
        server.requests()
    );
}

#[tokio::test]
async fn a_server_that_names_another_product_is_other_asked_once_per_root() {
    let ocis = r#"{"installed":true,"maintenance":false,"productname":"Infinite Scale"}"#;
    let server = test_server::serve_with(|base| {
        vec![
            moved(&format!("{base}/remote.php/dav/")),
            challenge(),
            test_server::json(200, ocis),
            moved(&format!("{base}/remote.php/dav/")),
            challenge(),
        ]
    });
    assert_eq!(named(&server.base).await, Named::Other);
    let requests = asked(&server.requests());
    assert_eq!(
        requests
            .iter()
            .filter(|request| request.contains("status.php"))
            .count(),
        1,
        "{requests:?}"
    );

    // An address that is a PHP endpoint is asked there, once.
    let server = test_server::serve(vec![test_server::json(200, ocis)]);
    assert_eq!(
        named(&format!("{}/remote.php/dav/", server.base)).await,
        Named::Other
    );
    assert_eq!(asked(&server.requests()), ["GET /status.php HTTP/1.1"]);

    // A status document that cannot be read is no answer either way.
    let server = test_server::serve(vec![test_server::json(500, "")]);
    assert_eq!(
        named(&format!("{}/remote.php/dav/", server.base)).await,
        Named::Unsure
    );
}

#[tokio::test]
async fn a_server_that_does_not_answer_is_unsure() {
    let server = test_server::serve(Vec::new());
    assert_eq!(named(&server.base).await, Named::Unsure);
    assert_eq!(named("not an address").await, Named::Unsure);
}

#[test]
fn a_typed_well_known_is_asked_at_the_directory_it_is_in() {
    let starts = |typed: &str| -> Vec<String> {
        well_known_starts(&Url::parse(typed).unwrap())
            .iter()
            .map(|start| start.path().to_owned())
            .collect()
    };
    assert_eq!(
        starts("https://example.org/nextcloud/.well-known/caldav?x=1"),
        [
            "/nextcloud/.well-known/caldav",
            "/nextcloud/.well-known/carddav",
            "/.well-known/caldav",
            "/.well-known/carddav",
        ]
    );
    assert_eq!(
        starts("https://example.org/.well-known/carddav"),
        ["/.well-known/caldav", "/.well-known/carddav"]
    );
}

#[tokio::test]
async fn a_nextcloud_server_in_maintenance_says_so() {
    let maintenance = r#"{"installed":true,"maintenance":true,"productname":"Nextcloud"}"#;
    let server = test_server::serve_with(|base| {
        vec![
            moved(&format!("{base}/remote.php/dav/")),
            test_server::json(503, ""),
            test_server::json(200, maintenance),
        ]
    });
    assert_eq!(named(&server.base).await, Named::Maintenance);
    assert_eq!(server.requests().len(), 3, "nothing more is asked");
}

#[test]
fn an_http_redirect_on_an_https_server_is_answered_as_https() {
    let from = Url::parse("https://cloud.example.org/.well-known/caldav").unwrap();
    let upgrade = |to: &str| on_the_same_server(&from, Url::parse(to).unwrap()).to_string();
    assert_eq!(
        upgrade("http://cloud.example.org/remote.php/dav/"),
        "https://cloud.example.org/remote.php/dav/"
    );
    for kept in [
        "http://elsewhere.example.org/remote.php/dav/",
        "http://cloud.example.org:8080/remote.php/dav/",
        "https://cloud.example.org/remote.php/dav/",
    ] {
        assert_eq!(upgrade(kept), kept);
    }
    let plain = Url::parse("http://127.0.0.1:8080/.well-known/caldav").unwrap();
    let next = Url::parse("http://127.0.0.1:8080/remote.php/dav/").unwrap();
    assert_eq!(on_the_same_server(&plain, next.clone()), next);
}
