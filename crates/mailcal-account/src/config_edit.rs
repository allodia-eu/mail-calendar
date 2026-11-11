//! Editing a standards account's servers and sign-in after it was set up, without it becoming a
//! different account.

use crate::{
    AccountConfig, CalDavAccount, Capabilities, Capability, CardDavAccount, ConfigError,
    ConnectionSecurity, ImapAccount, Secret, SmtpAccount,
    setup::{host_and_addr, imap_default_port, normalize_caldav_base_url, smtp_default_port},
};

/// A standards account's servers and sign-in, as Settings shows and edits them. A server that is
/// `None` is one the account does not have.
#[derive(Clone, PartialEq, Eq)]
pub struct EndpointEdit {
    /// The IMAP server, `host` or `host:port`; the standard port for its security when bare.
    pub imap_host: Option<String>,
    /// How the IMAP connection is secured.
    pub imap_security: ConnectionSecurity,
    /// The SMTP server, `host` or `host:port`.
    pub smtp_host: Option<String>,
    /// How the SMTP connection is secured.
    pub smtp_security: ConnectionSecurity,
    /// The CalDAV base URL; `https://` is assumed when it names no scheme.
    pub caldav_url: Option<String>,
    /// The CardDAV base URL, when it is not the calendar's.
    pub carddav_url: Option<String>,
    /// The login every server takes.
    pub username: String,
    /// A new password for every server, or `None` to keep the stored one. Never filled in when
    /// read back.
    pub password: Option<String>,
}

/// Never the password, which a `Debug` would carry into a log.
impl std::fmt::Debug for EndpointEdit {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EndpointEdit")
            .field("imap_host", &self.imap_host)
            .field("imap_security", &self.imap_security)
            .field("smtp_host", &self.smtp_host)
            .field("smtp_security", &self.smtp_security)
            .field("caldav_url", &self.caldav_url)
            .field("carddav_url", &self.carddav_url)
            .field("username", &self.username)
            .field("password_given", &self.password.is_some())
            .finish()
    }
}

/// What an edit of an account's servers leaves out, which [`AccountConfig::with_endpoints`]
/// refuses and Settings says before Save is pressed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EndpointGap {
    /// No login.
    Username,
    /// A new password given empty.
    Password,
    /// The account's IMAP server left without an address.
    Imap,
    /// The account's SMTP server left without an address: mail could no longer be sent.
    Smtp,
    /// The account's CalDAV server left without an address.
    CalDav,
    /// The account's CardDAV server left without an address, with no calendar server for
    /// contacts to fall back to.
    CardDav,
}

impl EndpointEdit {
    /// What this edit of `account` leaves out, the first of them in the order Settings draws the
    /// fields, or `None` when it can be tried. A server the account has keeps an address; the
    /// CardDAV one may be cleared while the account has a calendar server, which then serves its
    /// contacts.
    #[must_use]
    pub fn gap(&self, account: &AccountConfig) -> Option<EndpointGap> {
        let blank =
            |field: &Option<String>| field.as_deref().is_none_or(|value| value.trim().is_empty());
        // A port with nothing before it is no address: a form joins a cleared host with the port
        // still in its own field.
        let no_host = |field: &Option<String>| {
            field
                .as_deref()
                .is_none_or(|value| value.trim().is_empty() || value.trim().starts_with(':'))
        };
        if account.imap.is_some() && no_host(&self.imap_host) {
            Some(EndpointGap::Imap)
        } else if account.smtp.is_some() && no_host(&self.smtp_host) {
            Some(EndpointGap::Smtp)
        } else if account.caldav.is_some() && blank(&self.caldav_url) {
            Some(EndpointGap::CalDav)
        } else if account.carddav.is_some() && blank(&self.carddav_url) && blank(&self.caldav_url) {
            Some(EndpointGap::CardDav)
        } else if self.username.trim().is_empty() {
            Some(EndpointGap::Username)
        } else if self.password.as_deref().is_some_and(str::is_empty) {
            Some(EndpointGap::Password)
        } else {
            None
        }
    }
}

