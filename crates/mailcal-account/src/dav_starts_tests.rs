use super::{DavStarts, DavStartsFile};

fn dir(name: &str) -> std::path::PathBuf {
    let dir =
        std::env::temp_dir().join(format!("mailcal-dav-starts-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    dir
}

fn calendar(url: &str) -> DavStarts {
    DavStarts {
        calendar: Some(url.to_owned()),
        contacts: None,
    }
}

#[test]
fn a_missing_file_is_an_empty_cache() {
    assert!(DavStartsFile::in_dir(dir("missing")).load().is_empty());
}

#[test]
fn starts_are_kept_per_account_and_read_back() {
    let file = DavStartsFile::in_dir(dir("round-trip"));
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
    let file = DavStartsFile::in_dir(dir("forget"));
    file.store("a", &calendar("https://dav.example.net/a/principal/"))
        .unwrap();
    file.store("a", &DavStarts::default()).unwrap();
    assert!(file.load().is_empty());
}

#[test]
fn an_unreadable_file_is_an_empty_cache() {
    let base = dir("garbage");
    std::fs::create_dir_all(&base).unwrap();
    std::fs::write(base.join("dav-principals.toml"), "not = [toml").unwrap();
    assert!(DavStartsFile::in_dir(&base).load().is_empty());
}

#[test]
fn a_use_learned_nothing_about_keeps_what_it_had() {
    let stored = DavStarts {
        calendar: Some("https://dav.example.net/old/".to_owned()),
        contacts: Some("https://books.example.net/b/".to_owned()),
    };
    let learned = calendar("https://dav.example.net/new/");
    assert_eq!(
        stored.updated_with(&learned),
        DavStarts {
            calendar: Some("https://dav.example.net/new/".to_owned()),
            contacts: Some("https://books.example.net/b/".to_owned()),
        }
    );
}
