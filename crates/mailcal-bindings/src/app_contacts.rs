//! The contacts and recipient-autosuggest methods on [`MailcalApp`], and the view-model → FFI
//! conversions.
//!
//! [`MailcalApp::contact_list`] is the ordinary snapshot pull: dispatch
//! [`Intent::RefreshContacts`](crate::Intent::RefreshContacts) or
//! [`Intent::SearchContacts`](crate::Intent::SearchContacts), wait for `Surface::Contacts`, read
//! the slot. The other two are **direct queries**: a detail open and a per-keystroke
//! autosuggest are both answers to a question the user just asked, and routing them through the
//! snapshot slot would mean a race between two keystrokes deciding which answer survives.
//!
//! All three are **network-free**; they read the already-derived people index rather than
//! syncing anything, but network-free is not free. Each blocks the calling thread on the
//! runtime and lands on the store's connection thread, so a call made while a sync holds that
//! connection waits for it. A host must therefore keep these **off its UI thread**, and
//! anything per-keystroke wants a debounce as well.

use mailcal_app::RecipientMatch as AppRecipientMatch;
use mailcal_viewmodel::{
    ContactDetail as AppContactDetail, ContactRow as AppContactRow, ContactValue as AppContactValue,
};

use crate::{
    ContactDetail, ContactRow, ContactValue, ContactsSnapshot, MailcalApp, RecipientMatch,
};

#[uniffi::export]
impl MailcalApp {
    /// The current contacts snapshot; one row per unified person, ordered A–Z.
    ///
    /// Reflects the active search query, so a client re-reads this after `Surface::Contacts`
    /// without tracking whether the change came from a sync or from typing in the search field.
    pub fn contact_list(&self) -> ContactsSnapshot {
        ContactsSnapshot {
            rows: self
                .app
                .contacts()
                .rows
                .into_iter()
                .map(Into::into)
                .collect(),
        }
    }

    /// The detail of one person, by the id a [`ContactRow`] carries.
    ///
    /// Returns `None` when the person no longer exists. A row held across a merge still
    /// resolves (the engine keeps retired ids pointing at the surviving person) so a client
    /// need not refresh its list before opening a row it already has.
    pub fn contact_detail(&self, id: String) -> Option<ContactDetail> {
        self.runtime
            .block_on(self.app.contact_detail(&id))
            .map(Into::into)
    }

    /// Ranked recipient suggestions for a partially-typed address in To/Cc/Bcc.
    ///
    /// Empty for a blank query. No network, but it is three store reads (people, interaction
    /// history, coverage), so call it off the UI thread and debounce it rather than firing one
    /// per keystroke. Results include people known only from **sent mail**, so this is useful
    /// on an account with no address book at all.
    pub fn recipient_suggestions(&self, query: String) -> Vec<RecipientMatch> {
        self.runtime
            .block_on(self.app.recipient_suggestions(&query))
            .into_iter()
            .map(Into::into)
            .collect()
    }
}

impl From<AppContactRow> for ContactRow {
    fn from(row: AppContactRow) -> Self {
        Self {
            id: row.id,
            display_name: row.display_name,
            primary_email: row.primary_email,
            section: row.section,
            avatar: row.avatar.into(),
            account_count: row.account_count,
        }
    }
}

impl From<AppContactValue> for ContactValue {
    fn from(value: AppContactValue) -> Self {
        Self {
            value: value.value,
            accounts: value.accounts,
        }
    }
}

impl From<AppContactDetail> for ContactDetail {
    fn from(detail: AppContactDetail) -> Self {
        Self {
            id: detail.id,
            display_name: detail.display_name,
            avatar: detail.avatar.into(),
            emails: detail.emails.into_iter().map(Into::into).collect(),
            phones: detail.phones.into_iter().map(Into::into).collect(),
            organizations: detail.organizations.into_iter().map(Into::into).collect(),
            titles: detail.titles.into_iter().map(Into::into).collect(),
            accounts: detail.accounts,
        }
    }
}

impl From<AppRecipientMatch> for RecipientMatch {
    fn from(found: AppRecipientMatch) -> Self {
        Self {
            email: found.email,
            display_name: found.display_name,
            is_saved: found.is_saved,
        }
    }
}