/// An account after [`AccountConfig::with_endpoints`].
#[derive(Debug, Clone)]
pub struct EditedAccount {
    /// The edited config, its id and uses pinned.
    pub config: AccountConfig,
    /// The uses whose server moved to another host. What the device holds of them belongs to the
    /// old server and is synced again from the new one; a new port, security, login or password
    /// moves nothing.
    pub moved: Capabilities,
}

impl AccountConfig {
    /// The account's servers and login as Settings shows them, with no password.
    #[must_use]
    pub fn endpoints(&self) -> EndpointEdit {
        EndpointEdit {
            imap_host: self
                .imap
                .as_ref()
                .map(|imap| as_typed(&imap.addr, imap_default_port(imap.security))),
            imap_security: self
                .imap
                .as_ref()
                .map(|imap| imap.security)
                .unwrap_or_default(),
            smtp_host: self
                .smtp
                .as_ref()
                .map(|smtp| as_typed(&smtp.addr, smtp_default_port(smtp.security))),
            smtp_security: self
                .smtp
                .as_ref()
                .map(|smtp| smtp.security)
                .unwrap_or_default(),
            caldav_url: self.caldav.as_ref().map(|caldav| caldav.base_url.clone()),
            carddav_url: self
                .carddav
                .as_ref()
                .map(|carddav| carddav.base_url.clone()),
            username: self.username().to_owned(),
            password: None,
        }
    }

    /// This account with `edit`'s servers and login, keeping its id, its uses, its links, its
    /// accepted certificates and its sign-in.
    ///
    /// # Errors
    ///
    /// Returns [`ConfigError::Incomplete`] for what [`EndpointEdit::gap`] finds left out, and
    /// [`ConfigError::Refused`] when a use the account has would lose its server, or a password is
    /// given for an account that signs in through its provider.
    pub fn with_endpoints(&self, edit: &EndpointEdit) -> Result<EditedAccount, ConfigError> {
        if let Some(gap) = edit.gap(self) {
            return Err(ConfigError::Incomplete(match gap {
                EndpointGap::Username => "username",
                EndpointGap::Password => "password",
                EndpointGap::Imap => "mail server",
                EndpointGap::Smtp => "sending server",
                EndpointGap::CalDav => "calendar server",
                EndpointGap::CardDav => "address book server",
            }));
        }
        let username = edit.username.trim();
        let password = match (&edit.password, self.is_oauth()) {
            (Some(_), true) => {
                return Err(ConfigError::Refused(
                    "this account signs in through its provider, not with a password",
                ));
            }
            (Some(password), false) => Some(Secret::new(password.clone())),
            (None, _) => self.stored_password(),
        };
        let given = |field: &Option<String>| {
            field
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_owned)
        };

        let mut edited = self.clone();
        edited.imap = given(&edit.imap_host).map(|host| {
            let (server_name, addr) = host_and_addr(&host, imap_default_port(edit.imap_security));
            let server_name = kept_server_name(
                self.imap.as_ref().map(|old| (&old.addr, &old.server_name)),
                &addr,
                server_name,
            );
            ImapAccount {
                addr,
                server_name,
                username: username.to_owned(),
                password: password.clone(),
                security: edit.imap_security,
            }
        });
        edited.smtp = given(&edit.smtp_host).map(|host| {
            let (server_name, addr) = host_and_addr(&host, smtp_default_port(edit.smtp_security));
            let server_name = kept_server_name(
                self.smtp.as_ref().map(|old| (&old.addr, &old.server_name)),
                &addr,
                server_name,
            );
            SmtpAccount {
                addr,
                server_name,
                security: edit.smtp_security,
            }
        });
        edited.caldav = given(&edit.caldav_url).map(|url| CalDavAccount {
            base_url: normalize_caldav_base_url(&url),
            username: username.to_owned(),
            password: password.clone(),
        });
        edited.carddav = given(&edit.carddav_url).map(|url| CardDavAccount {
            base_url: normalize_caldav_base_url(&url),
            username: username.to_owned(),
            password: password.clone(),
        });

