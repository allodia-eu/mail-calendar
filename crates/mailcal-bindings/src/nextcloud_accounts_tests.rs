//! What a Nextcloud sign-in's completions refuse, and how an account a Nextcloud sign-in made
//! reads in Settings and leaves. A completion that connects is the live suite's
//! (`tests/live_nextcloud_sign_in.rs`).

use crate::{
    AccountCapability, MailcalError, SignInMethod,
    nextcloud::{
        nextcloud_tests::{AUTHORIZATION, STATUS, has_header},
        test_grant,
    },
    nextcloud_mock::{boot, mock_nextcloud, nextcloud_account},
};

#[test]
fn a_nextcloud_account_reads_by_its_profile_address_and_signs_in_through_nextcloud() {
    let server = mock_nextcloud(vec![]);
    let (app, _) = boot("nextcloud-entry", vec![nextcloud_account(&server.base)]);

    let entry = app.accounts_snapshot().accounts.remove(0);
    let host = server.base.trim_start_matches("http://");
    assert_eq!(entry.id, format!("f3a9c1@dav:{host}"));
    assert_eq!(entry.address, "alice@example.org");
    assert!(entry.nextcloud);
    assert_eq!(entry.sign_in, SignInMethod::Provider);
    assert!(
        entry.endpoints.is_none(),
        "its login is the server's to keep"
    );
    let options = app.account_sign_in_options(entry.id);
    assert!(options.nextcloud);
    assert!(!options.password && !options.sign_in);
}

#[test]
fn a_typed_password_at_a_nextcloud_server_can_switch_to_signing_in() {
    let nextcloud = mock_nextcloud(vec![("GET /status.php ", 200, STATUS)]);
    let elsewhere = mock_nextcloud(vec![]);
    let typed = |base: &str| {
        format!(
            "[caldav]\nbase_url = \"{base}/remote.php/dav/\"\nusername = \"alice\"\npassword = \"typed\"\n"
        )
    };
    let (app, _) = boot(
        "nextcloud-switch",
        vec![typed(&nextcloud.base), typed(&elsewhere.base)],
    );

    let entries = app.accounts_snapshot().accounts;
    let at = |base: &str| {
        let host = base.trim_start_matches("http://");
        entries
            .iter()
            .find(|entry| entry.id.ends_with(host))
            .unwrap()
            .clone()
    };
    let on_nextcloud = at(&nextcloud.base);
    assert!(!on_nextcloud.nextcloud);
    assert!(on_nextcloud.endpoints.is_some());
    assert!(app.account_sign_in_options(on_nextcloud.id).nextcloud);
    assert!(
        !app.account_sign_in_options(at(&elsewhere.base).id)
            .nextcloud
    );
}

#[test]
fn a_typed_password_at_a_server_that_shows_no_php_is_never_asked_a_php_path() {
    let stalwart = mock_nextcloud(vec![("GET /status.php ", 200, STATUS)]);
    let config = format!(
        "[caldav]\nbase_url = \"{}/dav/cal/\"\nusername = \"alice\"\npassword = \"typed\"\n",
        stalwart.base
    );
    let (app, _) = boot("nextcloud-no-php-account", vec![config]);

    let entry = app.accounts_snapshot().accounts.remove(0);
    assert!(!app.account_sign_in_options(entry.id).nextcloud);
    assert!(stalwart.requests_to("GET /status.php").is_empty());
}

#[test]
fn removing_a_nextcloud_account_withdraws_its_password() {
    let server = mock_nextcloud(vec![("DELETE /ocs/v2.php/core/apppassword ", 200, "")]);
    let (app, store) = boot("nextcloud-remove", vec![nextcloud_account(&server.base)]);
    let id = app.accounts_snapshot().accounts[0].id.clone();

    app.remove_account(id.clone()).unwrap();
    let request = server.wait_for("DELETE /ocs/v2.php/core/apppassword ");
    assert!(has_header(&request, AUTHORIZATION), "{request}");
    assert!(has_header(&request, "ocs-apirequest: true"), "{request}");
    assert!(store.deleted.lock().unwrap().contains(&id));
}

