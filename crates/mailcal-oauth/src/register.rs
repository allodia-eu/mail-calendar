//! RFC 7591 **dynamic client registration**, and the scope selection that decides what a
//! newly registered client asks for.
//!
//! An installed mail client cannot ship a pre-registered `client_id` for a server it has
//! never met, and a self-hosted JMAP server has no way to give us one. RFC 7591 is the
//! standard answer: POST a client description to the `registration_endpoint` the
//! authorization-server metadata advertised, get a `client_id` back.
//!
//! # This is discoverable, not guaranteed
//!
//! Nothing obliges a server to offer open registration, and one that does may withdraw it.
//! So registration failure is **not** an error path to surface: the caller falls back to the
//! manual secret. The registered `client_id` is persisted with the account precisely so this
//! runs once, not on every launch.

use serde::{Deserialize, Serialize};

use crate::discovery::{AuthServerMetadata, DiscoveryError};

/// The scope that asks for a refresh token (RFC 6749 §1.5 / OIDC). Without it an access token
/// expires in an hour and the account dies with it, so it is always requested.
const OFFLINE_ACCESS: &str = "offline_access";

/// One kind of data the app asks a server for, and the names a server may advertise for it.
struct Capability {
    /// The profile's names, matched on a scope's **last segment** so the URN form
    /// (`urn:ietf:params:oauth:scope:mail`), a bare `mail` and a URL form are all recognised.
    standard: &'static [&'static str],
    /// The protocol-named scopes of `draft-ietf-mailmaint-oauth-public-00` and `-01`, matched
    /// whole. Later revisions replaced them with the names above, and deployed servers still
    /// advertise them, so they are asked for only where the standard name is not offered.
    earlier_draft: &'static [&'static str],
}

/// Mail, as the account's protocol names it in the earlier drafts.
const fn mail(protocol: MailProtocol) -> Capability {
    Capability {
        standard: &["mail"],
        earlier_draft: match protocol {
            MailProtocol::Imap => &["imap", "smtp"],
            MailProtocol::Jmap => &["jmap"],
        },
    }
}

/// Calendars and contacts are asked for too: the account presents the same grant to CalDAV and
/// CardDAV, and contacts decide whether a sender's face appears beside their mail. A server
/// that advertises neither is unaffected, since the selection only asks for what it offers.
const CALENDARS: Capability = Capability {
    standard: &["calendars", "calendar"],
    earlier_draft: &["caldav"],
};
const CONTACTS: Capability = Capability {
    standard: &["contacts"],
    earlier_draft: &["carddav"],
};

/// Which protocol carries the account's mail, which decides the earlier drafts' mail scopes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MailProtocol {
    /// IMAP for reading and SMTP submission for sending.
    Imap,
    /// JMAP for both.
    Jmap,
}

/// A use of an account that its mail grant reaches over DAV.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DavUse {
    /// CalDAV.
    Calendars,
    /// CardDAV.
    Contacts,
}

/// Whether `requested` (the scopes a sign-in asked for) names `dav_use`: its standard name, or
/// the earlier drafts' `caldav`/`carddav`.
///
/// The **requested** set, not the granted one: a server may answer in its own vocabulary
/// (Fastmail grants `urn:ietf:params:jmap:calendars` and `https://www.fastmail.com/dev/protocol-*`
/// for a request naming `urn:ietf:params:oauth:scope:calendars`), so the granted set says only
/// what was refused when it is written in the request's names. A grant that asked for no
/// calendar scope has no claim on a calendar server, whatever that server accepts: Stalwart
/// opens CalDAV for a mail-only token, so this check is the client's to make.
///
/// A sign-in that named no scope beyond `offline_access` is not refused: its issuer advertised
/// none, applies its own default grant (RFC 6749 §3.3), and said nothing either way, as
/// [`grants_mail_access`] reads the same case for mail.
#[must_use]
pub fn requests_dav_use(requested: &[String], dav_use: DavUse) -> bool {
    if requested.iter().all(|scope| scope == OFFLINE_ACCESS) {
        return true;
    }
    let capability = match dav_use {
        DavUse::Calendars => CALENDARS,
        DavUse::Contacts => CONTACTS,
    };
    requested.iter().any(|scope| {
        capability.standard.contains(&last_segment(scope))
            || capability.earlier_draft.contains(&scope.as_str())
    })
}

