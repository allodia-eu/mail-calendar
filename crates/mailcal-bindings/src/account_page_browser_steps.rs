//! What a browser sign-in signs in, begins with and is completed with.

use crate::{AccountCapability, UnreachedUses};

/// What a sign-in signs in.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Target {
    /// The account itself.
    Account,
    /// The server holding `uses` of the mailbox, as an account of their own
    /// (`docs/accounts.md` rule 18).
    Separate { uses: Vec<AccountCapability> },
    /// The Allodia account, at the service's sign-up page when `create`.
    Allodia { create: bool },
}

/// The handle a started sign-in is completed with.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Pending {
    /// A Microsoft or Google account's consent.
    Consent(String),
    /// A standards or JMAP account's sign-in at its provider.
    Account(String),
    /// A separate server's sign-in at its own provider.
    Separate(String),
    /// A Nextcloud Login Flow, polled by the core.
    Nextcloud(String),
    /// An Allodia account's sign-in.
    Allodia(String),
}

/// What a sign-in's server answered when it was started.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Began {
    pub(crate) pending: Pending,
    pub(crate) url: String,
    /// Where the browser returns; `None` for a Nextcloud sign-in, which returns nothing.
    pub(crate) redirect_uri: Option<String>,
    /// The uses a switch to the provider would cost (`docs/accounts.md` rule 18).
    pub(crate) unreached: Vec<AccountCapability>,
}

/// What a completion is given.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Completion {
    Consent {
        pending: String,
        callback_url: String,
    },
    Account {
        pending: String,
        callback_url: String,
        unreached: Option<UnreachedUses>,
    },
    Separate {
        pending: String,
        callback_url: String,
    },
    Nextcloud {
        grant: String,
    },
    Allodia {
        pending: String,
        callback_url: String,
    },
}

/// A completion taken to run, with what its outcome is said with.
#[derive(Debug)]
pub(crate) struct Run {
    pub(crate) account: String,
    pub(crate) target: Target,
    pub(crate) switching: bool,
    pub(crate) completion: Completion,
}
