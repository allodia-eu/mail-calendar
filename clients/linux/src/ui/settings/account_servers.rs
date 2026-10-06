//! An account's servers and sign-in, on its page: for an account that signs in with a password,
//! whose servers are its own to change. Saving tries them first (`update_account_endpoints`), so a
//! typo never costs a working account, and an expired password is replaced here too.

use adw::prelude::*;
use mailcal_bindings::{AccountEndpoints, AccountEntry, ConnectionSecurity};

use super::{PageContext, group};
use crate::{
    l10n,
    ui::{
        AppInput,
        account_settings::{AccountsInput, EndpointsEdit},
    },
};

/// What the section says beneath its heading: the expired sign-in when there is one, otherwise
/// how saving works.
pub(super) fn description(address: &str, expired: bool) -> String {
    if expired {
        l10n::signin_expired_prompt(address)
    } else {
        l10n::settings_account_servers_description().to_owned()
    }
}

/// The fields as the person left them.
#[derive(Debug, Default, PartialEq, Eq)]
pub(super) struct Fields {
    pub(super) imap_host: Option<String>,
    pub(super) imap_security: Option<ConnectionSecurity>,
    pub(super) smtp_host: Option<String>,
    pub(super) smtp_security: Option<ConnectionSecurity>,
    pub(super) caldav_url: Option<String>,
    pub(super) carddav_url: Option<String>,
    pub(super) username: String,
    pub(super) password: String,
}

/// The servers to save: `original` with what the fields changed. A server the account does not
/// have stays absent, one with no field keeps its value, and an empty password keeps the stored
/// one.
pub(super) fn edited(original: &AccountEndpoints, fields: Fields) -> AccountEndpoints {
    let kept = |old: &Option<String>, typed: Option<String>| {
        old.as_ref().map(|old| typed.unwrap_or_else(|| old.clone()))
    };
    AccountEndpoints {
        imap_host: kept(&original.imap_host, fields.imap_host),
        imap_security: fields.imap_security.unwrap_or(original.imap_security),
        smtp_host: kept(&original.smtp_host, fields.smtp_host),
        smtp_security: fields.smtp_security.unwrap_or(original.smtp_security),
        caldav_url: kept(&original.caldav_url, fields.caldav_url),
        carddav_url: kept(&original.carddav_url, fields.carddav_url),
        username: fields.username,
        password: (!fields.password.is_empty()).then_some(fields.password),
    }
}

/// The section, for an account whose servers can be edited.
pub(super) fn server_group(
    ctx: &PageContext,
    entry: &AccountEntry,
    endpoints: &AccountEndpoints,
    expired: bool,
) -> adw::PreferencesGroup {
    let section = group(
        l10n::settings_account_servers_heading(),
        &description(&entry.address, expired),
    );
    let imap = endpoints.imap_host.as_deref().map(|address| {
        server_rows(
            &section,
            l10n::settings_account_field_imap(),
            address,
            endpoints.imap_security,
            Server::Imap,
        )
    });
    let smtp = endpoints.smtp_host.as_deref().map(|address| {
        server_rows(
            &section,
            l10n::setup_field_smtp(),
            address,
            endpoints.smtp_security,
            Server::Smtp,
        )
    });
    let caldav = endpoints
        .caldav_url
        .as_deref()
        .map(|url| entry_row(&section, l10n::setup_field_caldav(), url));
    let carddav = endpoints
        .carddav_url
        .as_deref()
        .map(|url| entry_row(&section, l10n::settings_account_field_carddav(), url));
    let username = entry_row(
        &section,
        l10n::settings_account_field_login(),
        &endpoints.username,
    );
    let password = adw::PasswordEntryRow::builder()
        .title(l10n::settings_account_field_new_password())
        .use_markup(false)
        .build();
    section.add(&password);

    let save = adw::ButtonRow::builder()
        .title(l10n::action_save())
        .use_markup(false)
        .build();
    save.add_css_class("suggested-action");
    let sender = ctx.sender.clone();
    let account = entry.id.clone();
    let original = endpoints.clone();
    save.connect_activated(move |_| {
        let fields = Fields {
            imap_host: imap.as_ref().map(ServerRows::address),
            imap_security: imap.as_ref().map(ServerRows::security),
            smtp_host: smtp.as_ref().map(ServerRows::address),
            smtp_security: smtp.as_ref().map(ServerRows::security),
            caldav_url: caldav.as_ref().map(|row| row.text().to_string()),
            carddav_url: carddav.as_ref().map(|row| row.text().to_string()),
            username: username.text().to_string(),
            password: password.text().to_string(),
        };
        sender.emit(AppInput::Accounts(AccountsInput::SaveEndpoints(Box::new(
            EndpointsEdit {
                account: account.clone(),
                endpoints: edited(&original, fields),
            },
        ))));
    });
    section.add(&save);
    section
}

