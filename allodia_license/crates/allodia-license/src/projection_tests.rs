// SPDX-FileCopyrightText: 2026 Allodia
// SPDX-License-Identifier: LicenseRef-Allodia-1.0

//! The one failure that must never happen here is a secret travelling, and the one that would go
//! unnoticed is a config flattened into something that connects to the wrong name.
//!
//! Every case feeds the projection a config in the shape the credential store actually holds,
//! secrets included, and asserts on what comes out.

use super::*;

/// What the setup wizard writes for a password account, secrets and all.
const IMAP_STORED: &str = r#"
[imap]
addr = "imap.example.com:993"
server_name = "imap.example.com"
username = "someone@example.com"
password = "hunter2"

[smtp]
addr = "smtp.example.com:465"
server_name = "smtp.example.com"

[caldav]
base_url = "https://caldav.example.com"
username = "someone@example.com"
password = "hunter2"
"#;

/// Serialize whatever came out and prove the secret is not in it.
fn payload(config: &SyncedConfig) -> String {
    serde_json::to_string(config).unwrap()
}

#[test]
fn no_password_reaches_the_payload_from_any_of_the_places_one_is_stored() {
    // Three secrets in the stored config: IMAP's, CalDAV's, and the one SMTP borrows. The types
    // in this module have nowhere to put any of them, which is what makes this hold for fields
    // nobody has thought of yet rather than only for these three.
    let synced = to_synced(IMAP_STORED).unwrap();
    let json = payload(&synced);
    assert!(
        !json.contains("hunter2"),
        "a secret reached the payload: {json}"
    );
    assert!(!json.to_lowercase().contains("password"), "{json}");
}

#[test]
fn an_imap_account_keeps_every_field_the_other_device_needs() {
    let synced = to_synced(IMAP_STORED).unwrap();
    match synced {
        SyncedConfig::Imap {
            email,
            imap,
            smtp,
            caldav,
        } => {
            assert_eq!(email, "someone@example.com");
            assert_eq!(imap.host, "imap.example.com");
            assert_eq!(imap.port, 993);
            assert_eq!(imap.username, "someone@example.com");
            assert_eq!(imap.security, Security::ImplicitTls);
            let smtp = smtp.unwrap();
            assert_eq!(smtp.host, "smtp.example.com");
            assert_eq!(smtp.port, 465);
            let caldav = caldav.unwrap();
            assert_eq!(caldav.base_url, "https://caldav.example.com");
            assert_eq!(caldav.calendar, None);
        }
        other => panic!("expected IMAP, got {other:?}"),
    }
}

#[test]
fn a_starttls_account_does_not_arrive_as_an_implicit_tls_one() {
    // Silently flipping this downgrades or breaks the connection on the other device, and the
    // stored form defaults to implicit TLS, so a mapping that forgot the field would look right
    // for the common case and be wrong for exactly the accounts that set it.
    let stored = IMAP_STORED.replace(
        "username = \"someone@example.com\"\npassword",
        "username = \"someone@example.com\"\nsecurity = \"starttls\"\npassword",
    );
    let synced = to_synced(&stored).unwrap();
    let SyncedConfig::Imap { imap, .. } = &synced else {
        panic!("expected IMAP");
    };
    assert_eq!(imap.security, Security::Starttls);
    assert!(synced.to_prefill().starttls);
}

#[test]
fn an_account_whose_dial_host_and_tls_name_differ_is_refused_rather_than_flattened() {
    // The service's shape has one host. Sending either of the two would give the other device a
    // config that dials the wrong name or verifies against the wrong certificate, and it would
    // read as the account having simply stopped working.
    let stored = IMAP_STORED.replace(
        "addr = \"imap.example.com:993\"",
        "addr = \"mail.example.com:993\"",
    );
    assert!(matches!(
        to_synced(&stored),
        Err(NotSyncable::SplitHost {
            endpoint: "imap",
            ..
        })
    ));
}

#[test]
fn a_dial_address_without_a_port_is_refused() {
    let stored = IMAP_STORED.replace("imap.example.com:993", "imap.example.com");
    assert!(matches!(
        to_synced(&stored),
        Err(NotSyncable::Address {
            endpoint: "imap",
            ..
        })
    ));
}

