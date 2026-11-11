use std::{
    fs,
    sync::Arc,
    time::{SystemTime, UNIX_EPOCH},
};

use mailcal_bindings::AccountCredentialStore;
use oo7::{Keyring, Secret, file};
use tokio::{runtime::Builder, sync::RwLock};

use super::{ACCOUNT_KIND, APPLICATION, SecretSink, SecretStore, block_on_runtime};

fn scratch_keyring() -> std::path::PathBuf {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock after epoch")
        .as_nanos();
    let directory = std::env::temp_dir().join(format!("mailcal-linux-secrets-{nonce}"));
    fs::create_dir_all(&directory).expect("create secure-store scratch directory");
    directory.join("accounts.keyring")
}

fn file_store(path: &std::path::Path, key: &[u8]) -> SecretStore {
    namespaced_file_store(path, key, None)
}

fn namespaced_file_store(
    path: &std::path::Path,
    key: &[u8],
    namespace: Option<&str>,
) -> SecretStore {
    let runtime = crate::host_runtime::shared().expect("a host runtime");
    let unlocked = runtime
        .block_on(file::UnlockedKeyring::load(
            path,
            Secret::from(key.to_vec()),
        ))
        .expect("open encrypted file backend");
    let keyring = Keyring::File(Arc::new(RwLock::new(Some(file::Keyring::Unlocked(
        unlocked,
    )))));
    SecretStore {
        runtime,
        keyring,
        namespace: namespace.map(str::to_owned),
    }
}

#[test]
fn account_items_are_scoped_by_application_kind_and_id() {
    let path = scratch_keyring();
    let store = file_store(&path, b"attribute-key");
    let attributes = store.account_attributes("alice@example.test@imap.example.test");

    assert_eq!(attributes["application"], APPLICATION);
    assert_eq!(attributes["kind"], ACCOUNT_KIND);
    assert_eq!(
        attributes["account"],
        "alice@example.test@imap.example.test"
    );
    assert!(!store.index_attributes().contains_key("account"));
}

/// The real store must keep writing and reading exactly the attributes it always has.
///
/// Every item already in a developer's or a user's keyring was written without a `namespace`,
/// so tagging the real store with one: even a default like `"default"`: would match none of
/// them on the next launch, and every account would appear to have vanished.
#[test]
fn the_real_store_tags_nothing_so_it_still_finds_what_is_already_stored() {
    let path = scratch_keyring();
    let store = file_store(&path, b"real-key");

    assert!(
        !store
            .account_attributes("someone")
            .contains_key("namespace")
    );
    assert!(!store.index_attributes().contains_key("namespace"));
}

/// A debug namespace and the real accounts cannot see each other, in either direction.
///
/// This is what lets a harness sign-in survive a relaunch: before it, those launches were
/// handed a store that refused every write, so signing in while testing never stuck.
///
/// Each step reopens the keyring rather than holding two stores at once; the file backend
/// refuses a write when the file changed under a handle it already had, which is an artefact
/// of testing without a Secret Service daemon rather than anything the app can hit.
#[test]
fn a_dev_namespace_and_the_real_accounts_never_see_each_other() {
    let path = scratch_keyring();
    let key = b"shared-keyring-key";
    let real = "[allodia]\nemail = \"real@example.test\"\nrefresh_token = \"real\"\n";
    let harness = "[allodia]\nemail = \"harness@example.test\"\nrefresh_token = \"dev\"\n";

    file_store(&path, key)
        .save("allodia-account", real)
        .expect("write the real entry");
    namespaced_file_store(&path, key, Some("dev"))
        .save("allodia-account", harness)
        .expect("write the dev entry");

    // One account id, two items, each store seeing only its own.
    assert_eq!(
        file_store(&path, key).configs().expect("real configs"),
        vec![real.to_owned()]
    );
    assert_eq!(
        namespaced_file_store(&path, key, Some("dev"))
            .configs()
            .expect("dev configs"),
        vec![harness.to_owned()]
    );

    // And erasing one leaves the other alone, which a shared index would not.
    namespaced_file_store(&path, key, Some("dev"))
        .remove("allodia-account")
        .expect("remove the dev entry");
    assert_eq!(
        namespaced_file_store(&path, key, Some("dev"))
            .configs()
            .expect("dev configs"),
        [] as [String; 0]
    );
    assert_eq!(
        file_store(&path, key).configs().expect("real configs"),
        vec![real.to_owned()],
        "signing out of the harness must not touch the developer's own accounts"
    );
}

/// Writes an account item exactly as given, the way a build before the encoding did.
fn store_raw(store: &SecretStore, id: &str, value: &[u8]) {
    block_on_runtime(
        store.runtime,
        store
            .keyring
            .create_item(id, &store.account_attributes(id), value, true),
    )
    .expect("runtime")
    .expect("a raw account item");
}

