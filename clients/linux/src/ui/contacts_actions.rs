//! Contacts actions on the top-level Relm4 model; the half that crosses the core boundary.

use std::sync::Arc;

use mailcal_bindings::{ContactDetail, Intent};

use super::{AppInput, AppModel, PrimaryView};

impl AppModel {
    /// Enters the contacts surface: drop any stale narrowing, paint what is already cached, then
    /// sync the address books.
    ///
    /// The query is cleared in the **core** as well as in the model, as one action. The search box
    /// is view state that dies with the view, but the query lives in the core; so without this,
    /// leaving contacts mid-search and coming back shows a filtered list under an empty search
    /// box: a narrowing the user can no longer see.
    ///
    /// The cached list is painted before the sync so that switching surface never shows an empty
    /// screen while the address books are consulted.
    pub(super) fn show_contacts(&mut self) {
        self.primary = PrimaryView::Contacts;
        self.contacts.entered();
        self.dispatch(Intent::SearchContacts {
            query: String::new(),
        });
        if let Some(app) = &self.app {
            self.contacts.refresh(app);
        }
        self.dispatch(Intent::RefreshContacts);
    }

    /// Narrows the list. The matching runs in the core, which answers with `Surface::Contacts`.
    pub(super) fn search_contacts(&mut self, query: String) {
        self.contacts.set_query(query.clone());
        self.dispatch(Intent::SearchContacts { query });
    }

    /// Opens one person's detail, off the UI thread.
    ///
    /// `contact_detail` is network-free but blocks on the core's runtime and lands on the store's
    /// connection thread, so a call made while a sync holds that connection waits for it; which
    /// on the GLib loop is a frozen window.
    pub(super) fn open_contact(&mut self, id: String, sender: relm4::Sender<AppInput>) {
        let Some(app) = &self.app else {
            return;
        };
        let app = Arc::clone(app);
        let lookup = self.contacts.begin_lookup();
        std::thread::spawn(move || {
            let detail = app.contact_detail(id);
            sender.emit(AppInput::ContactOpened(lookup, detail.map(Box::new)));
        });
    }

    pub(super) fn contact_opened(&mut self, lookup: u64, detail: Option<&ContactDetail>) {
        self.contacts
            .finish_lookup(lookup, detail, &self.snapshot.accounts);
    }
}
