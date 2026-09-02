//! Keeping the mail-account list the same on every device: the records a client binds to.
//!
//! What travels is the half of an account that is not secret: the address, the server names and
//! ports, the connection settings. Never a password and never a provider token, so an account
//! arriving from another device is an **offer**, something to set up here, with the typing already
//! done; rather than an account that appears working and is not. `allodia-license`'s `projection`
//! module is where that line is drawn, by types that have no field to put a password in.
//!
//! The records live here rather than under a `cfg`, for the reason
//! [`crate::allodia`] gives: one FFI surface, whatever the build carries. A build with no Allodia
//! registration answers every call below by saying it has none.

use crate::{
    autodetect::{DetectedServerRow, MissReason, SetupRecommendation},
    setup::ConnectionSecurity,
};

/// Where an offer's settings came from, for the provenance line the setup card carries.
const OFFER_SOURCE: &str = "your other devices";

/// Which route sets an account up.
///
/// The same four the setup wizard offers, because an offer is a shortcut through it rather than a
/// fifth way in. The same address over IMAP and over JMAP is two accounts, not one, which is why
/// this is part of what makes two records the same account.
#[derive(uniffi::Enum, Debug, Clone, Copy, PartialEq, Eq)]
pub enum AllodiaAccountKind {
    /// IMAP, with optional submission and diary.
    Imap,
    /// JMAP, where one base URL carries everything.
    Jmap,
    /// Gmail and Google Calendar.
    Google,
    /// Microsoft 365.
    Microsoft,
}

/// An account one of the person's other devices set up, and what is already known about it.
///
/// Everything but the address is a shortcut: the setup path an offer opens is the ordinary one, so
/// a device that cannot use these settings detects its own and still ends up connected.
#[derive(uniffi::Record, Debug, Clone, PartialEq, Eq)]
pub struct AllodiaAccountOffer {
    /// The service's own id for the record. A client hands it back to nothing; it is here so an
    /// offer can be told from another offer for the same address.
    pub id: String,
    /// The address the account is for.
    pub email: String,
    /// Which route sets it up.
    pub kind: AllodiaAccountKind,
    /// The mail host, for the kinds that name one.
    pub host: Option<String>,
    /// The mail port.
    pub port: Option<u16>,
    /// How the mail connection is secured.
    pub security: Option<ConnectionSecurity>,
    /// The submission host, when the account had one.
    pub smtp_host: Option<String>,
    /// The submission port.
    pub smtp_port: Option<u16>,
    /// How the submission connection is secured. Its own field: an account reading over implicit
    /// TLS and submitting over STARTTLS is ordinary.
    pub smtp_security: Option<ConnectionSecurity>,
    /// The diary's collection root, when the account had one.
    pub caldav_base_url: Option<String>,
    /// The JMAP session resource, for that kind.
    pub jmap_base_url: Option<String>,
}

/// How one account is shared with the person's other devices.
///
/// **One setting, three positions**, because the two questions underneath it; *is this account on
/// my other devices* and *does this device exchange changes about it*, are not independent in any
/// way a person can act on, and splitting them into a switch and a button produced a screen where
/// turning the switch off changed nothing they could see.
///
/// It is read from the local bookkeeping, so a client may ask per account while drawing a list; it
/// never touches the network. Changing it does.
#[derive(uniffi::Enum, Debug, Clone, Copy, PartialEq, Eq)]
pub enum AllodiaAccountSyncMode {
    /// On the person's other devices, and kept in step with them.
    ///
    /// Also the answer for an account that has not been sent yet: it is one pass away, which is a
    /// moment rather than a state somebody chose.
    On,
    /// On their other devices, but this one has stopped exchanging changes about it.
    ///
    /// The record stays exactly as it was, so the other devices keep the account and lose nothing.
    /// What this device holds is its own from here, which is the point: a hostname that is right
    /// on this network and wrong everywhere else has somewhere to live.
    Paused,
    /// Not on their other devices at all. It lives only here, and Allodia holds no record of it.
    Off,
}

