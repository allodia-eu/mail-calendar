use super::{DavLearned, DavStarts, DavStartsFile, Learned};

fn scratch() -> tempfile::TempDir {
    tempfile::tempdir().unwrap()
}

fn calendar(url: &str) -> DavStarts {
    DavStarts {
        calendar: Some(url.to_owned()),
        contacts: None,
    }
}

#[test]
fn a_missing_file_is_an_empty_cache() {
    let dir = scratch();
    assert!(DavStartsFile::in_dir(dir.path()).load().is_empty());
}

#[test]
fn starts_are_kept_per_account_and_read_back() {
    let dir = scratch();
    // A data directory that does not exist yet, so the first store has to make it.
    let file = DavStartsFile::in_dir(dir.path().join("data"));
    file.store("a", &calendar("https://dav.example.net/a/principal/"))
        .unwrap();
    file.store(
        "b",
        &DavStarts {
            calendar: None,
            contacts: Some("https://books.example.net/b/".to_owned()),
        },
    )
    .unwrap();

    let all = file.load();
    assert_eq!(all["a"], calendar("https://dav.example.net/a/principal/"));
    assert_eq!(
        all["b"].contacts.as_deref(),
        Some("https://books.example.net/b/")
    );
}

#[test]
fn storing_nothing_forgets_the_account() {
    let dir = scratch();
    let file = DavStartsFile::in_dir(dir.path());
    file.store("a", &calendar("https://dav.example.net/a/principal/"))
        .unwrap();
    file.store("a", &DavStarts::default()).unwrap();
    assert!(file.load().is_empty());
}

#[test]
fn an_unreadable_file_is_an_empty_cache() {
    let base = scratch();
    std::fs::write(base.path().join("dav-principals.toml"), "not = [toml").unwrap();
    assert!(DavStartsFile::in_dir(base.path()).load().is_empty());
}

#[test]
fn a_use_that_did_not_connect_keeps_what_it_had() {
    let stored = DavStarts {
        calendar: Some("https://dav.example.net/old/".to_owned()),
        contacts: Some("https://books.example.net/b/".to_owned()),
    };
    let learned = DavLearned {
        calendar: Learned::Principal("https://dav.example.net/new/".to_owned()),
        contacts: Learned::Nothing,
    };
    assert_eq!(
        stored.updated_with(&learned),
        DavStarts {
            calendar: Some("https://dav.example.net/new/".to_owned()),
            contacts: Some("https://books.example.net/b/".to_owned()),
        }
    );
}

/// A stored principal the server said was gone, with a fallback connect that named none, is
/// forgotten: otherwise every later connect spends a request on it first.
#[test]
fn a_use_that_connected_naming_no_principal_forgets_the_stored_one() {
    let stored = calendar("https://dav.example.net/gone/");
    let learned = DavLearned {
        calendar: Learned::connected(None),
        contacts: Learned::Nothing,
    };
    assert_eq!(stored.updated_with(&learned), DavStarts::default());
}
