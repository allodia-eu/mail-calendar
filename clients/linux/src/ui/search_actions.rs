//! Mail-search actions on the top-level Relm4 model; the half that crosses the core boundary.

use mailcal_bindings::{Intent, SearchScope};

use super::{AppModel, search::QueryChange, settings::Category};

impl AppModel {
    /// Searches for what is in the field, or leaves search when it is empty.
    ///
    /// The query and the scope both live in the **core**, which answers with a mailbox snapshot
    /// like any other list; so there is nothing to project here, and leaving search restores the
    /// account and folder the user opened it from without this client remembering either.
    pub(super) fn search_mail(&mut self, query: String) {
        let query = match self.search.set_query(query) {
            Some(QueryChange::Run(query)) => Some(query),
            Some(QueryChange::Leave) => None,
            None => return,
        };
        self.dispatch(Intent::Search { query });
    }

    /// Narrows the running search, or widens it back. Independent of the query, so moving the
    /// filter re-projects the results without retyping.
    pub(super) fn set_search_scope(&mut self, scope: SearchScope) {
        self.search.set_scope(scope);
        self.dispatch(Intent::SetSearchScope { scope });
    }

    /// Opens Settings where the sync depth is, the route the horizon line carries: search finds
    /// only what depth kept, so the fact and the way to change it belong together.
    pub(super) fn open_sync_depth_settings(&mut self) {
        self.settings.open(Some(Category::Accounts));
    }
}
