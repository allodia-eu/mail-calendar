//! Contacts actions on the top-level Relm4 model; the half that crosses the core boundary.

use std::sync::Arc;

use mailcal_bindings::{ContactDetail, ContactEdit, ContactTarget, Intent};

use super::{AppInput, AppModel, PrimaryView, contacts::EditTarget};

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
    pub(super) fn show_contacts(&mut self, sender: relm4::Sender<AppInput>) {
        self.primary = PrimaryView::Contacts;
        self.contacts.entered();
        self.dispatch(Intent::SearchContacts {
            query: String::new(),
        });
        if let Some(app) = &self.app {
            self.contacts.refresh(app);
        }
        self.load_contact_targets(sender);
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

    /// Reads the writable address books, off the UI thread for the same reason as above.
    ///
    /// Read on entering the surface rather than when the create button is pressed, because the
    /// answer decides whether that button exists at all.
    fn load_contact_targets(&self, sender: relm4::Sender<AppInput>) {
        let Some(app) = &self.app else {
            return;
        };
        let app = Arc::clone(app);
        std::thread::spawn(move || {
            let targets = app.contact_targets();
            sender.emit(AppInput::ContactTargetsLoaded(targets));
        });
    }

    pub(super) fn contact_targets_loaded(&mut self, targets: &[ContactTarget]) {
        self.contacts.set_targets(targets, &self.snapshot.accounts);
    }

    /// Opens the create form. A no-op with nowhere to file a contact, which is also why the
    /// button is hidden then: this is the second half of the same rule, not a duplicate of it.
    pub(super) fn begin_new_contact(&mut self) {
        self.contacts.begin_create();
    }

    /// The Edit button beside an open person.
    ///
    /// One editable card opens straight into the form. Several is a question only the user can
    /// answer, because a person is several accounts' cards and an edit writes to exactly one of
    /// them (`docs/contacts.md` §3).
    pub(super) fn edit_open_contact(&mut self, sender: relm4::Sender<AppInput>) {
        let Some((_, cards)) = self.contacts.open_person() else {
            return;
        };
        match cards {
            [] => (),
            [only] => {
                let (account, card) = (only.account.clone(), only.card.clone());
                self.begin_edit_contact(account, card, sender);
            }
            several => {
                let choices = several.to_vec();
                self.contacts.ask_which_card(choices);
            }
        }
    }

    /// Loads one card's values, off the UI thread, then opens the editor on them.
    ///
    /// Seeded from the **card** and never from the person on screen: the person is a merge, so
    /// its values belong to different accounts' cards and saving them into one would file the
    /// work address book's details in the personal one.
    pub(super) fn begin_edit_contact(
        &mut self,
        account: String,
        card: String,
        sender: relm4::Sender<AppInput>,
    ) {
        let (Some(app), Some((person, _))) = (&self.app, self.contacts.open_person()) else {
            return;
        };
        let app = Arc::clone(app);
        let person = person.to_owned();
        std::thread::spawn(move || {
            let seed = app.contact_card(person.clone(), account.clone(), card.clone());
            sender.emit(AppInput::ContactCardLoaded(
                EditTarget {
                    person,
                    account,
                    card,
                },
                seed.map(Box::new),
            ));
        });
    }

    pub(super) fn contact_card_loaded(&mut self, target: EditTarget, seed: Option<ContactEdit>) {
        // A card that has gone (a sync deleted it between the tap and the read) opens no
        // editor: seeding one from nothing would offer to save a blank card over it.
        if let Some(seed) = seed {
            self.contacts.begin_edit(target, seed);
        }
    }

    pub(super) fn dismiss_contact_editor(&mut self) {
        self.contacts.close_editor();
    }

    /// The editor's Save. The intent was already built (and validated) by the editor, so this
    /// only dispatches it and closes the form.
    pub(super) fn submit_contact_form(&mut self, intent: Intent) {
        self.contacts.close_editor();
        self.dispatch(intent);
    }
}