        let uses = self.capabilities();
        let served = [
            (Capability::Mail, edited.imap.is_some()),
            (Capability::Calendar, edited.caldav.is_some()),
            (
                Capability::Contacts,
                edited.carddav.is_some() || edited.caldav.is_some(),
            ),
        ];
        if served
            .iter()
            .any(|&(capability, has_server)| uses.contains(capability) && !has_server)
        {
            return Err(ConfigError::Refused(
                "a use the account has needs its server; switch the use off first",
            ));
        }
        edited.shape.id = self.shape.id.clone().or(self.account_id().ok());
        edited.shape.capabilities = Some(uses.clone());

        let host = |config: &AccountConfig, capability| match capability {
            Capability::Mail => config
                .imap
                .as_ref()
                .map(|imap| imap.server_name.to_lowercase()),
            Capability::Calendar => config.caldav_endpoint().map(|dav| host_of(dav.base_url)),
            _ => config.carddav_endpoint().map(|dav| host_of(dav.base_url)),
        };
        let moved = [Capability::Mail, Capability::Calendar, Capability::Contacts]
            .into_iter()
            .filter(|&capability| {
                uses.contains(capability) && host(self, capability) != host(&edited, capability)
            })
            .collect();
        // A shared mailbox's handle names a store on the mail server it was found on.
        if host(self, Capability::Mail) != host(&edited, Capability::Mail) {
            edited.shape.shared_mailboxes.clear();
        }
        // So does a shared calendar's, on the calendar server.
        if host(self, Capability::Calendar) != host(&edited, Capability::Calendar) {
            edited.shape.shared_calendars.clear();
        }
        Ok(EditedAccount {
            config: edited,
            moved,
        })
    }

    /// The password the account signs in with, wherever it is stored.
    fn stored_password(&self) -> Option<Secret> {
        self.imap
            .as_ref()
            .and_then(|imap| imap.password.clone())
            .or_else(|| self.caldav.as_ref().and_then(|dav| dav.password.clone()))
            .or_else(|| self.carddav.as_ref().and_then(|dav| dav.password.clone()))
    }
}

/// The TLS server name for a server dialled at `addr`: the stored one while the host is the one
/// it was stored for, since it may differ from the address (a server reached by its IP address
/// and verified by name), and otherwise the host itself.
fn kept_server_name(old: Option<(&String, &String)>, addr: &str, derived: String) -> String {
    match old {
        Some((old_addr, old_name)) if same_host(old_addr, addr) => old_name.clone(),
        _ => derived,
    }
}

/// A server's dial address as a person would type it: the bare host when the port is the standard
/// one for its security, so a form that switches only the security gets that security's port.
fn as_typed(addr: &str, default_port: u16) -> String {
    match addr.rsplit_once(':') {
        Some((host, port)) if port == default_port.to_string() => host.to_owned(),
        _ => addr.to_owned(),
    }
}

/// Whether two URLs name the same host, whatever their port or path.
fn same_host(a: &str, b: &str) -> bool {
    host_of(a) == host_of(b)
}

/// A URL's host, lowercased, without its scheme, login, port or path.
fn host_of(url: &str) -> String {
    let rest = url.split_once("://").map_or(url, |(_, rest)| rest);
    let authority = rest.split(['/', '?', '#']).next().unwrap_or_default();
    let host = authority
        .rsplit_once('@')
        .map_or(authority, |(_, host)| host);
    let host = match host.rsplit_once(':') {
        Some((name, port)) if !host.ends_with(']') && port.bytes().all(|b| b.is_ascii_digit()) => {
            name
        }
        _ => host,
    };
    host.to_lowercase()
}

#[cfg(test)]
#[path = "config_edit_tests.rs"]
mod tests;
