//! What an account's servers section says beneath its heading.

use mailcal_bindings::{AccountServers, InsecureCause};

use crate::l10n;

/// What the section says beneath its heading: that the server is reached only over plaintext
/// when it is, and what clears it, else the expired sign-in when there is one, otherwise how saving
/// works; then what an empty address book server means when it may be left empty. An entry row has
/// no subtitle, and a widget added to the group lands under the whole list, so that is said here
/// rather than under its field.
pub(super) fn description(address: &str, servers: &AccountServers) -> String {
    let mut said = if let Some(cause) = servers.insecure_server {
        match cause {
            InsecureCause::PlaintextAddress => l10n::settings_account_servers_plaintext_address(),
            InsecureCause::LedToPlaintext => l10n::settings_account_servers_led_to_plaintext(),
        }
        .to_owned()
    } else if servers.sign_in_refused {
        l10n::signin_expired_prompt(address)
    } else {
        l10n::settings_account_servers_description().to_owned()
    };
    if servers.contacts_fall_back_to_calendar {
        said.push(' ');
        said.push_str(l10n::settings_account_carddav_empty_hint());
    }
    said
}

#[cfg(test)]
mod tests {
    use mailcal_bindings::{
        AccountEndpoints, AccountServers, ConnectionSecurity, ControlState, InsecureCause,
    };

    use super::description;
    use crate::l10n;

    fn servers(
        refused: bool,
        insecure: Option<InsecureCause>,
        contacts_fall_back: bool,
    ) -> AccountServers {
        AccountServers {
            form: AccountEndpoints {
                imap_host: Some("imap.example.org".to_owned()),
                imap_security: ConnectionSecurity::ImplicitTls,
                smtp_host: None,
                smtp_security: ConnectionSecurity::ImplicitTls,
                caldav_url: None,
                carddav_url: None,
                username: "alice".to_owned(),
                password: None,
            },
            save: ControlState {
                enabled: true,
                running: false,
            },
            sign_in_refused: refused,
            insecure_server: insecure,
            contacts_fall_back_to_calendar: contacts_fall_back,
        }
    }

    #[test]
    fn the_section_says_when_the_sign_in_has_expired() {
        assert_eq!(
            description("alice@example.org", &servers(true, None, false)),
            l10n::signin_expired_prompt("alice@example.org")
        );
        assert_eq!(
            description("alice@example.org", &servers(false, None, false)),
            l10n::settings_account_servers_description()
        );
    }

    /// A new password cannot clear a server reached only over plaintext, so the section says
    /// what does instead: its https address typed here, or the server's operator.
    #[test]
    fn the_section_says_when_the_server_is_not_reached_securely_and_what_clears_it() {
        let plaintext = Some(InsecureCause::PlaintextAddress);
        let led = Some(InsecureCause::LedToPlaintext);
        assert_eq!(
            description("alice@example.org", &servers(false, plaintext, false)),
            l10n::settings_account_servers_plaintext_address()
        );
        assert_eq!(
            description("alice@example.org", &servers(true, plaintext, false)),
            l10n::settings_account_servers_plaintext_address()
        );
        assert_eq!(
            description("alice@example.org", &servers(false, led, false)),
            l10n::settings_account_servers_led_to_plaintext()
        );
    }

    #[test]
    fn the_section_says_an_empty_address_book_server_leaves_the_contacts_on_the_calendar() {
        assert_eq!(
            description("alice@example.org", &servers(false, None, true)),
            format!(
                "{} {}",
                l10n::settings_account_servers_description(),
                l10n::settings_account_carddav_empty_hint()
            )
        );
        assert!(
            description("alice@example.org", &servers(true, None, true))
                .ends_with(l10n::settings_account_carddav_empty_hint()),
            "beside an expired sign-in too"
        );
    }
}
