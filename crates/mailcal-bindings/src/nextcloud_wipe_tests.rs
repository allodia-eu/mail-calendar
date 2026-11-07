//! A refused Nextcloud app password is asked about once, and a wipe takes the account away.

use std::time::{Duration, Instant};

use crate::nextcloud_mock::{boot, mock_nextcloud, nextcloud_account};

#[test]
fn a_refused_password_marked_for_wipe_takes_the_account_away() {
    let server = mock_nextcloud(vec![
        ("PROPFIND ", 401, ""),
        ("POST /index.php/core/wipe/check ", 200, r#"{"wipe":true}"#),
        ("POST /index.php/core/wipe/success ", 200, "[]"),
    ]);
    let (app, store) = boot("nextcloud-wipe", vec![nextcloud_account(&server.base)]);
    let id = app.accounts_snapshot().accounts[0].id.clone();

    let finished = server.wait_for("POST /index.php/core/wipe/success ");
    assert!(finished.ends_with("token=app-password"), "{finished}");
    let deadline = Instant::now() + Duration::from_secs(20);
    while !app.accounts_snapshot().accounts.is_empty() {
        assert!(
            Instant::now() < deadline,
            "the wiped account is still listed"
        );
        std::thread::sleep(Duration::from_millis(25));
    }
    assert!(store.deleted.lock().unwrap().contains(&id));
    assert!(
        server.requests_to("DELETE ").is_empty(),
        "a wiped password is not withdrawn on top"
    );
    assert_eq!(
        server.requests_to("POST /index.php/core/wipe/check ").len(),
        1
    );
}

#[test]
fn a_refusal_that_is_no_wipe_leaves_the_account_to_sign_in_again() {
    let server = mock_nextcloud(vec![
        ("PROPFIND ", 401, ""),
        ("POST /index.php/core/wipe/check ", 404, "[]"),
    ]);
    let (app, store) = boot("nextcloud-no-wipe", vec![nextcloud_account(&server.base)]);

    let asked = server.wait_for("POST /index.php/core/wipe/check ");
    assert!(asked.ends_with("token=app-password"), "{asked}");
    std::thread::sleep(Duration::from_millis(300));
    assert_eq!(app.accounts_snapshot().accounts.len(), 1);
    assert!(store.deleted.lock().unwrap().is_empty());
    assert!(
        server
            .requests_to("POST /index.php/core/wipe/success ")
            .is_empty()
    );
}

#[test]
fn a_refused_password_no_nextcloud_sign_in_issued_is_not_asked_about() {
    let server = mock_nextcloud(vec![("PROPFIND ", 401, "")]);
    let typed = format!(
        "[caldav]\nbase_url = \"{}/remote.php/dav/\"\nusername = \"alice\"\npassword = \"typed\"\n",
        server.base
    );
    let (app, _) = boot("nextcloud-typed-refused", vec![typed]);

    server.wait_for("PROPFIND ");
    std::thread::sleep(Duration::from_millis(500));
    assert!(server.requests_to("POST /index.php/core/wipe/").is_empty());
    assert_eq!(app.accounts_snapshot().accounts.len(), 1);
}
