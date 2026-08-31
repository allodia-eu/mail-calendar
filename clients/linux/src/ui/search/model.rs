//! Pure mail-search state for the Linux client: what is typed, how it is narrowed, and the two
//! sentences the results surface states about them.
//!
//! The search itself is the **core's**; the query, the scope semantics and the newest-first
//! ordering all live there (`docs/search.md`), and nothing here matches, filters or sorts. What
//! this owns is the one invariant a client can break on its own: **what the filter shows is what
//! the core is applying**. The core drops the scope whenever the query clears, so every
//! transition here that empties the query resets the toggle in the same move. Otherwise the field
//! empties, the core widens back to all mail, and the filter goes on claiming the search is
//! narrowed to one folder.

use mailcal_bindings::{MailboxListSnapshot, SearchHorizon, SearchScope};

use super::super::folder_pane;
use crate::l10n;

/// What a change in the field means for the core.
///
/// Three outcomes rather than two, because "nothing to ask for" is a real one: Escape reports an
/// empty field whether or not a search was running, and leaving one that is not running still
/// costs a full snapshot rebuild; which resets how far down the list the user had scrolled.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum QueryChange {
    /// Run this query.
    Run(String),
    /// Leave search, which resets the scope with it.
    Leave,
}

/// The search chrome's state: the query behind the field, and the scope the filter is showing.
///
/// Deliberately not the results; those arrive in the mailbox snapshot like any other list, so
/// there is no second copy of the rows to fall out of step with the core's.
#[derive(Debug)]
pub(crate) struct SearchState {
    query: String,
    scope: SearchScope,
}

impl Default for SearchState {
    fn default() -> Self {
        Self {
            query: String::new(),
            scope: SearchScope::AllFolders,
        }
    }
}

impl SearchState {
    pub(crate) fn scope(&self) -> SearchScope {
        self.scope
    }

    /// Whether a search is running; what the whole search chrome is revealed by.
    ///
    /// Blank, not empty: the core reads a whitespace-only query as no search at all, so a client
    /// calling that active would draw a scope filter and a horizon line over an ordinary folder.
    pub(crate) fn is_active(&self) -> bool {
        !self.query.trim().is_empty()
    }

    /// Records what the field now holds, and answers with what to ask the core for.
    ///
    /// Leaving resets the scope here because the core resets it there, as one action: a narrowing
    /// the user can no longer see is a narrowing they will not think of, and the next search would
    /// silently be smaller than it looks (`docs/search.md`, rule 6).
    pub(crate) fn set_query(&mut self, query: String) -> Option<QueryChange> {
        let was_active = self.is_active();
        self.query = query;
        if self.is_active() {
            return Some(QueryChange::Run(self.query.clone()));
        }
        self.scope = SearchScope::AllFolders;
        was_active.then_some(QueryChange::Leave)
    }

    pub(crate) fn set_scope(&mut self, scope: SearchScope) {
        self.scope = scope;
    }
}

/// What the narrowing half of the filter covers: the mailbox list as it stands, named the way the
/// pane beside it names the same thing (`docs/search.md`, rule 4).
///
/// The core keeps the selected account and folder on a search snapshot precisely so this can be
/// read off it; the filter names the view the search was opened from, not the results.
pub(crate) fn scope_label(snapshot: &MailboxListSnapshot) -> String {
    let Some(account) = snapshot.selected_account.as_deref() else {
        // The unified view: "this scope" is every account's Inbox, not any one folder.
        return l10n::search_scope_inboxes().to_owned();
    };
    let Some(key) = snapshot.selected.as_deref() else {
        return l10n::search_scope_account().to_owned();
    };
    folder_pane::folders_of(snapshot, account)
        .iter()
        .find(|folder| folder.key == key)
        .map_or_else(
            // A key with no row behind it: the folder list has moved on under the search, and the
            // filter would otherwise offer to narrow to a folder that is no longer there.
            || l10n::search_scope_folder().to_owned(),
            |folder| folder_pane::folder_label(folder.role.as_ref(), &folder.name),
        )
}

/// How far back the results reach, or `None` for a list nobody searched.
///
/// The core sends a depth and never words, so the sentence is assembled here like every other
/// string. Saying it is the whole point of the line: search reads this device and nothing else, so
/// an unqualified empty result claims "there is no such message" when it means "not in the last
/// three months": and only the second is something the user can fix.
pub(crate) fn horizon_label(horizon: Option<&SearchHorizon>) -> Option<String> {
    Some(match horizon? {
        SearchHorizon::AllTime => l10n::search_horizon_all().to_owned(),
        SearchHorizon::Months { months } => l10n::search_horizon_months(i64::from(*months)),
    })
}

#[cfg(test)]
#[path = "model_tests.rs"]
mod tests;
