//! The OAuth client registrations a build carries, read from the environment at **compile time**.
//! The single place any Google, Microsoft or Allodia client id or secret enters the product;
//! nothing else in the tree may hold one.
//!
//! A registration is optional, and its absence is a supported build. A build without Google's
//! registration does not offer Google sign-in at all: the setup wizard drops the route and
//! detection stops recommending it: so a from-source build is a working open-standards client
//! (IMAP, JMAP, CalDAV, CardDAV), never a dead button. The same holds for Microsoft.
//!
//! **A blank value is an absent value.** A CI run without access to the secrets sets the
//! variables to the empty string rather than leaving them unset, and a build carrying
//! `client_id=""` would fail at the provider instead of at the wizard.
//!
//! Google issues a **separate client per client type** and the type decides the redirect and
//! whether a secret is involved, so the three cannot share one variable:
//!
//! | Target | Variables | Redirect | Secret |
//! |---|---|---|---|
//! | macOS, Windows, Linux | `MAILCAL_GOOGLE_DESKTOP_CLIENT_ID`, `MAILCAL_GOOGLE_DESKTOP_CLIENT_SECRET` | loopback, per flow | required by Google's token endpoint, non-confidential |
//! | iOS, iPadOS | `MAILCAL_GOOGLE_IOS_CLIENT_ID` | reversed client id | none |
//! | Android | `MAILCAL_GOOGLE_ANDROID_CLIENT_ID` | reversed client id | none |
//!
//! Microsoft registers one app for every platform, so `MAILCAL_MS_CLIENT_ID` is read on all of
//! them, and so is `MAILCAL_ALLODIA_CLIENT_ID` for signing in to an Allodia account.
//! `MAILCAL_ALLODIA_HOST` points that sign-in at a service other than the production one, which is
//! how a development build reaches a local instance; it defaults to [`DEFAULT_ALLODIA_HOST`].
//!
//! None of these values is confidential. The client ids are public by construction, and the
//! Desktop client secret is one Google's own installed-app guidance says "is obviously not
//! treated as a secret": an installed binary cannot keep one, and it grants nothing without a
//! fresh PKCE verifier and the user's consent
//! (<https://developers.google.com/identity/protocols/oauth2#installed>). They are injected
//! rather than committed so that the tree can be published without carrying one project's
//! registrations, not because embedding them endangers anything.

/// The suffix every Google client id ends in.
const GOOGLE_CLIENT_ID_SUFFIX: &str = ".apps.googleusercontent.com";

/// Google's OAuth client registration for the platform this build targets, as
/// [`google`] resolved it.
#[derive(Debug, Clone)]
pub struct GoogleClient {
    /// The client id, of the form `<project-number>-<random>.apps.googleusercontent.com`.
    pub client_id: String,
    /// The Desktop client's non-confidential secret, which Google's token endpoint requires on
    /// both the code exchange and the refresh even under PKCE. `None` for the iOS and Android
    /// client types, which have no secret at all.
    pub client_secret: Option<String>,
    /// The redirect this client type is registered for, when it is fixed at build time: the
    /// reversed-client-id custom scheme on iOS and Android. `None` on desktop, where the host
    /// binds a loopback port and supplies `http://127.0.0.1:<port>/` per flow.
    pub redirect_uri: Option<String>,
}

/// Google's registration for this build, or `None` when it carries none.
#[must_use]
pub fn google() -> Option<GoogleClient> {
    #[cfg(target_os = "ios")]
    let (client_id, client_secret, mobile) =
        (option_env!("MAILCAL_GOOGLE_IOS_CLIENT_ID"), None, true);
    #[cfg(target_os = "android")]
    let (client_id, client_secret, mobile) =
        (option_env!("MAILCAL_GOOGLE_ANDROID_CLIENT_ID"), None, true);
    #[cfg(not(any(target_os = "ios", target_os = "android")))]
    let (client_id, client_secret, mobile) = (
        option_env!("MAILCAL_GOOGLE_DESKTOP_CLIENT_ID"),
        option_env!("MAILCAL_GOOGLE_DESKTOP_CLIENT_SECRET"),
        false,
    );

    let client_id = present(client_id)?;
    Some(GoogleClient {
        redirect_uri: if mobile {
            reversed_client_id_redirect(client_id)
        } else {
            None
        },
        client_id: client_id.to_owned(),
        client_secret: present(client_secret).map(str::to_owned),
    })
}