/// One of this device's accounts that the person changed somewhere else.
#[derive(uniffi::Record, Debug, Clone, PartialEq, Eq)]
pub struct AllodiaAccountChange {
    /// This device's id for the account.
    pub account_id: String,
    /// The address it is for.
    pub email: String,
    /// Whether this device changed it as well.
    ///
    /// Both sides moved, so there is no answer that is right by itself: taking the other device's
    /// settings discards an edit made here, and keeping this device's means it stops syncing.
    /// Moving this account to
    /// [`AllodiaAccountSyncMode::Paused`] is the second of those.
    pub also_changed_here: bool,
}

/// What one pass did, and what it could not decide alone.
///
/// Everything this device had to say is already said by the time this is returned: an account the
/// service had not seen is uploaded, a changed one is pushed, and one it turns out to already hold
/// is adopted. What comes back is only the part that needs a person: accounts to set up, accounts
/// that moved somewhere else, accounts removed somewhere else.
#[derive(uniffi::Record, Debug, Clone, Default, PartialEq, Eq)]
pub struct AllodiaSyncReport {
    /// Accounts from the person's other devices that this one has not got.
    pub offers: Vec<AllodiaAccountOffer>,
    /// This device's accounts whose settings changed elsewhere.
    pub changed_elsewhere: Vec<AllodiaAccountChange>,
    /// This device's accounts that were removed elsewhere. Removing one here is
    /// [`MailcalApp::remove_account`](crate::MailcalApp::remove_account), the same call as any
    /// other removal; keeping it is moving it to
    /// [`AllodiaAccountSyncMode::Paused`].
    pub removed_elsewhere: Vec<AllodiaAccountChange>,
    /// How many of this device's accounts this pass sent to the service.
    pub sent: u32,
}

/// The route an offer takes, as the same recommendation detection produces.
///
/// An offer already carries what detection goes looking for, which provider, which hosts, which
/// ports, which security; because a device that set the account up wrote it down. Re-deriving that
/// from the address is the one thing account sync exists to avoid: it spends a round trip to
/// re-learn what is in front of us, and for an address whose provider cannot be found from its
/// domain (a hosted IMAP domain publishing no autoconfig) it learns *less*, dropping the person
/// onto the manual form for an account another device set up without trouble.
///
/// **Trusted by construction.** `is_trusted` gates the approval an undiscovered config needs
/// (`docs/account-autodetect.md`): the case where a non-HTTPS hop could have chosen the server a
/// password is about to be sent to. Nothing here was discovered: these settings were approved on
/// the person's own device and arrived over HTTPS from their own account, so asking again asks
/// them to re-answer a question they have already answered.
///
/// It is a **route**, not a connection. The password is still asked for on this device, because no
/// password ever travels; a route that turns out to be wrong fails the way any wrong route does,
/// and the manual form is still behind it.
#[uniffi::export]
#[must_use]
pub fn setup_from_offer(offer: AllodiaAccountOffer) -> SetupRecommendation {
    match offer.kind {
        AllodiaAccountKind::Google => SetupRecommendation::Google { email: offer.email },
        AllodiaAccountKind::Microsoft => SetupRecommendation::Microsoft { email: offer.email },
        // A record naming no server is one this device cannot route from. Detection is the
        // fallback rather than an error: it finds the same server the other device did.
        AllodiaAccountKind::Jmap => offer.jmap_base_url.map_or(
            SetupRecommendation::Manual {
                reason: MissReason::NothingFound,
            },
            |server_url| SetupRecommendation::Jmap {
                email: offer.email,
                server_url,
                is_trusted: true,
                source: OFFER_SOURCE.to_owned(),
            },
        ),
        AllodiaAccountKind::Imap => imap_route(offer),
    }
}

