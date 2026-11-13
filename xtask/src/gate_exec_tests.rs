use std::process::Command;

use super::{as_typed, local_sdk_dir, sdk_dir_property};
use crate::fixture::Repo;

#[test]
fn a_command_does_not_inherit_what_cargo_set_for_the_gate() {
    // `cargo test` sets these for this binary just as `cargo run` sets them for the gate.
    assert!(std::env::var_os("CARGO_MANIFEST_DIR").is_some());
    let mut command = Command::new("cargo");
    as_typed(&mut command);
    let removed: Vec<String> = command
        .get_envs()
        .filter(|(_, value)| value.is_none())
        .map(|(name, _)| name.to_string_lossy().into_owned())
        .collect();
    assert!(
        removed.iter().any(|name| name == "CARGO_MANIFEST_DIR"),
        "{removed:?}"
    );
    assert!(
        removed.iter().any(|name| name.starts_with("CARGO_PKG_")),
        "{removed:?}"
    );
}

#[test]
fn an_sdk_dir_counts_only_while_its_folder_exists() {
    let repo = Repo::new(&[]);
    let sdk = repo.root().join("android-sdk");
    // Escaped as Android Studio writes it, so a Windows path round-trips.
    let escaped = sdk
        .to_string_lossy()
        .replace('\\', "\\\\")
        .replace(':', "\\:");
    repo.write(
        "clients/android/local.properties",
        &format!("sdk.dir={escaped}\n"),
    );
    assert_eq!(local_sdk_dir(repo.root()), None);

    std::fs::create_dir_all(&sdk).expect("could not create the SDK folder");
    assert_eq!(local_sdk_dir(repo.root()), Some(sdk));
}

#[test]
fn an_sdk_dir_is_read_as_a_properties_file_reads_it() {
    let read = |text: &str| sdk_dir_property(text).map(|p| p.to_string_lossy().into_owned());
    assert_eq!(read("#sdk.dir=/opt/sdk\n! sdk.dir=/opt/sdk\n"), None);
    assert_eq!(read("sdk.directory=/opt/sdk\n"), None);
    assert_eq!(read("  sdk.dir = /opt/sdk\n"), Some("/opt/sdk".into()));
    assert_eq!(read("sdk.dir:/opt/sdk\n"), Some("/opt/sdk".into()));
    assert_eq!(
        read("sdk.dir=/old\nsdk.dir=/new\n"),
        Some("/new".into()),
        "the last assignment wins"
    );
    assert_eq!(
        read(r"sdk.dir=C\:\\Users\\me\\AppData\\Local\\Android\\Sdk"),
        Some(r"C:\Users\me\AppData\Local\Android\Sdk".into())
    );
}
