use super::{is_nextcloud, is_nextcloud_status};
use crate::{Root, test_server};

/// `status.php` as `nextcloud.allodia.eu` answered it, Nextcloud 35.
const NEXTCLOUD: &str = r#"{"installed":true,"maintenance":false,"needsDbUpgrade":false,"version":"35.0.2.2","versionstring":"35.0.2","edition":"","productname":"Nextcloud","extendedSupport":false}"#;

#[test]
fn nextcloud_answers_as_nextcloud() {
    assert!(is_nextcloud_status(NEXTCLOUD.as_bytes()));
}

#[test]
fn every_other_answer_is_not_nextcloud() {
    for body in [
        // ownCloud 10 reports its flags as strings, and its own name.
        r#"{"installed":"true","maintenance":"false","needsDbUpgrade":"false","version":"10.15.0.2","versionstring":"10.15.0","edition":"Community","productname":"ownCloud","product":"ownCloud"}"#,
        r#"{"installed":true,"maintenance":false,"needsDbUpgrade":false,"version":"10.11.0.0","versionstring":"10.11.0","edition":"Community","productname":"Infinite Scale","product":"Infinite Scale"}"#,
        r#"{"installed":true,"maintenance":true,"productname":"Nextcloud"}"#,
        r#"{"installed":false,"maintenance":false,"productname":"Nextcloud"}"#,
        r#"{"installed":true,"maintenance":false}"#,
        "<html><body><h1>401 Unauthorized</h1></body></html>",
        "[]",
        "",
    ] {
        assert!(!is_nextcloud_status(body.as_bytes()), "{body}");
    }
}

#[test]
fn a_document_says_nextcloud_maintenance_another_product_or_nothing() {
    use super::{Status, status_of_body};
    assert_eq!(status_of_body(NEXTCLOUD.as_bytes()), Status::Nextcloud);
    for body in [
        r#"{"installed":"true","maintenance":"false","productname":"ownCloud"}"#,
        r#"{"installed":true,"maintenance":false,"productname":"Infinite Scale"}"#,
        r#"{"installed":false,"maintenance":false,"productname":"Nextcloud"}"#,
    ] {
        assert_eq!(status_of_body(body.as_bytes()), Status::Other, "{body}");
    }
    assert_eq!(
        status_of_body(br#"{"installed":true,"maintenance":true,"productname":"Nextcloud"}"#),
        Status::Maintenance
    );
    for body in [
        r#"{"installed":true,"maintenance":false}"#,
        "<html><body><h1>404 Not Found</h1></body></html>",
        "[]",
        "",
    ] {
        assert_eq!(status_of_body(body.as_bytes()), Status::Unknown, "{body}");
    }
}

#[tokio::test]
async fn a_server_is_asked_at_its_root() {
    let server = test_server::serve(vec![test_server::json(200, NEXTCLOUD)]);
    let root = Root::of(&format!("{}/remote.php/dav/", server.base)).unwrap();
    assert!(is_nextcloud(&test_server::client(), &root).await);
    let request = server.requests().remove(0);
    assert!(request.starts_with("GET /status.php "), "{request}");
}

#[tokio::test]
async fn a_refusal_or_a_redirect_is_not_nextcloud() {
    let client = test_server::client();
    for reply in [
        test_server::response(
            "401 Unauthorized",
            &[("WWW-Authenticate", "Basic realm=\"sabre/dav\"")],
            "",
        ),
        test_server::response(
            "301 Moved Permanently",
            &[("Location", "https://elsewhere.example/status.php")],
            "",
        ),
    ] {
        let server = test_server::serve(vec![reply]);
        assert!(!is_nextcloud(&client, &Root::of(&server.base).unwrap()).await);
    }
}