/// The IMAP route, or the manual form when the record names no incoming server to take.
fn imap_route(offer: AllodiaAccountOffer) -> SetupRecommendation {
    let (Some(host), Some(port)) = (offer.host, offer.port) else {
        return SetupRecommendation::Manual {
            reason: MissReason::NothingFound,
        };
    };
    let imap_security = offer.security.unwrap_or(ConnectionSecurity::ImplicitTls);
    let smtp_security = offer
        .smtp_security
        .unwrap_or(ConnectionSecurity::ImplicitTls);
    // Submission needs both halves to be routable; one without the other is not a server.
    let smtp = offer.smtp_host.zip(offer.smtp_port);
    SetupRecommendation::Imap {
        // A restored account is described by what it was, not by a fresh detection: nothing
        // here re-read an autoconfig document, so no issuer was named.
        oauth_issuer: None,
        imap_host: host_with_port(&host, port, 993),
        smtp_host: smtp
            .as_ref()
            .map(|(host, port)| host_with_port(host, *port, 465)),
        imap_security,
        smtp_security,
        incoming: server_row("IMAP", &host, port, imap_security, &offer.email),
        outgoing: smtp
            .as_ref()
            .map(|(host, port)| server_row("SMTP", host, *port, smtp_security, &offer.email)),
        caldav_url: offer.caldav_base_url,
        is_trusted: true,
        source: OFFER_SOURCE.to_owned(),
        email: offer.email,
    }
}

/// `host`, or `host:port` when the port is not the standard one: the shape the setup form parses.
fn host_with_port(host: &str, port: u16, standard: u16) -> String {
    if port == standard {
        host.to_owned()
    } else {
        format!("{host}:{port}")
    }
}

fn server_row(
    protocol: &str,
    hostname: &str,
    port: u16,
    security: ConnectionSecurity,
    username: &str,
) -> DetectedServerRow {
    DetectedServerRow {
        protocol: protocol.to_owned(),
        hostname: hostname.to_owned(),
        port,
        security: match security {
            ConnectionSecurity::ImplicitTls => "SSL/TLS".to_owned(),
            ConnectionSecurity::StartTls => "STARTTLS".to_owned(),
        },
        username: username.to_owned(),
    }
}

#[cfg(feature = "allodia-license")]
mod from_license {
    use allodia_license::{SetupPrefill, SyncedAccount};

    use super::{AllodiaAccountKind, AllodiaAccountOffer};
    use crate::setup::ConnectionSecurity;

    impl AllodiaAccountKind {
        /// The wire label the service stores, as an enum a client can switch on.
        ///
        /// An unknown label cannot occur (the service and this build share the same four) but
        /// something has to be returned, and IMAP is the one route that asks for every field, so a
        /// record this version could not read would at least show the person what it holds.
        fn from_label(label: &str) -> Self {
            match label {
                "jmap" => Self::Jmap,
                "google" => Self::Google,
                "microsoft" => Self::Microsoft,
                _ => Self::Imap,
            }
        }
    }

    /// The security a prefilled endpoint dials with.
    ///
    /// `None` for a kind that names no endpoint, so a client can tell "implicit TLS" from "there
    /// is no such endpoint here".
    fn security(present: bool, starttls: bool) -> Option<ConnectionSecurity> {
        present.then_some(if starttls {
            ConnectionSecurity::StartTls
        } else {
            ConnectionSecurity::ImplicitTls
        })
    }

    impl AllodiaAccountOffer {
        /// What the service holds, as the fields a setup screen fills in.
        pub(crate) fn from_record(record: &SyncedAccount) -> Self {
            let prefill: SetupPrefill = record.config.to_prefill();
            let (smtp_host, smtp_port) = match prefill.smtp {
                Some((host, port)) => (Some(host), Some(port)),
                None => (None, None),
            };
            Self {
                id: record.id.clone(),
                kind: AllodiaAccountKind::from_label(prefill.kind),
                security: security(prefill.host.is_some(), prefill.starttls),
                smtp_security: security(smtp_host.is_some(), prefill.smtp_starttls),
                email: prefill.email,
                host: prefill.host,
                port: prefill.port,
                smtp_host,
                smtp_port,
                caldav_base_url: prefill.caldav_base_url,
                jmap_base_url: prefill.jmap_base_url,
            }
        }
    }
}

#[cfg(test)]
#[path = "allodia_sync_tests.rs"]
mod allodia_sync_tests;
