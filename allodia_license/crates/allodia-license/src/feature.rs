// SPDX-FileCopyrightText: 2026 Allodia
// SPDX-License-Identifier: LicenseRef-Allodia-1.0

//! What the app does that a permission has to allow, and which permission that is.
//!
//! Its own file so that adding one is a single, obvious edit: the scope goes in
//! [`SCOPES`](crate::SCOPES), the feature goes here, and every prompt that needs it follows
//! without a client learning a new word.

/// A thing this app does that a scope has to permit, and the scope it needs.
///
/// The point of naming them is that **adding a scope stays one line**. A grant issued before a
/// scope existed keeps working for everything else (a refresh asks for no scope, so it is not
/// refused), and the feature that needs the new one asks here and finds it missing, which is what
/// raises the prompt. Hand-coding "is `mailcal:accounts:read` missing" at the one call site that
/// needs it today is how the next scope arrives with no prompt behind it.
///
/// Only the ones that gate a *feature* belong here. The four in `REQUIRED_SCOPES` gate the sign-in
/// itself: without them there is no grant to inspect.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Feature {
    /// Reading which plan the account is on.
    Entitlement,
    /// Reading the account list the person's other devices published.
    ReadAccounts,
    /// Publishing this device's accounts to the others.
    WriteAccounts,
    /// Reading what the account is subscribed to, for the account screen.
    ReadSubscription,
    /// Changing the subscription: attaching a store purchase, starting a checkout, cancelling.
    WriteSubscription,
    /// Learning a writing style and drafting replies through Allodia's relay.
    UseAi,
    /// Reading the writing styles the person's other devices synced.
    ReadWritingStyles,
    /// Syncing this device's writing styles to the others.
    WriteWritingStyles,
}

impl Feature {
    /// The features whose absence is worth telling somebody about **before** they ask for one.
    ///
    /// These are what the app does on its own: reading the plan, syncing the account list. A grant
    /// short of one of them is a grant that will quietly fail at something nobody asked for, so it
    /// is worth a standing offer to sign in again.
    ///
    /// ⚠️ The two subscription features are deliberately **not** here. Opening the account screen
    /// and buying something are things a person does deliberately, so the prompt belongs on that
    /// screen rather than on everybody's account card. A grant short of them is also the ordinary
    /// state of every grant issued before those scopes existed, and telling all of those people to
    /// sign in again would be a prompt about a screen most of them will never open. The three AI
    /// features are left out for the same reason: their prompt belongs on the Writing style
    /// screen.
    pub const ALL: &'static [Self] = &[Self::Entitlement, Self::ReadAccounts, Self::WriteAccounts];

    /// The scope that permits it.
    #[must_use]
    pub const fn scope(self) -> &'static str {
        match self {
            Self::Entitlement => "mailcal:entitlement:read",
            Self::ReadAccounts => "mailcal:accounts:read",
            Self::WriteAccounts => "mailcal:accounts:write",
            Self::ReadSubscription => "mailcal:subscription:read",
            Self::WriteSubscription => "mailcal:subscription:write",
            Self::UseAi => "mailcal:ai:use",
            Self::ReadWritingStyles => "mailcal:writing-styles:read",
            Self::WriteWritingStyles => "mailcal:writing-styles:write",
        }
    }
}

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
    "mailcal:ai:use",
    "mailcal:writing-styles:read",
    "mailcal:writing-styles:write",
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
