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
}

impl Feature {
    /// Every feature a grant can be short of, so a caller can ask about all of them at once
    /// rather than remembering the list.
    pub const ALL: &'static [Self] = &[Self::Entitlement, Self::ReadAccounts, Self::WriteAccounts];

    /// The scope that permits it.
    #[must_use]
    pub const fn scope(self) -> &'static str {
        match self {
            Self::Entitlement => "mailcal:entitlement:read",
            Self::ReadAccounts => "mailcal:accounts:read",
            Self::WriteAccounts => "mailcal:accounts:write",
        }
    }
}
