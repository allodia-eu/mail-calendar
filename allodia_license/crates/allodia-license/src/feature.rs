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
    /// sign in again would be a prompt about a screen most of them will never open.
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
        }
    }
}
