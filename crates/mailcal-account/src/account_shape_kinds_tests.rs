//! The shared keys through each kind's own loader and serializer.
//!
//! Two properties per kind. A document written before these keys existed comes back byte for
//! byte, because every stored account is rewritten on the next token rotation or setting change
//! and nothing may creep into it. And a document that carries them keeps them across that
//! rewrite, with a pinned id winning over the derived one.

use crate::{Capability, load_google_str, load_jmap_str, load_microsoft_str, load_str};

const IMAP: &str = r#"
[imap]
addr = "imap.example.org:993"
server_name = "imap.example.org"
username = "alice@example.org"
password = "pw"

[smtp]
addr = "smtp.example.org:465"
server_name = "smtp.example.org"
"#;

const MICROSOFT: &str = r#"
[microsoft]
email = "alice@example.com"
client_id = "client-abc"
tenant = "common"
redirect_uri = "eu.allodia.mailcal://oauth"
scopes = ["offline_access", "https://graph.microsoft.com/Mail.ReadWrite"]
refresh_token = "rt"
"#;

const GOOGLE: &str = r#"
[google]
email = "alice@gmail.com"
client_id = "client-abc"
redirect_uri = "eu.allodia.mailcal:/oauth"
scopes = ["https://mail.google.com/"]
refresh_token = "rt"
"#;

const JMAP: &str = r#"
[jmap]
email = "alice@fastmail.example"
base_url = "https://api.fastmail.example"
password = "pw"
"#;

const SHAPE: &str = r#"
id = "pinned@imap.example.org"
capabilities = ["mail", "calendar"]

[links]
contacts = "alice@dav:cloud.example"
"#;

/// Rewrites a stored document the way the app does: load it, serialize it again.
fn rewrite(text: &str) -> String {
    if text.contains("[microsoft]") {
        load_microsoft_str(text).unwrap().to_toml().unwrap()
    } else if text.contains("[google]") {
        load_google_str(text).unwrap().to_toml().unwrap()
    } else if text.contains("[jmap]") {
        load_jmap_str(text).unwrap().to_toml().unwrap()
    } else {
        load_str(text).unwrap().to_toml().unwrap()
    }
}

#[test]
fn a_document_from_before_the_shared_keys_is_rewritten_byte_for_byte() {
    for kind in [IMAP, MICROSOFT, GOOGLE, JMAP] {
        // What the previous build stored is what this serializer writes for the same account.
        let stored = rewrite(kind);
        assert_eq!(rewrite(&stored), stored);
        for key in ["id =", "capabilities =", "[links]", "granted_scopes ="] {
            assert!(
                !stored.lines().any(|line| line.starts_with(key)),
                "{key} crept into:\n{stored}"
            );
        }
    }
}

#[test]
fn every_kind_keeps_its_shared_keys_across_a_rewrite() {
    for kind in [IMAP, MICROSOFT, GOOGLE, JMAP] {
        let stored = rewrite(&format!("{SHAPE}{kind}"));
        let reread = rewrite(&stored);
        assert_eq!(reread, stored);
        for key in [
            "id = \"pinned@imap.example.org\"",
            "capabilities = [\"mail\", \"calendar\"]",
            "contacts = \"alice@dav:cloud.example\"",
        ] {
            assert!(stored.contains(key), "{key} lost from:\n{stored}");
        }
    }
}

#[test]
fn a_pinned_id_wins_over_the_derived_one_for_every_kind() {
    let with_shape = |kind: &str| format!("{SHAPE}{kind}");
    let ids = [
        load_str(&with_shape(IMAP)).unwrap().account_id(),
        load_microsoft_str(&with_shape(MICROSOFT))
            .unwrap()
            .account_id(),
        load_google_str(&with_shape(GOOGLE)).unwrap().account_id(),
        load_jmap_str(&with_shape(JMAP)).unwrap().account_id(),
    ];
    for id in ids {
        assert_eq!(id.unwrap().as_str(), "pinned@imap.example.org");
    }
    // The derived id is still there for what needs to know which mailbox a sign-in named.
    let imap = load_str(&with_shape(IMAP)).unwrap();
    assert_eq!(
        imap.derived_account_id().unwrap().as_str(),
        "alice@example.org@imap.example.org"
    );
}

#[test]
fn every_kind_reads_what_the_account_is_used_for() {
    let config = load_jmap_str(&format!("{SHAPE}{JMAP}")).unwrap();
    let capabilities = config.shape.capabilities.unwrap();
    assert!(capabilities.contains(Capability::Calendar));
    assert!(!capabilities.contains(Capability::Contacts));
}

#[test]
fn the_granted_scopes_are_kept_beside_the_requested_ones() {
    for kind in [MICROSOFT, GOOGLE] {
        let section_end = kind.trim_end().len();
        let text = format!(
            "{}\ngranted_scopes = [\"offline_access\"]\n",
            &kind[..section_end]
        );
        let stored = rewrite(&text);
        assert!(
            stored.contains("granted_scopes = [\"offline_access\"]"),
            "{stored}"
        );
        assert_eq!(rewrite(&stored), stored);
    }
}

#[test]
fn an_account_that_stores_no_choice_is_used_for_what_its_kind_always_meant() {
    use Capability::{Calendar, Colleagues, Contacts, Mail};
    let listed = |capabilities: crate::Capabilities| capabilities.iter().collect::<Vec<_>>();

    // A standards account without a calendar endpoint is a mailbox and nothing else.
    assert_eq!(listed(load_str(IMAP).unwrap().capabilities()), [Mail]);
    // With one, the calendar and the address book found beside it come along.
    let with_caldav =
        format!("{IMAP}\n[caldav]\nbase_url = \"https://dav.example.org\"\nusername = \"alice\"\n");
    assert_eq!(
        listed(load_str(&with_caldav).unwrap().capabilities()),
        [Mail, Calendar, Contacts]
    );
    // JMAP offers what its session advertises; the session decides at the dial.
    assert_eq!(
        listed(load_jmap_str(JMAP).unwrap().capabilities()),
        [Mail, Calendar, Contacts]
    );
    // Microsoft and Google asked for everything at sign-in, colleagues included.
    for all in [
        load_microsoft_str(MICROSOFT).unwrap().capabilities(),
        load_google_str(GOOGLE).unwrap().capabilities(),
    ] {
        assert_eq!(listed(all), [Mail, Calendar, Contacts, Colleagues]);
    }
}

#[test]
fn a_stored_choice_is_what_the_account_is_used_for_whatever_its_kind() {
    let chosen = "capabilities = [\"calendar\"]\n";
    for capabilities in [
        load_str(&format!("{chosen}{IMAP}")).unwrap().capabilities(),
        load_microsoft_str(&format!("{chosen}{MICROSOFT}"))
            .unwrap()
            .capabilities(),
        load_google_str(&format!("{chosen}{GOOGLE}"))
            .unwrap()
            .capabilities(),
        load_jmap_str(&format!("{chosen}{JMAP}"))
            .unwrap()
            .capabilities(),
    ] {
        assert_eq!(
            capabilities.iter().collect::<Vec<_>>(),
            [Capability::Calendar]
        );
    }
}
