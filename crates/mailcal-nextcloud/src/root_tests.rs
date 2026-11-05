use super::Root;

fn root(url: &str) -> Option<String> {
    Root::of(url).map(|root| root.as_str().to_owned())
}

#[test]
fn every_url_a_nextcloud_server_hands_out_names_its_installation() {
    for (url, expected) in [
        ("https://cloud.example.org", "https://cloud.example.org"),
        ("https://cloud.example.org/", "https://cloud.example.org"),
        (
            "https://cloud.example.org/remote.php/dav/",
            "https://cloud.example.org",
        ),
        (
            "https://cloud.example.org/remote.php/dav",
            "https://cloud.example.org",
        ),
        (
            "https://cloud.example.org/remote.php",
            "https://cloud.example.org",
        ),
        (
            "https://cloud.example.org/.well-known/caldav",
            "https://cloud.example.org",
        ),
        (
            "https://cloud.example.org/index.php/login/v2",
            "https://cloud.example.org",
        ),
        (
            "https://cloud.example.org/status.php",
            "https://cloud.example.org",
        ),
        (
            "https://example.org/nextcloud/remote.php/dav/principals/users/alice/",
            "https://example.org/nextcloud",
        ),
        (
            "https://example.org/nextcloud/",
            "https://example.org/nextcloud",
        ),
        (
            "https://cloud.example.org:8443/remote.php/dav?x=1#y",
            "https://cloud.example.org:8443",
        ),
        (
            "  https://Cloud.Example.org/  ",
            "https://cloud.example.org",
        ),
    ] {
        assert_eq!(root(url).as_deref(), Some(expected), "{url}");
    }
}

#[test]
fn only_https_or_this_machine_can_be_a_server() {
    for url in [
        "http://cloud.example.org/remote.php/dav/",
        "ftp://cloud.example.org/",
        "https://alice:pw@cloud.example.org/",
        "https://alice@cloud.example.org/",
        "cloud.example.org",
        "",
        "http://127.0.0.1.example.com/",
    ] {
        assert_eq!(root(url), None, "{url}");
    }
    assert_eq!(
        root("http://127.0.0.1:18082/remote.php/dav").as_deref(),
        Some("http://127.0.0.1:18082")
    );
    assert_eq!(
        root("http://localhost/").as_deref(),
        Some("http://localhost")
    );
}

#[test]
fn endpoints_hang_off_the_installation_directory() {
    let root = Root::of("https://example.org/nextcloud/remote.php/dav/").unwrap();
    assert_eq!(
        root.dav_url(),
        "https://example.org/nextcloud/remote.php/dav/"
    );
    assert_eq!(
        root.endpoint("/index.php/login/v2"),
        "https://example.org/nextcloud/index.php/login/v2"
    );
    assert_eq!(root.host(), "example.org");
}

#[test]
fn a_url_on_the_same_server_is_kept_and_upgraded_to_https() {
    let root = Root::of("https://cloud.example.org").unwrap();
    assert_eq!(
        root.owns("https://cloud.example.org/login/v2/poll")
            .as_deref(),
        Some("https://cloud.example.org/login/v2/poll")
    );
    assert_eq!(
        root.owns("http://cloud.example.org/login/v2/flow/abc")
            .as_deref(),
        Some("https://cloud.example.org/login/v2/flow/abc")
    );
    let ported = Root::of("https://cloud.example.org:8443").unwrap();
    assert_eq!(
        ported
            .owns("http://cloud.example.org:8443/login/v2/poll")
            .as_deref(),
        Some("https://cloud.example.org:8443/login/v2/poll")
    );
}

#[test]
fn a_url_on_another_server_is_refused() {
    let root = Root::of("https://cloud.example.org").unwrap();
    for url in [
        "https://evil.example/login/v2/poll",
        "https://cloud.example.org.evil.example/login/v2/poll",
        "https://cloud.example.org:8443/login/v2/poll",
        "http://cloud.example.org:8080/login/v2/poll",
        "https://alice:pw@cloud.example.org/login/v2/poll",
        "javascript:alert(1)",
        "not a url",
    ] {
        assert_eq!(root.owns(url), None, "{url}");
    }
    let loopback = Root::of("http://127.0.0.1:18082").unwrap();
    assert_eq!(
        loopback
            .owns("http://127.0.0.1:18082/login/v2/poll")
            .as_deref(),
        Some("http://127.0.0.1:18082/login/v2/poll")
    );
    assert_eq!(loopback.owns("https://127.0.0.1:18082/login/v2/poll"), None);
}

#[test]
fn a_root_round_trips_as_text() {
    let root = Root::of("https://example.org/nextcloud/remote.php/dav").unwrap();
    let json = serde_json::to_string(&root).unwrap();
    assert_eq!(json, "\"https://example.org/nextcloud\"");
    assert_eq!(serde_json::from_str::<Root>(&json).unwrap(), root);
    assert!(serde_json::from_str::<Root>("\"http://cloud.example.org\"").is_err());
}
