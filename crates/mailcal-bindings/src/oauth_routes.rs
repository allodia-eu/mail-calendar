//! Which browser sign-in routes this build can offer, over the FFI.
//!
//! Google's and Microsoft's OAuth client registrations are injected at compile time
//! ([`mailcal_oauth::credentials`]), so whether a route exists is a property of the binary, not
//! of the account being added. A client asks once, at setup, and shows only the routes it is
//! told about: a build given no registration is a working IMAP / JMAP / CalDAV / CardDAV client,
//! never one with a sign-in button that fails at the provider.

/// The browser sign-in routes this build carries a registration for, and what the host's browser
/// half needs to drive them.
#[derive(uniffi::Record)]
pub struct OAuthRoutes {
    /// Whether the setup wizard may offer the Google (Gmail + Google Calendar) sign-in.
    pub google: bool,
    /// The redirect Google registered for this build's client type, when it is fixed at build
    /// time: the reversed-client-id custom scheme the iOS and Android hosts open the browser
    /// against and capture. `None` on macOS, Windows and Linux, whose Desktop client redirects
    /// to a loopback port the host picks per flow, and `None` whenever `google` is false.
    pub google_redirect_uri: Option<String>,
    /// Whether the setup wizard may offer the Microsoft 365 sign-in. Microsoft's redirect is
    /// registered per platform against the host's own bundle/package identity, so unlike
    /// Google's it stays with the client.
    pub microsoft: bool,
}

/// The routes this build can offer. Cheap and constant: a client may call it per screen.
#[must_use]
#[uniffi::export]
pub fn oauth_routes() -> OAuthRoutes {
    let google = mailcal_oauth::credentials::google();
    OAuthRoutes {
        google_redirect_uri: google.as_ref().and_then(|g| g.redirect_uri.clone()),
        google: google.is_some(),
        microsoft: mailcal_oauth::credentials::microsoft_client_id().is_some(),
    }
}

#[cfg(test)]
mod tests {
    use super::oauth_routes;

    /// Whichever registrations this build was given, the two halves of the Google answer agree:
    /// a redirect is never offered for a route that is not.
    #[test]
    fn a_redirect_is_never_offered_without_its_route() {
        let routes = oauth_routes();
        assert!(routes.google || routes.google_redirect_uri.is_none());
    }
}
