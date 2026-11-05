use super::{current_user, revoke};
use crate::{AppCredentials, NextcloudError, Root, Secret, test_server};

fn credentials() -> AppCredentials {
    AppCredentials {
        login_name: "alice@test.local".to_owned(),
        app_password: Secret::new("app-password".to_owned()),
    }
}

/// `Basic` for `alice@test.local:app-password`.
const AUTHORIZATION: &str = "authorization: Basic YWxpY2VAdGVzdC5sb2NhbDphcHAtcGFzc3dvcmQ=";

fn has_header(request: &str, header: &str) -> bool {
    request
        .lines()
        .any(|line| line.eq_ignore_ascii_case(header))
}

#[tokio::test]
async fn the_user_is_its_id_and_its_profile_address() {
    let server = test_server::serve(vec![test_server::json(
        200,
        r#"{"ocs":{"meta":{"status":"ok","statuscode":200},"data":{"id":"alice","email":"alice@test.local","display-name":"Alice"}}}"#,
    )]);
    let root = Root::of(&server.base).unwrap();
    let user = current_user(&test_server::client(), &root, &credentials())
        .await
        .unwrap();
    assert_eq!(user.id, "alice");
    assert_eq!(user.email.as_deref(), Some("alice@test.local"));
    let request = server.requests().remove(0);
    assert!(
        request.starts_with("GET /ocs/v2.php/cloud/user?format=json "),
        "{request}"
    );
    assert!(has_header(&request, AUTHORIZATION), "{request}");
    assert!(has_header(&request, "ocs-apirequest: true"), "{request}");
}

#[tokio::test]
async fn a_profile_without_an_address_has_none() {
    for data in [
        r#"{"id":"f3a9c1","email":null}"#,
        r#"{"id":"f3a9c1","email":"  "}"#,
        r#"{"id":"f3a9c1"}"#,
    ] {
        let server = test_server::serve(vec![test_server::json(
            200,
            &format!(r#"{{"ocs":{{"data":{data}}}}}"#),
        )]);
        let user = current_user(
            &test_server::client(),
            &Root::of(&server.base).unwrap(),
            &credentials(),
        )
        .await
        .unwrap();
        assert_eq!(user.email, None, "{data}");
    }
}

#[tokio::test]
async fn a_refused_or_unreadable_user_is_an_error() {
    for (reply, expected) in [
        (test_server::json(401, ""), "Status(401)"),
        (
            test_server::json(200, r#"{"ocs":{"data":{"id":""}}}"#),
            "Malformed",
        ),
        (test_server::json(200, "<html>"), "Malformed"),
    ] {
        let server = test_server::serve(vec![reply]);
        let err = current_user(
            &test_server::client(),
            &Root::of(&server.base).unwrap(),
            &credentials(),
        )
        .await
        .unwrap_err();
        assert_eq!(format!("{err:?}"), expected);
    }
}

#[tokio::test]
async fn revoking_deletes_the_password_it_authenticates_with() {
    let server = test_server::serve(vec![test_server::json(200, "<ocs/>")]);
    let root = Root::of(&server.base).unwrap();
    revoke(&test_server::client(), &root, &credentials())
        .await
        .unwrap();
    let request = server.requests().remove(0);
    assert!(
        request.starts_with("DELETE /ocs/v2.php/core/apppassword "),
        "{request}"
    );
    assert!(has_header(&request, AUTHORIZATION), "{request}");
    assert!(has_header(&request, "ocs-apirequest: true"), "{request}");
}

#[tokio::test]
async fn a_revocation_the_server_refuses_is_reported() {
    let server = test_server::serve(vec![test_server::json(401, "")]);
    let root = Root::of(&server.base).unwrap();
    assert!(matches!(
        revoke(&test_server::client(), &root, &credentials()).await,
        Err(NextcloudError::Status(401))
    ));
}