/// A scope's last segment: `mail` for `urn:ietf:params:oauth:scope:mail` and for `mail`.
fn last_segment(scope: &str) -> &str {
    scope.rsplit([':', '/']).next().unwrap_or(scope)
}

/// The advertised scopes that name `capability`: the standard names when any is offered, the
/// earlier drafts' otherwise. Never both, so a server offering the two forms shows one
/// permission per kind of data on its consent screen.
fn advertised<'a>(metadata: &'a AuthServerMetadata, capability: &Capability) -> Vec<&'a str> {
    let standard: Vec<&str> = metadata
        .scopes_supported
        .iter()
        .map(String::as_str)
        .filter(|scope| capability.standard.contains(&last_segment(scope)))
        .collect();
    if !standard.is_empty() {
        return standard;
    }
    metadata
        .scopes_supported
        .iter()
        .map(String::as_str)
        .filter(|scope| capability.earlier_draft.contains(scope))
        .collect()
}

/// What a completed registration gave us.
#[derive(Debug, Clone)]
pub struct RegisteredClient {
    /// The issued client identifier. Not a secret.
    pub client_id: String,
    /// A client secret, when the server issued one despite our asking for a public client.
    /// Carried through to the token exchange because a server that issues one generally
    /// requires it, but never treated as confidential: an installed app cannot keep it.
    pub client_secret: Option<String>,
}

/// This software's stable identifier (RFC 7591 §2 `software_id`): a UUID assigned once by the
/// publisher and **identical across every install and every version** of Allodia Mail &
/// Calendar.
///
/// It is the closest thing to "a stable client id we choose" that the standard allows. It is
/// *not* the `client_id`; dynamic registration always has the **server** mint that, and there is
/// no way for a client to assert one, but it lets a server recognise which software is
/// registering, across installs it has never seen before, without any prior relationship. A
/// server that wants to display, group, or apply policy to Allodia clients has something stable
/// to key on. Never change it.
const SOFTWARE_ID: &str = "df16a4a8-25e5-428c-907a-a789a3a7b52e";

/// This build's version, sent as RFC 7591 `software_version` beside [`SOFTWARE_ID`].
///
/// The pair is what lets a server tell "the same software, a newer release" from "different
/// software": the id never moves, the version moves every release. One version for the whole
/// product ([`docs/versioning.md`](../../../docs/versioning.md)), so the crate's own is it.
const SOFTWARE_VERSION: &str = env!("CARGO_PKG_VERSION");

/// The RFC 7591 registration request body.
///
/// `draft-ietf-mailmaint-oauth-public` also lists `client_uri`, `logo_uri`, `tos_uri` and
/// `policy_uri`: the branded links a consent screen shows beside the client name. This tree
/// does not hold them: publisher URLs travel with the brand rather than with the source
/// ([`docs/branding.md`](../../../docs/branding.md), "Publisher metadata"), and inventing ones
/// that may 404 on somebody's consent screen is worse than sending none.
#[derive(Serialize)]
struct RegistrationRequest<'a> {
    client_name: &'a str,
    software_id: &'a str,
    software_version: &'a str,
    redirect_uris: Vec<&'a str>,
    grant_types: Vec<&'a str>,
    response_types: Vec<&'a str>,
    /// `none` declares a public client: PKCE protects the exchange, and we hold no secret.
    token_endpoint_auth_method: &'a str,
    /// `native` tells the server this is an installed app with a custom-scheme redirect,
    /// which is what makes a non-HTTPS `redirect_uri` acceptable to it (RFC 8252).
    application_type: &'a str,
    scope: &'a str,
}

/// The RFC 7591 success body (`201`), reduced to what we keep.
#[derive(Deserialize)]
struct RegistrationResponse {
    client_id: String,
    client_secret: Option<String>,
}

