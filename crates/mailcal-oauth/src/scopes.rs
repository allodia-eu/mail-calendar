//! Which scopes each use of an account asks for, at the two integrated providers.
//!
//! An account is used for some of mail, calendar, contacts and colleagues, and a sign-in asks for
//! the scopes of those uses only, so a person who wants a calendar is not asked for their mail and
//! an organisation that approved only the mail scopes can still admit the app. The per-scope
//! justification an administrator reads is the table in
//! [`docs/provider-oauth.md`](../../../docs/provider-oauth.md); [`MICROSOFT_GRAPH_SCOPES`] and
//! [`GOOGLE_SCOPES`] carry the reasoning for each scope, and are the union of the groups here.
//!
//! [`MICROSOFT_GRAPH_SCOPES`]: crate::MICROSOFT_GRAPH_SCOPES
//! [`GOOGLE_SCOPES`]: crate::GOOGLE_SCOPES

/// The scopes one use of an account asks for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ScopeGroup {
    /// Everything requested for this use.
    pub requested: &'static [&'static str],
    /// The one scope without which the use cannot open at all. The others serve a feature within
    /// it (sending, a sender name), and a grant without one of them still opens the use and lets
    /// that feature be refused when it is reached.
    pub needed: &'static str,
}

/// One provider's scopes, grouped by use.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProviderScopes {
    /// Requested whatever the account is used for.
    pub always: &'static [&'static str],
    /// Reading and sending mail.
    pub mail: ScopeGroup,
    /// The account's calendar.
    pub calendar: ScopeGroup,
    /// The account's own contacts.
    pub contacts: ScopeGroup,
    /// The organisation's directory, requested only beside contacts.
    pub colleagues: ScopeGroup,
    /// Requested only when mail is not, because mail's scope is otherwise what the account's own
    /// address is read with.
    pub address_without_mail: &'static [&'static str],
    /// The resource a scope may be granted under without being named, and which a token response
    /// may name it without: Microsoft answers `Mail.ReadWrite` for a request of
    /// `https://graph.microsoft.com/Mail.ReadWrite`, in whatever case it chooses.
    implied_resource: Option<&'static str>,
}

/// Microsoft Graph, delegated.
pub const MICROSOFT: ProviderScopes = ProviderScopes {
    always: &["offline_access", "https://graph.microsoft.com/User.Read"],
    mail: ScopeGroup {
        requested: &[
            "https://graph.microsoft.com/Mail.ReadWrite",
            "https://graph.microsoft.com/Mail.Send",
        ],
        needed: "https://graph.microsoft.com/Mail.ReadWrite",
    },
    calendar: ScopeGroup {
        requested: &["https://graph.microsoft.com/Calendars.ReadWrite"],
        needed: "https://graph.microsoft.com/Calendars.ReadWrite",
    },
    contacts: ScopeGroup {
        requested: &["https://graph.microsoft.com/Contacts.ReadWrite"],
        needed: "https://graph.microsoft.com/Contacts.ReadWrite",
    },
    colleagues: ScopeGroup {
        requested: &["https://graph.microsoft.com/User.ReadBasic.All"],
        needed: "https://graph.microsoft.com/User.ReadBasic.All",
    },
    // `User.Read` is always requested, and `GET /me` names the account with it.
    address_without_mail: &[],
    implied_resource: Some("https://graph.microsoft.com/"),
};

/// Google's APIs. A refresh token comes from request parameters, not a scope, so nothing is
/// requested always.
pub const GOOGLE: ProviderScopes = ProviderScopes {
    always: &[],
    mail: ScopeGroup {
        requested: &[
            "https://mail.google.com/",
            "https://www.googleapis.com/auth/gmail.settings.basic",
        ],
        needed: "https://mail.google.com/",
    },
    calendar: ScopeGroup {
        requested: &["https://www.googleapis.com/auth/calendar"],
        needed: "https://www.googleapis.com/auth/calendar",
    },
    contacts: ScopeGroup {
        requested: &[
            "https://www.googleapis.com/auth/contacts",
            "https://www.googleapis.com/auth/contacts.other.readonly",
        ],
        needed: "https://www.googleapis.com/auth/contacts",
    },
    colleagues: ScopeGroup {
        requested: &["https://www.googleapis.com/auth/directory.readonly"],
        needed: "https://www.googleapis.com/auth/directory.readonly",
    },
    // Non-sensitive, but it has to be listed on the Cloud project's consent screen.
    address_without_mail: &["https://www.googleapis.com/auth/userinfo.email"],
    implied_resource: None,
};

impl ProviderScopes {
    /// Whether `granted`, as a token response spelled it, holds `scope` as this table spells it.
    #[must_use]
    pub fn holds(&self, granted: &[String], scope: &str) -> bool {
        let wanted = self.canonical(scope);
        granted.iter().any(|held| self.canonical(held) == wanted)
    }

    fn canonical(&self, scope: &str) -> String {
        let scope = scope.trim().to_ascii_lowercase();
        match self.implied_resource {
            Some(resource) => scope
                .strip_prefix(resource)
                .map_or_else(|| scope.clone(), ToOwned::to_owned),
            None => scope,
        }
    }
}

#[cfg(test)]
#[path = "scopes_tests.rs"]
mod tests;
