//! An account's servers and sign-in, on its page: for an account that signs in with a password,
//! whose servers are its own to change. Saving tries them first (`SaveServers`), so a typo never
//! costs a working account, and an expired password is replaced here too.

use std::rc::Rc;

use adw::prelude::*;
use mailcal_bindings::{
    AccountEndpoints, AccountEndpointsGap, AccountEntry, AccountPageIntent, AccountServers,
    ConnectionSecurity, MailServerKind, standard_port,
};

use super::{PageContext, group};
use crate::{l10n, ui::controls};

/// What the section says beneath its heading: the expired sign-in when there is one, otherwise
/// how saving works.
pub(super) fn description(address: &str, expired: bool) -> String {
    if expired {
        l10n::signin_expired_prompt(address)
    } else {
        l10n::settings_account_servers_description().to_owned()
    }
}

/// What Save says the fields leave out, naming the field as the form labels it.
pub(super) fn gap_said(gap: AccountEndpointsGap) -> String {
    l10n::settings_account_servers_gap(match gap {
        AccountEndpointsGap::Imap => l10n::settings_account_field_imap(),
        AccountEndpointsGap::Smtp => l10n::setup_field_smtp(),
        AccountEndpointsGap::CalDav => l10n::setup_field_caldav(),
        AccountEndpointsGap::CardDav => l10n::settings_account_field_carddav(),
        AccountEndpointsGap::Username => l10n::settings_account_field_login(),
        AccountEndpointsGap::Password => l10n::settings_account_field_new_password(),
    })
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

/// The section, for an account whose servers can be edited, drawn from the page's `servers`.
pub(super) fn server_group(
    ctx: &PageContext,
    entry: &AccountEntry,
    servers: &AccountServers,
    expired: bool,
) -> adw::PreferencesGroup {
    let section = group(
        l10n::settings_account_servers_heading(),
        &description(&super::account_names::entry_name(entry), expired),
    );
    let form = &servers.form;
    let imap = form.imap_host.as_deref().map(|address| {
        server_rows(
            &section,
            l10n::settings_account_field_imap(),
            address,
            form.imap_security,
            MailServerKind::Imap,
        )
    });
    let smtp = form.smtp_host.as_deref().map(|address| {
        server_rows(
            &section,
            l10n::setup_field_smtp(),
            address,
            form.smtp_security,
            MailServerKind::Smtp,
        )
    });
    let caldav = form
        .caldav_url
        .as_deref()
        .map(|url| entry_row(&section, l10n::setup_field_caldav(), url));
    let carddav = form
        .carddav_url
        .as_deref()
        .map(|url| entry_row(&section, l10n::settings_account_field_carddav(), url));
    let username = entry_row(
        &section,
        l10n::settings_account_field_login(),
        &form.username,
    );
    let password = adw::PasswordEntryRow::builder()
        .title(l10n::settings_account_field_new_password())
        .use_markup(false)
        .build();
    password.set_text(form.password.as_deref().unwrap_or_default());
    section.add(&password);
    // Held with Save, so nothing typed during a save is drawn over when it ends.
    let held = mailcal_bindings::ControlState {
        running: false,
        ..servers.save
    };
    for field in rows(&section) {
        controls::hold_field(&field, held);
    }

    let original = form.clone();
    let read = Rc::new(move || {
        edited(
            &original,
            Fields {
                imap_host: imap.as_ref().map(ServerRows::address),
                imap_security: imap.as_ref().map(ServerRows::security),
                smtp_host: smtp.as_ref().map(ServerRows::address),
                smtp_security: smtp.as_ref().map(ServerRows::security),
                caldav_url: caldav.as_ref().map(|row| row.text().to_string()),
                carddav_url: carddav.as_ref().map(|row| row.text().to_string()),
                username: username.text().to_string(),
                password: password.text().to_string(),
            },
        )
    });
    let drawn = Rc::new(std::cell::RefCell::new(read()));
    let sender = ctx.sender.clone();
    let (account, reading, last, group) = (
        entry.id.clone(),
        Rc::clone(&read),
        Rc::clone(&drawn),
        section.clone(),
    );
    let save = Rc::new(controls::FormSend::new(
        l10n::action_save(),
        servers.save,
        move || {
            let endpoints = reading();
            last.replace(endpoints.clone());
            controls::form_unsent(&group, false);
            super::account_page::change(
                &sender,
                AccountPageIntent::SaveServers {
                    account: account.clone(),
                    endpoints,
                },
            );
        },
    ));
    section.add(save.row());
    let (app, account) = (ctx.app.clone(), entry.id.clone());
    let follow = {
        let (section, save) = (section.clone(), Rc::clone(&save));
        move || {
            let now = read();
            controls::form_unsent(&section, now != *drawn.borrow());
            let gap = app.account_endpoints_gap(account.clone(), now);
            save.ready(gap.map(gap_said).as_deref());
        }
    };
    follow();
    let follow: Rc<dyn Fn()> = Rc::new(follow);
    watch(&section, &follow);
    section
}

/// The rows `section` holds so far.
fn rows(section: &adw::PreferencesGroup) -> Vec<gtk::Widget> {
    let mut found = Vec::new();
    collect_rows(section.upcast_ref(), &mut found);
    found
}

fn collect_rows(root: &gtk::Widget, found: &mut Vec<gtk::Widget>) {
    let mut child = root.first_child();
    while let Some(widget) = child {
        if widget.is::<adw::PreferencesRow>() {
            found.push(widget.clone());
        } else {
            collect_rows(&widget, found);
        }
        child = widget.next_sibling();
    }
}

/// Calls `changed` whenever a field or picker under `root` changes.
fn watch(root: &impl IsA<gtk::Widget>, changed: &Rc<dyn Fn()>) {
    let mut child = root.as_ref().first_child();
    while let Some(widget) = child {
        if let Some(editable) = widget.dynamic_cast_ref::<gtk::Editable>() {
            let changed = Rc::clone(changed);
            editable.connect_changed(move |_| changed());
        } else if let Some(picker) = widget.dynamic_cast_ref::<adw::ComboRow>() {
            let changed = Rc::clone(changed);
            picker.connect_selected_notify(move |_| changed());
        }
        watch(&widget, changed);
        child = widget.next_sibling();
    }
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

/// A stored address, `host` or `host:port`, as its host and its port, the default one when it
/// names none.
pub(super) fn split(
    address: &str,
    server: MailServerKind,
    security: ConnectionSecurity,
) -> (String, String) {
    let named = address.rsplit_once(':').filter(|(host, port)| {
        !port.is_empty() && port.bytes().all(|b| b.is_ascii_digit()) && !host.contains(':')
    });
    match named {
        Some((host, port)) => (host.to_owned(), port.to_owned()),
        None => (
            address.to_owned(),
            standard_port(server, security).to_string(),
        ),
    }
}

/// The address to store: the bare host when the port is the default for `security` or left
/// empty, `host:port` otherwise.
pub(super) fn joined(
    host: &str,
    port: &str,
    server: MailServerKind,
    security: ConnectionSecurity,
) -> String {
    let (host, port) = (host.trim(), port.trim());
    if port.is_empty() || port == standard_port(server, security).to_string() {
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
    server: MailServerKind,
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
    server: MailServerKind,
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
            && port.text().trim() == standard_port(server, previous.get()).to_string()
        {
            port.set_text(&standard_port(server, now).to_string());
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
    use mailcal_bindings::{AccountEndpoints, ConnectionSecurity, MailServerKind as Server};

    use super::{Fields, description, edited, joined, split};
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