#[test]
fn removing_any_other_account_withdraws_nothing() {
    let server = mock_nextcloud(vec![]);
    let typed = format!(
        "[caldav]\nbase_url = \"{}/remote.php/dav/\"\nusername = \"alice\"\npassword = \"typed\"\n",
        server.base
    );
    let (app, _) = boot("nextcloud-remove-typed", vec![typed]);
    let id = app.accounts_snapshot().accounts[0].id.clone();

    app.remove_account(id).unwrap();
    std::thread::sleep(std::time::Duration::from_millis(300));
    assert!(server.requests_to("DELETE ").is_empty());
}

#[test]
fn a_second_sign_in_as_the_same_user_is_the_account_already_set_up() {
    let server = mock_nextcloud(vec![]);
    let (app, _) = boot("nextcloud-twice", vec![nextcloud_account(&server.base)]);
    let existing = app.accounts_snapshot().accounts[0].id.clone();

    // Signed in by address this time: another login name, the same person.
    let grant = test_grant(&server.base, "alice@example.org", "second", Some("alice"));
    let added = app.add_nextcloud_account(grant, vec![AccountCapability::Calendar]);
    assert!(
        matches!(&added, Err(MailcalError::AlreadySetUp { account_id }) if *account_id == existing),
        "{:?}",
        added.err()
    );
}

#[test]
fn signing_in_again_as_somebody_else_is_refused_and_withdrawn() {
    let server = mock_nextcloud(vec![("DELETE /ocs/v2.php/core/apppassword ", 200, "")]);
    let (app, _) = boot(
        "nextcloud-other-person",
        vec![nextcloud_account(&server.base)],
    );
    let id = app.accounts_snapshot().accounts[0].id.clone();

    let grant = test_grant(&server.base, "bob", "bob-password", Some("bob"));
    let again = app.complete_account_nextcloud_sign_in(id, grant);
    assert!(
        matches!(again, Err(MailcalError::DifferentAccount)),
        "{again:?}"
    );
    let request = server.wait_for("DELETE /ocs/v2.php/core/apppassword ");
    assert!(
        has_header(&request, "authorization: Basic Ym9iOmJvYi1wYXNzd29yZA=="),
        "{request}"
    );
}

#[test]
fn a_sign_in_at_another_server_is_not_this_account_s() {
    let server = mock_nextcloud(vec![]);
    let other = mock_nextcloud(vec![]);
    let (app, _) = boot(
        "nextcloud-other-server",
        vec![nextcloud_account(&server.base)],
    );
    let id = app.accounts_snapshot().accounts[0].id.clone();

    let grant = test_grant(&other.base, "f3a9c1", "pw", Some("alice"));
    assert!(matches!(
        app.complete_account_nextcloud_sign_in(id, grant),
        Err(MailcalError::Config(_))
    ));
}

#[test]
fn a_mailbox_s_separate_server_is_asked_where_its_calendar_is() {
    let server = mock_nextcloud(vec![("GET /status.php ", 200, STATUS)]);
    let mailbox = format!(
        "[imap]\naddr = \"127.0.0.1:1\"\nserver_name = \"127.0.0.1\"\nusername = \"alice@example.org\"\npassword = \"typed\"\n\
         \n[caldav]\nbase_url = \"{}/remote.php/dav/\"\nusername = \"alice@example.org\"\npassword = \"typed\"\n",
        server.base
    );
    let (app, _) = boot("nextcloud-separate", vec![mailbox]);
    let id = app.accounts_snapshot().accounts[0].id.clone();

    assert!(
        app.separate_server_offers_nextcloud_sign_in(id.clone(), vec![AccountCapability::Calendar])
    );
    assert!(
        !app.separate_server_offers_nextcloud_sign_in(id.clone(), vec![AccountCapability::Mail])
    );
    let other = mock_nextcloud(vec![]);
    let grant = test_grant(&other.base, "f3a9c1", "pw", Some("alice"));
    assert!(matches!(
        app.add_linked_nextcloud_account(id, vec![AccountCapability::Calendar], grant),
        Err(MailcalError::Config(_))
    ));
}
