//! What an Allodia sign-in asks for: the scopes, split into the ones a sign-in cannot do without
//! and the ones that gate a feature and are asked for only where the service advertises them.

/// What sign-in asks for, and every one is a scope the service advertises.
///
/// `openid`, `profile` and `email` identify the person, so a client can say which account is signed
/// in. **`offline_access` is the load-bearing one**: without it the service issues no refresh
/// token, and the sign-in silently becomes a session that expires with no way back, which is the
/// whole problem OAuth was chosen to solve here.
///
/// `mailcal:entitlement:read` is what the entitlement endpoint requires, and it is the narrowest
/// thing this app needs: permission to read which plan an account is on, and nothing else. Nothing
/// here reaches mail: an Allodia account and a mail account are different things, and a token
/// issued for this app cannot touch the second.
pub const SCOPES: &[&str] = &[
    "openid",
    "profile",
    "email",
    "offline_access",
    "mailcal:entitlement:read",
    "mailcal:accounts:read",
    "mailcal:accounts:write",
    "mailcal:subscription:read",
    "mailcal:subscription:write",
];

/// The ones a sign-in is not worth completing without, sent whether or not the service lists them.
///
/// `offline_access` is the load-bearing one and `openid`/`profile`/`email` are how the app learns
/// whose account it is. Filtering these against an incomplete `scopes_supported` would turn a
/// service that simply under-advertises into a sign-in that succeeds and then cannot say who
/// signed in, or one that expires within the hour with no way back.
const REQUIRED_SCOPES: &[&str] = &["openid", "profile", "email", "offline_access"];

/// What a build asks for **only** where the service says it accepts it.
///
/// These gate features rather than the sign-in itself, so a client that reaches a deployment
/// predating them should lose the feature and keep the sign-in. Asking for a scope a server has
/// not advertised is refused outright by enough of them that the alternative is a client which
/// cannot sign in at all until the server catches up.
fn optional_scopes(advertised: &[String]) -> Vec<String> {
    SCOPES
        .iter()
        .filter(|scope| !REQUIRED_SCOPES.contains(*scope))
        .filter(|scope| advertised.iter().any(|offered| offered == *scope))
        .map(|scope| (*scope).to_owned())
        .collect()
}

/// Everything to ask for, given what the service says it accepts.
pub(crate) fn scopes_for(advertised: &[String]) -> Vec<String> {
    let mut scopes: Vec<String> = REQUIRED_SCOPES.iter().map(|s| (*s).to_owned()).collect();
    scopes.extend(optional_scopes(advertised));
    scopes
}
