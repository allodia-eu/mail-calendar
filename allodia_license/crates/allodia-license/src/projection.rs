// SPDX-FileCopyrightText: 2026 Allodia
// SPDX-License-Identifier: LicenseRef-Allodia-1.0

//! Turning a stored account config into the shape the service holds, and back into a prefilled
//! setup.
//!
//! **The types here have no password field, and that is the point.** They read the same TOML the
//! credential store keeps, but they describe only the half that travels, so this module cannot
//! send a secret even by mistake, and nobody has to remember to strip one. A field added to the
//! stored config joins the payload only when somebody adds it here on purpose.
//!
//! **The reverse direction is not a config.** An account arriving from the service has no password
//! and cannot get one: it is entered once per device, in the keystore, and never travels. So
//! [`SyncedConfig`] converts into a **prefill** for the setup screen rather than into something the
//! core could store, which is the offers model expressed as a type, and the reason a device can
//! never end up with an account that looks configured and cannot connect.

use serde::Deserialize;

use crate::accounts::{
    CalDavEndpoint, ImapEndpoint, JmapAuth, Security, SmtpEndpoint, SyncedConfig,
};

/// Why a stored account cannot be represented in the shape the service holds.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum NotSyncable {
    /// The TOML is not one of the four kinds, or could not be read at all.
    #[error("this is not a mail account config")]
    NotAnAccount,
    /// A dial address that is not `host:port`.
    #[error("the {endpoint} address is not a host and a port: {addr}")]
    Address {
        /// Which endpoint, for the message.
        endpoint: &'static str,
        /// What was stored.
        addr: String,
    },
    /// The dial host and the TLS name disagree, which the service's shape has no room for.
    ///
    /// Rare, and deliberately refused rather than flattened: sending one of the two would give the
    /// other device a config that connects to the wrong name or verifies against the wrong
    /// certificate, and it would look like the account simply stopped working.
    #[error("the {endpoint} dial host and TLS name differ ({addr} vs {server_name})")]
    SplitHost {
        /// Which endpoint, for the message.
        endpoint: &'static str,
        /// The dial host.
        addr: String,
        /// The TLS name.
        server_name: String,
    },
}

/// The stored TOML, read for the fields that travel and no others.
#[derive(Deserialize)]
struct StoredDocument {
    imap: Option<StoredImapAccount>,
    smtp: Option<StoredEndpoint>,
    caldav: Option<StoredCalDav>,
    jmap: Option<StoredJmap>,
    google: Option<StoredProvider>,
    microsoft: Option<StoredProvider>,
}

#[derive(Deserialize)]
struct StoredImapAccount {
    addr: String,
    server_name: String,
    username: String,
    #[serde(default)]
    security: StoredSecurity,
}

#[derive(Deserialize)]
struct StoredEndpoint {
    addr: String,
    server_name: String,
    #[serde(default)]
    security: StoredSecurity,
}

#[derive(Deserialize)]
struct StoredCalDav {
    base_url: String,
    username: String,
    #[serde(default)]
    calendar: Option<String>,
}

#[derive(Deserialize)]
struct StoredJmap {
    #[serde(default)]
    email: Option<String>,
    base_url: String,
    /// Present when the account was connected by signing in rather than by pasting a secret. Read
    /// as a bare flag: what it contains is a grant, and no part of it travels.
    #[serde(default)]
    oauth: Option<toml::Value>,
}

#[derive(Deserialize)]
struct StoredProvider {
    email: String,
}

/// Mirrors the stored `security` values, defaulting exactly as the core's does.
#[derive(Deserialize, Default, Clone, Copy)]
enum StoredSecurity {
    #[default]
    #[serde(rename = "implicit-tls")]
    ImplicitTls,
    #[serde(rename = "starttls")]
    Starttls,
}

impl From<StoredSecurity> for Security {
    fn from(value: StoredSecurity) -> Self {
        match value {
            StoredSecurity::ImplicitTls => Self::ImplicitTls,
            StoredSecurity::Starttls => Self::Starttls,
        }
    }
}

