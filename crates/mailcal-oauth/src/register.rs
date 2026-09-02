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

/// The capability scopes a mail-and-calendar client needs, matched against what a server
/// advertises.
///
/// These are matched on a scope's **last segment**, so both the URN form JMAP servers use
/// (`urn:ietf:params:oauth:scope:mail`) and a bare `mail` are recognised: the pattern is the
/// IETF-registered naming, not one provider's spelling.
///
/// `contacts` is now among them. It was deliberately absent while the only thing it bought was
/// a Contacts list, because adding it re-prompts every already-connected JMAP account, and the
/// same account added with an app password showed a full list while the OAuth one showed an
/// empty one, which was the sharper half of the cost. Two things changed the balance: contacts
/// now also decide whether a sender's face appears beside their mail, and the Microsoft and
/// Google sets moved in the same release, so a user who reconnects pays one prompt across every
/// provider rather than one per provider per release.
///
/// A server that advertises no `contacts` scope is unaffected: the selection only ever asks for
/// what the metadata offers.
const WANTED_CAPABILITIES: &[&str] = &["mail", "calendars", "calendar", "contacts"];

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
/// that it requests **only** the advertised scopes whose last segment names a capability we
/// actually use; never the whole `scopes_supported` list, which on a real server includes
/// contacts, admin and other grants we have no business holding.
///
/// A server that advertises no scopes at all gets `offline_access` alone and is left to apply
/// its own default grant, which is the RFC 6749 §3.3 behaviour.
#[must_use]
pub fn select_scopes(metadata: &AuthServerMetadata) -> Vec<String> {
    let mut scopes = vec![OFFLINE_ACCESS.to_owned()];
    for scope in &metadata.scopes_supported {
        if scope == OFFLINE_ACCESS {
            continue;
        }
        let segment = scope.rsplit([':', '/']).next().unwrap_or(scope);
        if WANTED_CAPABILITIES.contains(&segment) {
            scopes.push(scope.clone());
        }
    }
    scopes
}

/// Whether the selected scopes include a mail capability: the minimum for a usable mail
/// account.
///
/// A server that advertises scopes but none we recognise as mail would hand us a token good
/// for nothing, and the failure would surface much later as an empty mailbox. Treat it as
/// "discovery didn't work here" and fall back. A server advertising **no** scopes is a
/// different case (it applies its own default), so it passes.
#[must_use]
pub fn grants_mail_access(metadata: &AuthServerMetadata, selected: &[String]) -> bool {
    metadata.scopes_supported.is_empty()
        || selected
            .iter()
            .any(|scope| scope.rsplit([':', '/']).next().unwrap_or(scope) == "mail")
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
mod tests {
    use super::*;

    fn metadata(scopes: &[&str]) -> AuthServerMetadata {
        AuthServerMetadata {
            issuer: "https://as.example.com".to_owned(),
            authorization_endpoint: "https://as.example.com/authorize".to_owned(),
            token_endpoint: "https://as.example.com/token".to_owned(),
            registration_endpoint: Some("https://as.example.com/register".to_owned()),
            revocation_endpoint: None,
            userinfo_endpoint: None,
            scopes_supported: scopes.iter().map(|s| (*s).to_owned()).collect(),
            end_session_endpoint: None,
            prompt_values_supported: Vec::new(),
            issuer_parameter_supported: false,
        }
    }

    #[test]
    fn the_software_id_is_a_fixed_uuid() {
        // It identifies the *software*, not an install, so it must never be generated per device
        // or per run: a server keying on it would then see every install as different software.
        assert_eq!(SOFTWARE_ID, "df16a4a8-25e5-428c-907a-a789a3a7b52e");
        assert_eq!(SOFTWARE_ID.len(), 36);
    }

    #[test]
    fn offline_access_is_always_requested() {
        // Without it the server issues no refresh token and the account breaks an hour after
        // setup: the single most expensive thing to get wrong here.
        assert!(select_scopes(&metadata(&[])).contains(&OFFLINE_ACCESS.to_owned()));
        assert_eq!(
            select_scopes(&metadata(&["urn:ietf:params:oauth:scope:mail"]))[0],
            OFFLINE_ACCESS
        );
    }

    #[test]
    fn only_the_capabilities_we_use_are_requested() {
        // The shape a real JMAP server advertises. Mail, calendars and contacts are all
        // exercised by the app; admin is not, and an over-broad consent screen is a
        // user-visible harm; we cannot honestly ask for a permission we never use.
        let selected = select_scopes(&metadata(&[
            "urn:ietf:params:oauth:scope:mail",
            "urn:ietf:params:oauth:scope:contacts",
            "urn:ietf:params:oauth:scope:calendars",
            "https://example.com/auth/admin",
            "offline_access",
        ]));
        assert_eq!(
            selected,
            vec![
                "offline_access",
                "urn:ietf:params:oauth:scope:mail",
                "urn:ietf:params:oauth:scope:contacts",
                "urn:ietf:params:oauth:scope:calendars",
            ]
        );
        assert!(
            !selected.iter().any(|scope| scope.ends_with("admin")),
            "a capability the app never exercises must not reach the consent screen"
        );
    }

    #[test]
    fn bare_and_path_style_scope_names_are_recognised_too() {
        // The match is on the last segment, so the URN form, a bare word and a URL form all
        // work: this is the "generic from the standards" rule, not one provider's spelling.
        assert!(select_scopes(&metadata(&["mail"])).contains(&"mail".to_owned()));
        assert!(
            select_scopes(&metadata(&["https://example.com/auth/calendar"]))
                .contains(&"https://example.com/auth/calendar".to_owned())
        );
    }

    #[test]
    fn offline_access_is_never_duplicated_when_the_server_advertises_it() {
        let selected = select_scopes(&metadata(&["offline_access", "mail"]));
        assert_eq!(
            selected.iter().filter(|s| *s == OFFLINE_ACCESS).count(),
            1,
            "a duplicated scope is rejected outright by some servers: {selected:?}"
        );
    }

    #[test]
    fn a_server_offering_no_mail_scope_is_not_usable() {
        // A token good for calendars alone would surface much later as a permanently empty
        // mailbox. Catch it at discovery and fall back to the manual secret instead.
        let meta = metadata(&["urn:ietf:params:oauth:scope:calendars"]);
        let selected = select_scopes(&meta);
        assert!(!grants_mail_access(&meta, &selected));

        let meta = metadata(&["urn:ietf:params:oauth:scope:mail"]);
        let selected = select_scopes(&meta);
        assert!(grants_mail_access(&meta, &selected));
    }

    #[test]
    fn a_server_advertising_no_scopes_at_all_is_allowed_through() {
        // RFC 6749 §3.3: with no scope advertised the server applies its own default grant.
        // That is legal and common on small self-hosted servers, so it must not be mistaken
        // for "no mail access".
        let meta = metadata(&[]);
        assert!(grants_mail_access(&meta, &select_scopes(&meta)));
    }
}