/// Chooses the scopes to request from what the server advertises.
///
/// Always includes `offline_access` (no refresh token, no account an hour later). Beyond
/// that it requests **only** the advertised scopes naming mail, calendars or contacts, in the
/// order the server lists them; never the whole `scopes_supported` list, which on a real server
/// includes admin and other grants we have no business holding.
///
/// A server that advertises no scopes at all gets `offline_access` alone and is left to apply
/// its own default grant, which is the RFC 6749 §3.3 behaviour.
#[must_use]
pub fn select_scopes(metadata: &AuthServerMetadata, protocol: MailProtocol) -> Vec<String> {
    let wanted: Vec<&str> = [mail(protocol), CALENDARS, CONTACTS]
        .iter()
        .flat_map(|capability| advertised(metadata, capability))
        .collect();
    let mut scopes = vec![OFFLINE_ACCESS.to_owned()];
    scopes.extend(
        metadata
            .scopes_supported
            .iter()
            .filter(|scope| *scope != OFFLINE_ACCESS && wanted.contains(&scope.as_str()))
            .cloned(),
    );
    scopes
}

/// Whether the selected scopes cover mail: the minimum for a usable mail account.
///
/// A server that advertises scopes but none we recognise as mail would hand us a token good
/// for nothing, and the failure would surface much later as an empty mailbox. Treat it as
/// "discovery didn't work here" and fall back. A server advertising **no** scopes is a
/// different case (it applies its own default), so it passes.
///
/// An IMAP account needs the earlier drafts' `imap` **and** `smtp`: it reads over one and sends
/// over the other, and a token for only the first fails at the first message sent.
#[must_use]
pub fn grants_mail_access(
    metadata: &AuthServerMetadata,
    selected: &[String],
    protocol: MailProtocol,
) -> bool {
    let has = |name: &str| selected.iter().any(|scope| scope == name);
    metadata.scopes_supported.is_empty()
        || selected.iter().any(|scope| last_segment(scope) == "mail")
        || mail(protocol).earlier_draft.iter().all(|name| has(name))
}

/// Registers this app as a new public client with the authorization server (RFC 7591).
///
/// `client_name` is what the server shows the user on the consent screen, and `redirect_uri`
/// is the custom scheme the platform auth session captures.
///
/// # Errors
///
/// Returns [`DiscoveryError::NoAuthorizationServer`] if the server advertises no
/// `registration_endpoint`, [`DiscoveryError::MalformedMetadata`] if it rejects the
/// registration or answers with something unparseable, or
/// [`DiscoveryError::Transport`] on a network failure. Every one of them means "fall back to
/// the manual secret".
pub async fn register_client(
    http: &reqwest::Client,
    metadata: &AuthServerMetadata,
    client_name: &str,
    redirect_uri: &str,
    scopes: &[String],
) -> Result<RegisteredClient, DiscoveryError> {
    let endpoint = metadata
        .registration_endpoint
        .as_deref()
        .ok_or_else(|| DiscoveryError::NoAuthorizationServer(metadata.issuer.clone()))?;
    let request = RegistrationRequest {
        client_name,
        software_id: SOFTWARE_ID,
        software_version: SOFTWARE_VERSION,
        redirect_uris: vec![redirect_uri],
        grant_types: vec!["authorization_code", "refresh_token"],
        response_types: vec!["code"],
        token_endpoint_auth_method: "none",
        application_type: "native",
        scope: &scopes.join(" "),
    };
    let response = http
        .post(endpoint)
        .json(&request)
        .send()
        .await
        .map_err(DiscoveryError::Transport)?;
    let status = response.status();
    let body = response.text().await.map_err(DiscoveryError::Transport)?;
    if !status.is_success() {
        return Err(DiscoveryError::MalformedMetadata {
            url: endpoint.to_owned(),
            detail: format!("registration refused: http {}; {body}", status.as_u16()),
        });
    }
    let parsed: RegistrationResponse =
        serde_json::from_str(&body).map_err(|err| DiscoveryError::MalformedMetadata {
            url: endpoint.to_owned(),
            detail: err.to_string(),
        })?;
    Ok(RegisteredClient {
        client_id: parsed.client_id,
        client_secret: parsed.client_secret,
    })
}

#[cfg(test)]
#[path = "register_tests.rs"]
mod register_tests;