#[test]
fn the_three_other_kinds_carry_only_what_the_service_stores_for_them() {
    let google =
        to_synced("[google]\nemail = \"someone@gmail.com\"\nrefresh_token = \"hunter2\"\n")
            .unwrap();
    assert!(matches!(&google, SyncedConfig::Google { email } if email == "someone@gmail.com"));
    assert!(!payload(&google).contains("hunter2"));

    let microsoft =
        to_synced("[microsoft]\nemail = \"someone@example.com\"\nrefresh_token = \"hunter2\"\n")
            .unwrap();
    assert!(matches!(&microsoft, SyncedConfig::Microsoft { .. }));
    assert!(!payload(&microsoft).contains("hunter2"));

    let jmap = to_synced(
        "[jmap]\nemail = \"someone@example.com\"\nbase_url = \"https://jmap.example.com\"\npassword = \"hunter2\"\n",
    )
    .unwrap();
    assert!(matches!(
        &jmap,
        SyncedConfig::Jmap {
            auth: JmapAuth::Secret,
            ..
        }
    ));
    assert!(!payload(&jmap).contains("hunter2"));
}

#[test]
fn a_jmap_account_connected_by_signing_in_says_so_without_carrying_the_grant() {
    let jmap = to_synced(
        "[jmap]\nemail = \"someone@example.com\"\nbase_url = \"https://jmap.example.com\"\n\
         [jmap.oauth]\nrefresh_token = \"hunter2\"\ntoken_endpoint = \"https://as.example.com/token\"\n",
    )
    .unwrap();
    assert!(matches!(
        &jmap,
        SyncedConfig::Jmap {
            auth: JmapAuth::OAuth,
            ..
        }
    ));
    let json = payload(&jmap);
    assert!(!json.contains("hunter2"), "the grant travelled: {json}");
    assert!(!json.contains("token_endpoint"), "{json}");
}

#[test]
fn the_allodia_grant_itself_is_not_a_mail_account() {
    // It sits in the same store under a reserved id, and reading it as an account to sync would
    // upload the app's own sign-in.
    assert_eq!(
        to_synced("[allodia]\nemail = \"someone@example.com\"\nrefresh_token = \"hunter2\"\n"),
        Err(NotSyncable::NotAnAccount)
    );
}

#[test]
fn an_offer_becomes_a_prefill_and_never_a_stored_config() {
    // The reverse direction has no password and cannot have one, which is why it produces
    // something the setup screen fills in rather than something the core could store.
    let prefill = to_synced(IMAP_STORED).unwrap().to_prefill();
    assert_eq!(prefill.kind, "imap");
    assert_eq!(prefill.email, "someone@example.com");
    assert_eq!(prefill.host.as_deref(), Some("imap.example.com"));
    assert_eq!(prefill.port, Some(993));
    assert_eq!(prefill.smtp, Some(("smtp.example.com".to_owned(), 465)));
    assert_eq!(
        prefill.caldav_base_url.as_deref(),
        Some("https://caldav.example.com")
    );
}

#[test]
fn each_endpoint_carries_its_own_security() {
    // A server wanting implicit TLS for reading and STARTTLS for submission is ordinary, so one
    // flag for both would send the other device to the right host on the wrong port, and it
    // would look like the account simply stopped sending.
    let mixed = "[imap]\naddr = \"mail.example.com:993\"\nserver_name = \"mail.example.com\"\n\
                 username = \"someone@example.com\"\npassword = \"hunter2\"\n\
                 [smtp]\naddr = \"mail.example.com:587\"\nserver_name = \"mail.example.com\"\n\
                 security = \"starttls\"\n";
    let prefill = to_synced(mixed).unwrap().to_prefill();
    assert!(!prefill.starttls, "reading is implicit TLS");
    assert!(prefill.smtp_starttls, "submission is upgraded in band");
}

#[test]
fn a_provider_offer_prefills_the_address_and_nothing_else_to_type() {
    // Everything but the address is derived, so the person taps sign-in rather than filling a form.
    let prefill = SyncedConfig::Google {
        email: "someone@gmail.com".to_owned(),
    }
    .to_prefill();
    assert_eq!(prefill.kind, "google");
    assert_eq!(prefill.email, "someone@gmail.com");
    assert!(prefill.host.is_none());
    assert!(prefill.smtp.is_none());
}
