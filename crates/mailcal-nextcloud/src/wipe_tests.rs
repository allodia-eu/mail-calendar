use super::{wipe_finished, wipe_requested};
use crate::{Root, Secret, test_server};

fn password() -> Secret {
    Secret::new("app-password".to_owned())
}

#[tokio::test]
async fn a_password_marked_for_wipe_is_answered_with_a_wipe() {
    let server = test_server::serve(vec![test_server::json(200, r#"{"wipe":true}"#)]);
    let root = Root::of(&server.base).unwrap();
    assert!(
        wipe_requested(&test_server::client(), &root, &password())
            .await
            .unwrap()
    );
    let request = server.requests().remove(0);
    assert!(
        request.starts_with("POST /index.php/core/wipe/check "),
        "{request}"
    );
    assert!(request.ends_with("token=app-password"), "{request}");
}

#[tokio::test]
async fn any_other_password_is_not_a_wipe() {
    let server = test_server::serve(vec![test_server::json(404, "[]")]);
    let root = Root::of(&server.base).unwrap();
    assert!(
        !wipe_requested(&test_server::client(), &root, &password())
            .await
            .unwrap()
    );
}

#[tokio::test]
async fn a_server_that_cannot_say_is_an_error_not_a_wipe() {
    for reply in [test_server::json(500, ""), test_server::json(200, "<html>")] {
        let server = test_server::serve(vec![reply]);
        let root = Root::of(&server.base).unwrap();
        assert!(
            wipe_requested(&test_server::client(), &root, &password())
                .await
                .is_err()
        );
    }
}

#[tokio::test]
async fn a_finished_wipe_is_reported_with_the_same_password() {
    let server = test_server::serve(vec![test_server::json(200, "[]")]);
    let root = Root::of(&server.base).unwrap();
    wipe_finished(&test_server::client(), &root, &password())
        .await
        .unwrap();
    let request = server.requests().remove(0);
    assert!(
        request.starts_with("POST /index.php/core/wipe/success "),
        "{request}"
    );
    assert!(request.ends_with("token=app-password"), "{request}");
}
