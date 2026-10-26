//! Moving a standards account's calendar and contacts to an account of their own: what a mail
//! sign-in that cannot reach their server leaves them as (`docs/accounts.md` rule 18).
//!
//! An account is one sign-in (rule 1), so a calendar server that takes a password while the
//! mailbox signs in through its provider is a second account, linked to the first.

use mailcal_oauth::DavUse;

use crate::{AccountConfig, Capabilities, Capability, Secret, config_dav::grant_reaches};

impl AccountConfig {
    /// The calendar and contacts uses this account is used for that its grant does not reach:
    /// empty on an account that signs in with a password, which reaches whatever takes it.
    #[must_use]
    pub fn uses_its_grant_does_not_reach(&self) -> Vec<Capability> {
        let Some(grant) = &self.oauth else {
            return Vec::new();
        };
        let chosen = self.capabilities();
        [
            (
                Capability::Calendar,
                self.caldav_endpoint(),
                DavUse::Calendars,
            ),
            (
                Capability::Contacts,
                self.carddav_endpoint(),
                DavUse::Contacts,
            ),
        ]
        .into_iter()
        .filter(|(capability, endpoint, dav_use)| {
            chosen.contains(*capability)
                && endpoint
                    .is_some_and(|endpoint| !grant_reaches(grant, endpoint.base_url, *dav_use))
        })
        .map(|(capability, ..)| capability)
        .collect()
    }

    /// An account used for `uses` alone, at this account's servers for them, signing in as each
    /// server's stored login does: what [`without_uses`](Self::without_uses) moves them to.
    /// `None` when `uses` names neither calendar nor contacts, or a server this account lacks.
    ///
    /// It carries no grant (it is the password half of a split) and no link; the caller links
    /// the two.
    #[must_use]
    pub fn calendar_account_for(&self, uses: &[Capability]) -> Option<AccountConfig> {
        let calendar = uses.contains(&Capability::Calendar);
        let contacts = uses.contains(&Capability::Contacts);
        if !calendar && !contacts {
            return None;
        }
        let caldav = if calendar {
            Some(self.caldav.clone()?)
        } else {
            None
        };
        // The address book's own server, or the calendar's when it shares it: kept as its own
        // section when the calendar does not move with it.
        let carddav = if contacts {
            let endpoint = self.carddav_endpoint()?;
            (caldav.is_none() || self.carddav.is_some()).then(|| crate::CardDavAccount {
                base_url: endpoint.base_url.to_owned(),
                username: endpoint.username.to_owned(),
                password: endpoint.password.cloned(),
            })
        } else {
            None
        };
        let mut shape = crate::AccountShape {
            capabilities: Some(uses.iter().copied().collect::<Capabilities>()),
            ..crate::AccountShape::default()
        };
        shape.links.mail = self.account_id().ok();
        Some(AccountConfig {
            imap: None,
            smtp: None,
            caldav,
            carddav,
            oauth: None,
            certificate_exceptions: self.certificate_exceptions.clone(),
            shape,
            // An account of its own learns its principals on its first connect.
            dav_starts: crate::DavStarts::default(),
        })
    }

    /// This account signing in to its calendar and contacts servers as `username` with
    /// `password`: the login a person typed for the separate server.
    #[must_use]
    pub fn with_dav_login(&self, username: &str, password: &str) -> Self {
        let mut updated = self.clone();
        let secret = Secret::new(password.to_owned());
        if let Some(caldav) = &mut updated.caldav {
            username.clone_into(&mut caldav.username);
            caldav.password = Some(secret.clone());
        }
        if let Some(carddav) = &mut updated.carddav {
            username.clone_into(&mut carddav.username);
            carddav.password = Some(secret);
        }
        updated
    }

    /// This account signing in to its calendar and contacts servers with `grant` rather than a
    /// password: what a sign-in at the separate server's own provider leaves it as.
    #[must_use]
    pub fn with_dav_grant(&self, grant: crate::OAuthGrant) -> Self {
        let mut updated = self.clone();
        if let Some(caldav) = &mut updated.caldav {
            caldav.password = None;
        }
        if let Some(carddav) = &mut updated.carddav {
            carddav.password = None;
        }
        updated.oauth = Some(grant);
        updated
    }

    /// This account no longer used for `uses`, and without the servers only they used, linked
    /// to `moved_to` for each of them when they moved there: the mail half of a split.
    #[must_use]
    pub fn without_uses(
        &self,
        uses: &[Capability],
        moved_to: Option<&engine_core::ids::AccountId>,
    ) -> Self {
        let mut updated = self.clone();
        let calendar = uses.contains(&Capability::Calendar);
        let contacts = uses.contains(&Capability::Contacts);
        let kept: Capabilities = self
            .capabilities()
            .iter()
            .filter(|capability| !uses.contains(capability))
            .collect();
        match (calendar, contacts) {
            (true, true) => {
                updated.caldav = None;
                updated.carddav = None;
            }
            // Contacts found at the calendar's server keep that server as their own.
            (true, false) => {
                if updated.carddav.is_none()
                    && let Some(caldav) = &updated.caldav
                {
                    updated.carddav = Some(crate::CardDavAccount {
                        base_url: caldav.base_url.clone(),
                        username: caldav.username.clone(),
                        password: caldav.password.clone(),
                    });
                }
                updated.caldav = None;
            }
            (false, true) => updated.carddav = None,
            (false, false) => {}
        }
        updated.shape.capabilities = Some(kept);
        if let Some(moved_to) = moved_to {
            if calendar {
                updated.shape.links.calendar = Some(moved_to.clone());
            }
            if contacts {
                updated.shape.links.contacts = Some(moved_to.clone());
            }
        }
        updated
    }
}

#[cfg(test)]
#[path = "config_split_tests.rs"]
mod tests;