fn entry_row(section: &adw::PreferencesGroup, title: &str, text: &str) -> adw::EntryRow {
    let row = adw::EntryRow::builder()
        .title(title)
        .text(text)
        .use_markup(false)
        .build();
    section.add(&row);
    row
}

/// Which server a row edits, for the port it takes by default.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Server {
    Imap,
    Smtp,
}

impl Server {
    /// The standard port for `security`, which a stored address leaves out.
    pub(super) const fn default_port(self, security: ConnectionSecurity) -> u16 {
        match (self, security) {
            (Self::Imap, ConnectionSecurity::ImplicitTls) => 993,
            (Self::Imap, ConnectionSecurity::StartTls) => 143,
            (Self::Smtp, ConnectionSecurity::ImplicitTls) => 465,
            (Self::Smtp, ConnectionSecurity::StartTls) => 587,
        }
    }
}

/// A stored address, `host` or `host:port`, as its host and its port, the default one when it
/// names none.
pub(super) fn split(
    address: &str,
    server: Server,
    security: ConnectionSecurity,
) -> (String, String) {
    let named = address.rsplit_once(':').filter(|(host, port)| {
        !port.is_empty() && port.bytes().all(|b| b.is_ascii_digit()) && !host.contains(':')
    });
    match named {
        Some((host, port)) => (host.to_owned(), port.to_owned()),
        None => (
            address.to_owned(),
            server.default_port(security).to_string(),
        ),
    }
}

/// The address to store: the bare host when the port is the default for `security` or left
/// empty, `host:port` otherwise.
pub(super) fn joined(
    host: &str,
    port: &str,
    server: Server,
    security: ConnectionSecurity,
) -> String {
    let (host, port) = (host.trim(), port.trim());
    if port.is_empty() || port == server.default_port(security).to_string() {
        host.to_owned()
    } else {
        format!("{host}:{port}")
    }
}

/// One server's rows: its host, its port, and how the connection is secured.
struct ServerRows {
    host: adw::EntryRow,
    port: adw::EntryRow,
    picker: adw::ComboRow,
    server: Server,
}

impl ServerRows {
    fn security(&self) -> ConnectionSecurity {
        if self.picker.selected() == 1 {
            ConnectionSecurity::StartTls
        } else {
            ConnectionSecurity::ImplicitTls
        }
    }

    fn address(&self) -> String {
        joined(
            &self.host.text(),
            &self.port.text(),
            self.server,
            self.security(),
        )
    }
}

fn server_rows(
    section: &adw::PreferencesGroup,
    title: &str,
    address: &str,
    security: ConnectionSecurity,
    server: Server,
) -> ServerRows {
    let (host, port) = split(address, server, security);
    let host = entry_row(section, title, &host);
    let port = entry_row(section, l10n::setup_field_port(), &port);
    port.set_input_purpose(gtk::InputPurpose::Digits);
    let choices = gtk::StringList::new(&[
        l10n::setup_security_implicit_tls(),
        l10n::setup_security_starttls(),
    ]);
    let picker = adw::ComboRow::builder()
        .title(l10n::setup_field_security())
        .model(&choices)
        .selected(u32::from(security == ConnectionSecurity::StartTls))
        .use_markup(false)
        .build();
    section.add(&picker);
    // A port still at the old security's default moves to the new one's, as on the setup form;
    // one the person typed stays.
    let previous = std::rc::Rc::new(std::cell::Cell::new(security));
    let following = port.downgrade();
    picker.connect_selected_notify(move |picker| {
        let now = if picker.selected() == 1 {
            ConnectionSecurity::StartTls
        } else {
            ConnectionSecurity::ImplicitTls
        };
        if let Some(port) = following.upgrade()
            && port.text().trim() == server.default_port(previous.get()).to_string()
        {
            port.set_text(&server.default_port(now).to_string());
        }
        previous.set(now);
    });
    ServerRows {
        host,
        port,
        picker,
        server,
    }
}