/// Read a stored account config into the shape the service holds.
///
/// # Errors
/// [`NotSyncable`] when the config is not a mail account, or holds something this shape cannot
/// carry without changing what it means.
pub fn to_synced(stored_toml: &str) -> Result<SyncedConfig, NotSyncable> {
    let document: StoredDocument =
        toml::from_str(stored_toml).map_err(|_| NotSyncable::NotAnAccount)?;

    if let Some(google) = document.google {
        return Ok(SyncedConfig::Google {
            email: google.email,
        });
    }
    if let Some(microsoft) = document.microsoft {
        return Ok(SyncedConfig::Microsoft {
            email: microsoft.email,
        });
    }
    if let Some(jmap) = document.jmap {
        return Ok(SyncedConfig::Jmap {
            // A JMAP config stored before the address was recorded has none; the account is still
            // the person's, and the address is what the other device needs to recognise it.
            email: jmap.email.unwrap_or_default(),
            base_url: jmap.base_url,
            auth: if jmap.oauth.is_some() {
                JmapAuth::OAuth
            } else {
                JmapAuth::Secret
            },
        });
    }
    let Some(imap) = document.imap else {
        return Err(NotSyncable::NotAnAccount);
    };
    let (host, port) = split_addr("imap", &imap.addr)?;
    require_one_host("imap", host, &imap.server_name)?;
    Ok(SyncedConfig::Imap {
        email: imap.username.clone(),
        imap: ImapEndpoint {
            host: imap.server_name,
            port,
            security: imap.security.into(),
            username: imap.username,
        },
        smtp: document
            .smtp
            .map(|smtp| {
                let (host, port) = split_addr("smtp", &smtp.addr)?;
                require_one_host("smtp", host, &smtp.server_name)?;
                Ok(SmtpEndpoint {
                    host: smtp.server_name,
                    port,
                    security: smtp.security.into(),
                })
            })
            .transpose()?,
        caldav: document.caldav.map(|caldav| CalDavEndpoint {
            base_url: caldav.base_url,
            username: caldav.username,
            calendar: caldav.calendar,
        }),
    })
}

/// What an arriving account fills into the setup screen.
///
/// Deliberately not a stored config: there is no password here and there cannot be one, so the
/// person supplies it on this device and the setup path that already validates a credential is the
/// one that runs. A device never ends up with an account that looks configured and cannot connect.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SetupPrefill {
    /// Which of the four setup routes to open.
    pub kind: &'static str,
    /// The address to put in the address field.
    pub email: String,
    /// The mail host, for the two kinds that name one.
    pub host: Option<String>,
    /// The mail port, likewise.
    pub port: Option<u16>,
    /// Whether the mail connection is upgraded in-band.
    pub starttls: bool,
    /// The submission host and port, when the account had one.
    pub smtp: Option<(String, u16)>,
    /// Whether the submission connection is upgraded in-band.
    ///
    /// Its own field rather than folded into `starttls`: the two endpoints are configured
    /// separately and a server that wants implicit TLS on one and STARTTLS on the other is
    /// ordinary, so one flag for both would send a device to the wrong port with the right host.
    pub smtp_starttls: bool,
    /// The diary's collection root, when the account had one.
    pub caldav_base_url: Option<String>,
    /// The JMAP session resource, for that kind.
    pub jmap_base_url: Option<String>,
}

impl SyncedConfig {
    /// Turn an offer into what the setup screen should show.
    #[must_use]
    pub fn to_prefill(&self) -> SetupPrefill {
        let mut prefill = SetupPrefill {
            kind: self.kind(),
            email: self.email().to_owned(),
            host: None,
            port: None,
            starttls: false,
            smtp: None,
            smtp_starttls: false,
            caldav_base_url: None,
            jmap_base_url: None,
        };
        match self {
            Self::Imap {
                imap, smtp, caldav, ..
            } => {
                prefill.host = Some(imap.host.clone());
                prefill.port = Some(imap.port);
                prefill.starttls = imap.security == Security::Starttls;
                prefill.smtp = smtp.as_ref().map(|smtp| (smtp.host.clone(), smtp.port));
                prefill.smtp_starttls = smtp
                    .as_ref()
                    .is_some_and(|smtp| smtp.security == Security::Starttls);
                prefill.caldav_base_url = caldav.as_ref().map(|caldav| caldav.base_url.clone());
            }
            Self::Jmap { base_url, .. } => prefill.jmap_base_url = Some(base_url.clone()),
            Self::Google { .. } | Self::Microsoft { .. } => {}
        }
        prefill
    }
}

/// Split a stored `host:port` dial address.
fn split_addr<'a>(endpoint: &'static str, addr: &'a str) -> Result<(&'a str, u16), NotSyncable> {
    let fail = || NotSyncable::Address {
        endpoint,
        addr: addr.to_owned(),
    };
    // From the right: an IPv6 literal carries colons of its own, and one reaching here is a config
    // this shape cannot carry anyway, it is refused by the host check below rather than mangled.
    let (host, port) = addr.rsplit_once(':').ok_or_else(fail)?;
    let port: u16 = port.parse().map_err(|_| fail())?;
    if host.is_empty() || port == 0 {
        return Err(fail());
    }
    Ok((host, port))
}

/// Refuse a config whose dial host and TLS name are not the same name.
fn require_one_host(
    endpoint: &'static str,
    addr_host: &str,
    server_name: &str,
) -> Result<(), NotSyncable> {
    if addr_host.eq_ignore_ascii_case(server_name) {
        return Ok(());
    }
    Err(NotSyncable::SplitHost {
        endpoint,
        addr: addr_host.to_owned(),
        server_name: server_name.to_owned(),
    })
}

#[cfg(test)]
#[path = "projection_tests.rs"]
mod projection_tests;
