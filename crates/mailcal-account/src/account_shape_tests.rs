//! The shared account keys: read from any kind's document, written only when set.

use super::*;

fn id(text: &str) -> AccountId {
    AccountId::try_from(text).unwrap()
}

#[test]
fn a_document_without_the_shared_keys_reads_as_the_empty_shape() {
    let shape = AccountShape::read(
        "[imap]\naddr = \"imap.example.org:993\"\nserver_name = \"imap.example.org\"\n",
    )
    .unwrap();
    assert_eq!(shape, AccountShape::default());
}

#[test]
fn the_empty_shape_writes_nothing() {
    let mut root = toml::Table::new();
    AccountShape::default().write_into(&mut root);
    assert!(root.is_empty(), "{root:?}");
}

#[test]
fn every_key_is_read_beside_a_kinds_own_section() {
    let shape = AccountShape::read(
        r#"
id = "alice@dav:cloud.example"
capabilities = ["contacts", "calendar"]

[links]
mail = "alice@imap.example.org"

[caldav]
base_url = "https://cloud.example/remote.php/dav"
"#,
    )
    .unwrap();
    assert_eq!(shape.id, Some(id("alice@dav:cloud.example")));
    assert_eq!(
        shape.capabilities,
        Some(
            [Capability::Calendar, Capability::Contacts]
                .into_iter()
                .collect()
        )
    );
    assert_eq!(shape.links.mail, Some(id("alice@imap.example.org")));
    assert_eq!(shape.links.calendar, None);
}

#[test]
fn a_written_shape_reads_back_unchanged() {
    let shape = AccountShape {
        id: Some(id("alice@imap.example.org")),
        capabilities: Some([Capability::Mail].into_iter().collect()),
        links: AccountLinks {
            calendar: Some(id("alice@dav:cloud.example")),
            contacts: Some(id("alice@dav:cloud.example")),
            mail: None,
        },
    };
    let mut root = toml::Table::new();
    shape.write_into(&mut root);
    let text = toml::to_string(&root).unwrap();
    assert_eq!(AccountShape::read(&text).unwrap(), shape, "{text}");
}

#[test]
fn capabilities_are_written_in_one_order_whatever_order_they_were_read_in() {
    // A document that is rewritten on every token rotation must not churn over a set.
    let shape =
        AccountShape::read("capabilities = [\"colleagues\", \"mail\", \"contacts\"]").unwrap();
    let mut root = toml::Table::new();
    shape.write_into(&mut root);
    assert_eq!(
        toml::to_string(&root).unwrap().trim(),
        r#"capabilities = ["mail", "contacts", "colleagues"]"#
    );
}

#[test]
fn an_empty_capability_list_reads_as_the_older_meaning() {
    // An account used for nothing is removed rather than emptied; a list with nothing in it
    // must not strand the account by switching everything off.
    let shape = AccountShape::read("capabilities = []").unwrap();
    assert_eq!(shape.capabilities, None);
}

#[test]
fn a_capability_this_build_does_not_know_is_kept_and_counts_for_nothing() {
    // A newer build may have written it. Refusing it would leave the account unreadable here;
    // dropping it would rewrite the account as used for less than it is on the next save.
    let shape = AccountShape::read("capabilities = [\"tasks\", \"mail\"]").unwrap();
    let capabilities = shape.capabilities.as_ref().unwrap();
    assert!(capabilities.contains(Capability::Mail));
    assert_eq!(capabilities.iter().collect::<Vec<_>>(), [Capability::Mail]);

    let mut root = toml::Table::new();
    shape.write_into(&mut root);
    assert_eq!(
        toml::to_string(&root).unwrap().trim(),
        r#"capabilities = ["mail", "tasks"]"#
    );
}