#[cfg(test)]
mod tests {
    use mailcal_bindings::{AccountEndpoints, ConnectionSecurity};

    use super::{Fields, Server, description, edited, joined, split};
    use crate::l10n;

    fn mailbox() -> AccountEndpoints {
        AccountEndpoints {
            imap_host: Some("imap.example.org".to_owned()),
            imap_security: ConnectionSecurity::ImplicitTls,
            smtp_host: Some("smtp.example.org".to_owned()),
            smtp_security: ConnectionSecurity::StartTls,
            caldav_url: None,
            carddav_url: None,
            username: "alice".to_owned(),
            password: None,
        }
    }

    #[test]
    fn an_empty_password_keeps_the_stored_one_and_a_typed_one_replaces_it() {
        let fields = || Fields {
            imap_host: Some("imap.example.org".to_owned()),
            smtp_host: Some("smtp.example.org".to_owned()),
            username: "alice".to_owned(),
            ..Fields::default()
        };
        assert_eq!(edited(&mailbox(), fields()), mailbox());

        let mut changed = fields();
        changed.password = "new secret".to_owned();
        changed.imap_security = Some(ConnectionSecurity::StartTls);
        let endpoints = edited(&mailbox(), changed);
        assert_eq!(endpoints.password.as_deref(), Some("new secret"));
        assert_eq!(endpoints.imap_security, ConnectionSecurity::StartTls);
    }

    #[test]
    fn a_server_the_account_does_not_have_is_never_added_by_the_form() {
        let fields = Fields {
            caldav_url: Some("https://dav.example.org".to_owned()),
            username: "alice".to_owned(),
            ..Fields::default()
        };
        let endpoints = edited(&mailbox(), fields);
        assert_eq!(endpoints.caldav_url, None);
        // And a server with no field keeps what it had.
        assert_eq!(endpoints.imap_host.as_deref(), Some("imap.example.org"));
    }

    #[test]
    fn a_stored_address_shows_its_port_and_a_default_port_is_stored_bare() {
        let tls = ConnectionSecurity::ImplicitTls;
        let starttls = ConnectionSecurity::StartTls;
        assert_eq!(
            split("imap.example.org", Server::Imap, tls),
            ("imap.example.org".to_owned(), "993".to_owned())
        );
        assert_eq!(
            split("smtp.example.org", Server::Smtp, starttls),
            ("smtp.example.org".to_owned(), "587".to_owned())
        );
        assert_eq!(
            split("127.0.0.1:12993", Server::Imap, tls),
            ("127.0.0.1".to_owned(), "12993".to_owned())
        );

        assert_eq!(
            joined("imap.example.org", "993", Server::Imap, tls),
            "imap.example.org"
        );
        assert_eq!(
            joined("imap.example.org", "", Server::Imap, tls),
            "imap.example.org"
        );
        assert_eq!(
            joined(" imap.example.org ", "1143", Server::Imap, tls),
            "imap.example.org:1143"
        );
        // A port that is the default for the other security is not this one's default.
        assert_eq!(
            joined("imap.example.org", "143", Server::Imap, tls),
            "imap.example.org:143"
        );
        assert_eq!(
            joined("imap.example.org", "143", Server::Imap, starttls),
            "imap.example.org"
        );
    }

    #[test]
    fn the_section_says_when_the_sign_in_has_expired() {
        assert_eq!(
            description("alice@example.org", true),
            l10n::signin_expired_prompt("alice@example.org")
        );
        assert_eq!(
            description("alice@example.org", false),
            l10n::settings_account_servers_description()
        );
    }
}