/// Microsoft's client id for this build, or `None` when it carries none.
#[must_use]
pub fn microsoft_client_id() -> Option<String> {
    present(option_env!("MAILCAL_MS_CLIENT_ID")).map(str::to_owned)
}

/// Allodia's own OAuth client registration for signing in to an Allodia account, or `None` when
/// this build carries none.
///
/// One registration for every platform, like Microsoft's: the account service is an OAuth 2.0
/// authorization server that discovers its own metadata (RFC 8414), so the redirect is negotiated
/// per flow rather than fixed at build time.
///
/// **A static client id rather than dynamic registration.** A first-party app has no reason to mint
/// a fresh client on every install, and a registered one can be revoked.
///
/// Absent is a supported build, and the same one a fork gets: the setup surfaces drop the Allodia
/// sign-in route entirely, which is what keeps "an unbranded build has no Allodia surface" true by
/// construction rather than by discipline.
#[must_use]
pub fn allodia_client_id() -> Option<String> {
    present(option_env!("MAILCAL_ALLODIA_CLIENT_ID")).map(str::to_owned)
}

/// The account service this build talks to, without a trailing slash.
///
/// Injected like the client id, and for the same reason: a development build points at a local
/// instance while a shipped one points at Allodia's. It is a **build-time** value and there is no
/// runtime path to it: the sovereignty carve-out rests on a user being unable to redirect their
/// own account traffic, not on the address being a literal in this file.
///
/// Defaults to the production service, so a build that injects only the client id is still correct.
#[must_use]
pub fn allodia_host() -> String {
    present(option_env!("MAILCAL_ALLODIA_HOST"))
        .unwrap_or(DEFAULT_ALLODIA_HOST)
        .trim_end_matches('/')
        .to_owned()
}

/// Where an Allodia account lives unless a build says otherwise.
pub const DEFAULT_ALLODIA_HOST: &str = "https://mailcal.allodia.eu";

/// A variable that was set to something, trimmed. Unset and blank are the same answer.
fn present(value: Option<&'static str>) -> Option<&'static str> {
    value.map(str::trim).filter(|value| !value.is_empty())
}

/// The custom-scheme redirect a Google **iOS or Android** client is registered for: the whole
/// client id with its dotted components reversed, plus an arbitrary-but-required path.
///
/// The project-number prefix is part of the scheme. Dropping it produces a scheme that looks
/// right and fails the authorization request with `redirect_uri_mismatch`, because Google matches
/// a mobile redirect on the scheme alone.
///
/// Returns `None` for an id that is not a Google client id, so a mistyped variable disables the
/// route rather than registering a scheme no browser will ever hand back.
fn reversed_client_id_redirect(client_id: &str) -> Option<String> {
    let suffix = client_id.strip_suffix(GOOGLE_CLIENT_ID_SUFFIX)?;
    (!suffix.is_empty()).then(|| format!("com.googleusercontent.apps.{suffix}:/oauth2redirect"))
}

#[cfg(test)]
mod tests {
    use super::{present, reversed_client_id_redirect};

    #[test]
    fn blank_and_unset_are_the_same_answer() {
        assert_eq!(present(None), None);
        assert_eq!(present(Some("")), None);
        assert_eq!(present(Some("   ")), None);
        assert_eq!(present(Some(" abc ")), Some("abc"));
    }

    #[test]
    fn mobile_redirect_reverses_the_whole_client_id() {
        assert_eq!(
            reversed_client_id_redirect("1234567890-abcdef.apps.googleusercontent.com").as_deref(),
            Some("com.googleusercontent.apps.1234567890-abcdef:/oauth2redirect"),
        );
    }

    #[test]
    fn an_id_that_is_not_googles_has_no_mobile_redirect() {
        assert_eq!(reversed_client_id_redirect("1234567890-abcdef"), None);
        assert_eq!(
            reversed_client_id_redirect(".apps.googleusercontent.com"),
            None
        );
    }
}