/// What reaches the keyring is one line, and an item stored before that still loads.
#[test]
fn a_multi_line_config_reaches_the_keyring_as_one_line() {
    let path = scratch_keyring();
    let config = "capabilities = [\"mail\"]\n\n[imap]\nhost = \"imap.example.test\"\n";
    let legacy = "[jmap]\nemail = \"old@example.test\"\n";
    let store = file_store(&path, b"one-line-key");
    store.save("new", config).expect("save");
    store
        .write_index(&["legacy".to_owned(), "new".to_owned()])
        .expect("index");
    store_raw(&store, "legacy", legacy.as_bytes());

    let raw = store
        .read_raw(&store.account_attributes("new"))
        .expect("read")
        .expect("the new item");
    assert!(!raw.contains(&b'\n'), "the stored config spans lines");

    assert_eq!(
        store.configs().expect("configs"),
        [legacy.to_owned(), config.to_owned()]
    );
    let _ = fs::remove_dir_all(path.parent().expect("keyring parent"));
}

/// One damaged item reaches the core as stored, for it to leave out alone; the rest still load.
#[test]
fn one_damaged_account_never_takes_the_others_with_it() {
    let path = scratch_keyring();
    let config = "capabilities = [\"mail\"]\n\n[imap]\nhost = \"imap.example.test\"\n";
    let store = file_store(&path, b"damaged-key");
    store.save("damaged", "placeholder\n").expect("save");
    store.save("intact", config).expect("save");
    store_raw(&store, "damaged", b"base64:not base64!");

    let stored = store.load().expect("one damaged item still loads the rest");

    assert_eq!(
        stored.configs,
        ["base64:not base64!".to_owned(), config.to_owned()]
    );
    assert!(
        stored.cut_short.is_empty(),
        "a damaged value is not one cut short"
    );
    let _ = fs::remove_dir_all(path.parent().expect("keyring parent"));
}

/// A config the plain-text keyring cut to its first line is reported once and then removed.
#[test]
fn an_account_cut_short_is_reported_then_removed_with_its_index_entry() {
    let path = scratch_keyring();
    let config = "capabilities = [\"mail\"]\n\n[imap]\nhost = \"imap.example.test\"\n";
    let store = file_store(&path, b"cut-short-key");
    store.save("cut", "placeholder\n").expect("save");
    store.save("whole", config).expect("save");
    store_raw(&store, "cut", b"capabilities = [\"mail\"]");

    let stored = store.load().expect("load");
    assert_eq!(
        stored.configs.len(),
        2,
        "the core still sees it, to report it"
    );
    assert_eq!(stored.cut_short, ["cut".to_owned()]);

    assert_eq!(
        store.forget_cut_short(&stored.cut_short).expect("forget"),
        1
    );
    let after = store.load().expect("load again");
    assert_eq!(after.configs, [config.to_owned()]);
    assert_eq!(after.cut_short, [] as [String; 0]);
    assert_eq!(store.read_index().expect("index"), ["whole".to_owned()]);
    let _ = fs::remove_dir_all(path.parent().expect("keyring parent"));
}

#[test]
fn credential_rotation_can_persist_from_inside_the_core_runtime() {
    let secret_runtime = Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("secret runtime");
    let core_runtime = Builder::new_multi_thread()
        .worker_threads(1)
        .enable_all()
        .build()
        .expect("core runtime");

    let persisted = core_runtime.block_on(async {
        block_on_runtime(&secret_runtime, async { "persisted" }).expect("nested block")
    });

    assert_eq!(persisted, "persisted");
}

#[test]
fn encrypted_store_replaces_then_deletes_the_account_credential_and_index() {
    let path = scratch_keyring();
    let key = [0x5a; 64];
    let store = Arc::new(file_store(&path, &key));
    let sink = SecretSink::new(Arc::clone(&store));

    sink.persist("first".to_owned(), "secret-v1".to_owned())
        .expect("save first credential");
    sink.persist("second".to_owned(), "secret-two".to_owned())
        .expect("save second credential");
    sink.persist("first".to_owned(), "secret-v2".to_owned())
        .expect("replace first credential");
    assert_eq!(
        store.configs().expect("read account index"),
        ["secret-v2", "secret-two"],
        "replacement keeps one ordered index entry and exposes only the new secret",
    );
    let encrypted = fs::read(&path).expect("read encrypted keyring file");
    assert!(
        !encrypted
            .windows("secret-v2".len())
            .any(|bytes| bytes == b"secret-v2")
    );

    sink.delete("first".to_owned())
        .expect("delete first credential");
    assert_eq!(
        store.configs().expect("read index after delete"),
        ["secret-two"]
    );
    drop(sink);
    drop(store);

    let reopened = file_store(&path, &key);
    assert_eq!(
        reopened.configs().expect("reopen persisted keyring"),
        ["secret-two"],
        "the removed credential does not return on a cold open",
    );
    let directory = path.parent().expect("keyring parent");
    let _ = fs::remove_dir_all(directory);
}
