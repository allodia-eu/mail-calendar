//! PKCE (RFC 7636): the proof-key a public client (a desktop/mobile app that can
//! keep no client secret) sends so a stolen authorisation `code` is useless without
//! the matching verifier.
//!
//! We generate a high-entropy **code verifier** (32 random bytes → 43-char
//! base64url), derive its **code challenge** as `base64url(SHA-256(verifier))`, put
//! the challenge in the authorization request, and send the verifier only on the
//! back-channel token exchange. The `S256` method is the only one we use (never the
//! `plain` fallback). A separate [`random_state`] value covers CSRF on the redirect.

use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use ring::{
    digest,
    rand::{SecureRandom, SystemRandom},
};

/// A PKCE verifier/challenge pair for one authorization request.
///
/// The `verifier` is secret (it stands in for a client secret), so it is redacted in
/// `Debug`; the `challenge` travels in the (front-channel) authorization URL and is
/// not sensitive.
#[derive(Clone)]
pub struct Pkce {
    verifier: String,
    challenge: String,
}

impl Pkce {
    /// Generates a fresh pair from the system CSPRNG: a 43-character base64url
    /// verifier (32 random bytes, within RFC 7636's 43–128 range) and its `S256`
    /// challenge.
    #[must_use]
    pub fn generate() -> Self {
        let mut bytes = [0u8; 32];
        SystemRandom::new()
            .fill(&mut bytes)
            .expect("system CSPRNG fills 32 bytes");
        Self::from_verifier(URL_SAFE_NO_PAD.encode(bytes))
    }

    /// Rebuilds the pair from a stored `verifier`, recomputing the `S256` challenge;
    /// so a host that persisted only the verifier between the "begin" and "complete"
    /// steps of the flow can reconstruct it.
    #[must_use]
    pub fn from_verifier(verifier: String) -> Self {
        let digest = digest::digest(&digest::SHA256, verifier.as_bytes());
        let challenge = URL_SAFE_NO_PAD.encode(digest.as_ref());
        Self {
            verifier,
            challenge,
        }
    }

    /// The secret code verifier, sent only on the back-channel token exchange.
    #[must_use]
    pub fn verifier(&self) -> &str {
        &self.verifier
    }

    /// The `S256` code challenge, placed in the authorization request URL.
    #[must_use]
    pub fn challenge(&self) -> &str {
        &self.challenge
    }
}

impl core::fmt::Debug for Pkce {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        // Never leak the verifier; the challenge is not sensitive but is noise in logs.
        f.debug_struct("Pkce")
            .field("verifier", &"<redacted>")
            .finish_non_exhaustive()
    }
}

/// A fresh, unguessable `state` value (16 random bytes → base64url) binding one
/// authorization request to its redirect, so a callback that doesn't echo it back is
/// rejected as forged (CSRF).
#[must_use]
pub fn random_state() -> String {
    let mut bytes = [0u8; 16];
    SystemRandom::new()
        .fill(&mut bytes)
        .expect("system CSPRNG fills 16 bytes");
    URL_SAFE_NO_PAD.encode(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn verifier_is_a_valid_length_and_challenge_is_the_s256_of_it() {
        let pkce = Pkce::generate();
        // 32 bytes base64url-no-pad is 43 chars; inside RFC 7636's 43..=128.
        assert_eq!(pkce.verifier().len(), 43);
        assert!((43..=128).contains(&pkce.verifier().len()));

        // The challenge is base64url(SHA-256(verifier)); recompute independently.
        let expected = URL_SAFE_NO_PAD
            .encode(digest::digest(&digest::SHA256, pkce.verifier().as_bytes()).as_ref());
        assert_eq!(pkce.challenge(), expected);
        // A 256-bit digest base64url-no-pad is 43 chars.
        assert_eq!(pkce.challenge().len(), 43);
    }

    #[test]
    fn from_verifier_recomputes_the_same_challenge() {
        let original = Pkce::generate();
        let rebuilt = Pkce::from_verifier(original.verifier().to_owned());
        assert_eq!(original.challenge(), rebuilt.challenge());
    }

    #[test]
    fn generate_is_unique_per_call() {
        // Two calls must not collide (the whole point of a CSPRNG verifier).
        assert_ne!(Pkce::generate().verifier(), Pkce::generate().verifier());
        assert_ne!(random_state(), random_state());
    }

    #[test]
    fn debug_never_leaks_the_verifier() {
        let pkce = Pkce::generate();
        assert!(!format!("{pkce:?}").contains(pkce.verifier()));
    }

    #[test]
    fn base64url_is_url_safe_and_unpadded() {
        // No `+`, `/`, or `=`; safe to drop straight into a URL query value.
        let state = random_state();
        assert!(!state.contains(['+', '/', '=']));
    }
}
