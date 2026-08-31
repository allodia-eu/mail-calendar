//! The connectivity view-model: whether the device is offline, and which accounts can't
//! reach their mail server.
//!
//! Two distinct signals a host renders differently: a **device**
//! that has no network at all (a global "you're offline" banner: the app stops attempting
//! network calls until it returns), versus **one account** whose last sync couldn't reach
//! its server while the device *was* online (a per-account outage; expired credentials, a
//! provider down; badged next to that account in the switcher). They are mutually
//! exclusive: while offline the per-account list is empty, since the fault is the device's,
//! not any one account's.

/// An immutable snapshot of connectivity for a host to render.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ConnectivitySnapshot {
    /// The device has no network connectivity (per the host's OS reachability API). A host
    /// shows a global offline banner; the core suppresses network syncs until it clears.
    pub offline: bool,
    /// The ids of accounts whose most recent sync could not reach their mail server **while
    /// the device was online**: a per-account outage a host badges in the switcher. Always
    /// empty while [`offline`](Self::offline) is `true` (then it's the device, not the
    /// account, so no per-account warning is shown).
    pub unreachable_accounts: Vec<String>,
    /// The ids of accounts whose calendar is withheld because the OAuth grant lacks the
    /// calendar scope (a Graph `403`; connected before calendar support, or revoked consent).
    /// **Mail is unaffected**: this is not an outage; a host shows a "reconnect to enable
    /// calendar" prompt whose action re-runs the account's OAuth sign-in. Unlike
    /// [`unreachable_accounts`](Self::unreachable_accounts) this is a standing permission gap,
    /// so it is **not** suppressed while offline (the state is real regardless of connectivity).
    pub calendar_reauth_accounts: Vec<String>,
    /// The ids of accounts whose mail **write/send** is withheld because the OAuth grant lacks the
    /// `Mail.ReadWrite` / `Mail.Send` scopes (a Graph `403 ErrorAccessDenied`; connected before
    /// those scopes, or revoked consent). Mail **reading** is unaffected; a host shows a
    /// "reconnect to send and manage mail" prompt whose action re-runs the account's OAuth sign-in
    /// (which re-grants the whole scope set, so one re-consent clears this **and**
    /// [`calendar_reauth_accounts`](Self::calendar_reauth_accounts)). Raised at the point of use
    /// (a refused send or mail action), not at connect; mail reads fine on the read-only scope;
    /// and, like the calendar prompt, **not** suppressed while offline.
    pub mail_reauth_accounts: Vec<String>,
    /// The ids of accounts whose stored OAuth grant is **dead**; expired or revoked, so the
    /// refresh token no longer mints an access token (Google `invalid_grant`, a Microsoft
    /// `AADSTS700082`, an OAuth JMAP token withdrawn). Unlike the two lists above this is not a
    /// missing *scope* (**nothing** syncs until the user signs in again) and unlike
    /// [`unreachable_accounts`](Self::unreachable_accounts) it is not an outage: the server was
    /// reached and answered, and no amount of waiting or retrying fixes it. Hosts therefore show
    /// a "your sign-in expired; reconnect" prompt rather than an unreachable badge, and an
    /// account here is **excluded** from `unreachable_accounts` so the two never contradict each
    /// other. Like the reauth prompts it survives going offline, since re-consent is the remedy
    /// either way.
    pub signin_expired_accounts: Vec<String>,
}
