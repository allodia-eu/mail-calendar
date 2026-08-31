//! The Signatures settings category: the library; write once, reuse on any account; above the
//! per-account defaults, one **For new messages** and one **For replies or forwards** picker each.
//!
//! State lives in the Rust core (the `SignaturesSnapshot`); this renders it and dispatches the
//! setters. Two things the layout is deliberate about, and they match every other client: the
//! library comes first, because an account picker with nothing to pick is meaningless; a
//! first-time user has to write a signature before the defaults mean anything. And **None** is a
//! real option in both pickers rather than a separate enable switch: "None in both" already says
//! "this account sends no signature", and a second control that could disagree with the pickers is
//! a bug waiting to happen.
//!
//! **The category is built once and then reconciled, never rebuilt**: a create appends a row, a
//! rename retitles one, the empty-state row is hidden rather than removed, and every slot picker
//! shares one model, so a new signature reaches all of them without a picker being replaced.
//!
//! Reconciliation keeps the long-lived Settings tree quiet. The signature editor stays in that
//! same toplevel too; creating and destroying a second visible toplevel can race GTK's AT-SPI root
//! enumeration (`docs/signatures.md`).

use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};

use adw::prelude::*;
use gtk::glib;
use mailcal_bindings::{AccountSignatureRow, SignatureRow, SignatureSlotKind, SignaturesSnapshot};

use super::{PageContext, group, page_box, pages, signature_editor};
use crate::{l10n, ui::mailbox::plain_text_row};

pub(super) fn signatures(ctx: &PageContext) -> gtk::Box {
    let content = page_box(l10n::settings_category_signatures());
    let snapshot = ctx.app.signatures();

    let library_group = group(
        l10n::settings_signatures_library_heading(),
        l10n::settings_signatures_library_description(),
    );
    // Present from the start and hidden when it does not apply, so writing a first signature adds
    // a row rather than replacing one.
    let empty = named_row(l10n::settings_signatures_empty());
    empty.set_visible(snapshot.signatures.is_empty());
    library_group.add(&empty);

    // One model behind every slot picker: a create appends a name to it and each account's two
    // pickers gain the option, with no picker being rebuilt.
    let names = gtk::StringList::new(&[l10n::settings_signatures_none()]);

    let library = Library {
        ctx: ctx.clone(),
        group: library_group.downgrade(),
        empty: empty.downgrade(),
        rows: Rc::new(RefCell::new(Vec::new())),
        names: names.clone(),
        ids: Rc::new(RefCell::new(Vec::new())),
        pickers: Rc::new(RefCell::new(Vec::new())),
        syncing: Rc::new(Cell::new(false)),
    };

    content.append(&library_group);
    content.append(&library.add_button());
    content.append(&library.defaults_group(&snapshot));
    library.refresh();
    content
}

/// The category's live widgets, and the bookkeeping that lets them be updated rather than replaced.
///
/// Cloned into every handler; the widget references are weak so a handler cannot keep the page
/// alive past the Settings window.
#[derive(Clone)]
struct Library {
    ctx: PageContext,
    group: glib::WeakRef<adw::PreferencesGroup>,
    /// The "you haven't written one yet" row, shown and hidden rather than added and removed.
    empty: glib::WeakRef<adw::ActionRow>,
    /// The rows on screen, in the core's order, so a refresh can tell a create from a rename.
    rows: Rc<RefCell<Vec<(String, adw::ActionRow)>>>,
    /// **None** first, then the library; the model every slot picker draws from.
    names: gtk::StringList,
    /// The library's ids, parallel to `names` from index 1.
    ids: Rc<RefCell<Vec<String>>>,
    /// Every slot picker: which account and slot it sets, and the row itself.
    pickers: Rc<RefCell<Vec<Picker>>>,
    /// Set while selections are written back from the core, so a picker's own change handler does
    /// not echo them straight back into it.
    syncing: Rc<Cell<bool>>,
}

#[derive(Clone)]
struct Picker {
    account: String,
    new_message: bool,
    row: adw::ComboRow,
}

impl Library {
    /// Re-reads the core and brings the category to match it.
    ///
    /// Every setter signals `Surface::Settings`, but this window is built once and does not listen
    /// for that; reconciling here is what makes a create, a rename, a delete or an assignment
    /// visible at once; including a delete's other half, which clears the slots that pointed at
    /// it.
    fn refresh(&self) {
        let snapshot = self.ctx.app.signatures();
        self.sync_names(&snapshot.signatures);
        self.sync_rows(&snapshot.signatures);
        self.sync_selections(&snapshot.accounts);
    }

    /// Brings the shared picker model to match the library, **entry by entry**.
    ///
    /// Not a wholesale splice: replacing every entry replaces the rows a realised picker popover
    /// built from them, which is the widget destruction this category exists to avoid. Written
    /// this way a create is a pure `append`, a rename touches one entry, and an assignment touches
    /// none.
    fn sync_names(&self, signatures: &[SignatureRow]) {
        for (index, signature) in signatures.iter().enumerate() {
            let Ok(position) = u32::try_from(index + 1) else {
                break;
            };
            match self.names.string(position) {
                Some(current) if current == signature.name => {}
                Some(_) => self.names.splice(position, 1, &[signature.name.as_str()]),
                None => self.names.append(&signature.name),
            }
        }
        let wanted = u32::try_from(signatures.len())
            .unwrap_or(u32::MAX)
            .saturating_add(1);
        while self.names.n_items() > wanted {
            self.names.remove(self.names.n_items() - 1);
        }
        self.ids
            .replace(signatures.iter().map(|row| row.id.clone()).collect());
    }

    /// Appends what is new, retitles what was renamed, and removes only what the user deleted.
    fn sync_rows(&self, signatures: &[SignatureRow]) {
        let Some(section) = self.group.upgrade() else {
            return;
        };
        let mut rows = self.rows.borrow_mut();
        rows.retain(|(id, row)| {
            let kept = signatures.iter().any(|signature| &signature.id == id);
            if !kept {
                section.remove(row);
            }
            kept
        });
        for signature in signatures {
            if let Some((_, row)) = rows.iter().find(|(id, _)| id == &signature.id) {
                if row.title() != signature.name {
                    row.set_title(&signature.name);
                }
            } else {
                let row = self.signature_row(signature);
                section.add(&row);
                rows.push((signature.id.clone(), row));
            }
        }
        if let Some(empty) = self.empty.upgrade() {
            empty.set_visible(signatures.is_empty());
        }
    }

    /// Puts each picker on what the account now assigns, without the change looking like the user
    /// making a choice.
    fn sync_selections(&self, accounts: &[AccountSignatureRow]) {
        let ids = self.ids.borrow();
        self.syncing.set(true);
        for picker in self.pickers.borrow().iter() {
            let Some(account) = accounts
                .iter()
                .find(|account| account.account_id == picker.account)
            else {
                continue;
            };
            let assigned = if picker.new_message {
                account.new_message.as_deref()
            } else {
                account.reply_forward.as_deref()
            };
            // Only when it actually moved: assigning the value it already holds still notifies,
            // and a picker that rebuilds its selected child for nothing is churn in the one place
            // this category is trying not to churn.
            let selection = slot_selection(assigned, &ids);
            if picker.row.selected() != selection {
                picker.row.set_selected(selection);
            }
        }
        self.syncing.set(false);
    }

    /// The button that writes a new signature. Outside the group rather than the last row in it,
    /// so appending a library row lands after the existing ones without anything being re-added.
    fn add_button(&self) -> gtk::Button {
        let add = gtk::Button::with_label(l10n::settings_signatures_add());
        add.add_css_class("suggested-action");
        add.set_halign(gtk::Align::Start);
        let library = self.clone();
        add.connect_clicked(move |_| {
            library.edit(signature_editor::EditingSignature {
                id: None,
                name: l10n::settings_signatures_default_name().to_owned(),
                body_html: String::new(),
            });
        });
        add
    }

    /// One library row: the name, Edit, Delete. The row itself is not activatable; Delete sits
    /// beside it, and a stray click that opens an editor is recoverable while one that deletes is
    /// not.
    fn signature_row(&self, signature: &SignatureRow) -> adw::ActionRow {
        let row = named_row(&signature.name);

        let edit = gtk::Button::with_label(l10n::settings_signatures_edit());
        edit.set_valign(gtk::Align::Center);
        let library = self.clone();
        let id = signature.id.clone();
        let name = signature.name.clone();
        edit.connect_clicked(move |_| {
            library.edit(signature_editor::EditingSignature {
                id: Some(id.clone()),
                name: name.clone(),
                // Fetched only now: the snapshot carries names, so drawing this list never drags
                // an embedded logo across the FFI.
                body_html: library
                    .ctx
                    .app
                    .signature_html(id.clone())
                    .unwrap_or_default(),
            });
        });
        row.add_suffix(&edit);

        let delete = gtk::Button::with_label(l10n::settings_signatures_delete());
        delete.add_css_class("destructive-action");
        delete.set_valign(gtk::Align::Center);
        let library = self.clone();
        let id = signature.id.clone();
        delete.connect_clicked(move |_| library.confirm_delete(id.clone()));
        row.add_suffix(&delete);
        row
    }

    /// For each configured account, which signature a new message opens with and which a reply or
    /// forward does; independently, each with **None**.
    fn defaults_group(&self, snapshot: &SignaturesSnapshot) -> adw::PreferencesGroup {
        let section = group(
            l10n::settings_signatures_defaults_heading(),
            l10n::settings_signatures_defaults_description(),
        );
        if snapshot.accounts.is_empty() {
            section.add(&named_row(l10n::settings_accounts_empty()));
            return section;
        }
        for account in &snapshot.accounts {
            // With one account the address is still shown: the setting is per account, and a user
            // who later adds a second must not have to relearn that.
            let heading = named_row(&account.email);
            heading.add_css_class("heading");
            section.add(&heading);
            section.add(&self.slot_row(
                l10n::settings_signatures_new_message_label(),
                account,
                true,
            ));
            section.add(&self.slot_row(
                l10n::settings_signatures_reply_forward_label(),
                account,
                false,
            ));
        }
        section
    }

    /// One slot's picker, labelled by the slot rather than by the account; two rows holding the
    /// same signature would otherwise be indistinguishable.
    fn slot_row(
        &self,
        label: &str,
        account: &AccountSignatureRow,
        new_message: bool,
    ) -> adw::ComboRow {
        let row = slot_picker(label, &self.names);
        let library = self.clone();
        let account_id = account.account_id.clone();
        row.connect_selected_notify(move |picker| {
            // A selection written back from the core is not the user choosing; storing it again
            // would be a write per refresh, and on a slot the core had just cleared it would put
            // the assignment back.
            if library.syncing.get() {
                return;
            }
            let chosen = slot_choice(picker.selected(), &library.ids.borrow());
            library.ctx.app.set_account_signature(
                account_id.clone(),
                if new_message {
                    SignatureSlotKind::NewMessage
                } else {
                    SignatureSlotKind::ReplyForward
                },
                chosen,
            );
        });
        self.pickers.borrow_mut().push(Picker {
            account: account.account_id.clone(),
            new_message,
            row: row.clone(),
        });
        row
    }

    fn edit(&self, editing: signature_editor::EditingSignature) {
        let library = self.clone();
        signature_editor::open(&self.ctx, editing, move || library.refresh());
    }

    /// Deleting is confirmed in a modal, as every other destructive action in this client is, and
    /// the message says what it costs beyond this list: every account pointing at this signature
    /// loses it; which the core does in one place, so no client can forget a teardown path.
    fn confirm_delete(&self, id: String) {
        let dialog = pages::confirm_window(
            &self.ctx.window,
            l10n::settings_signatures_delete_title(),
            l10n::settings_signatures_delete_message(),
        );
        let actions = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        actions.set_halign(gtk::Align::End);
        let cancel = gtk::Button::with_label(l10n::action_cancel());
        let window = dialog.clone();
        cancel.connect_clicked(move |_| window.close());
        actions.append(&cancel);
        let delete = gtk::Button::with_label(l10n::settings_signatures_delete());
        delete.add_css_class("destructive-action");
        let window = dialog.clone();
        let library = self.clone();
        delete.connect_clicked(move |_| {
            library.ctx.app.delete_signature(id.clone());
            library.refresh();
            window.close();
        });
        actions.append(&delete);
        dialog
            .child()
            .and_downcast::<gtk::Box>()
            .expect("confirmation content")
            .append(&actions);
        dialog.present();
    }
}

/// A row whose title is text somebody else wrote; a signature's name, an account's address.
///
/// Markup is switched off with a **setter, before the title**: `AdwPreferencesRow` parses its
/// title as Pango markup by default, so "Sales & Marketing" renders blank and a name shaped like
/// `<b>…</b>` is applied rather than shown. A property builder cannot promise the order, because
/// `g_object_new` applies properties in its own.
fn named_row(text: &str) -> adw::ActionRow {
    let row = plain_text_row();
    row.set_title(text);
    row
}

/// A labelled picker for one slot.
///
/// An `AdwComboRow` rather than the settings pages' row-plus-`GtkDropDown`, because here the label
/// is the whole point: an account has two of these and they both read "None" until one is set. A
/// `GtkDropDown` is labelled by its **selected item** through a relation, and by the ARIA rules GTK
/// follows a relation beats an explicit label: so a screen reader would hear "None, combo box"
/// twice per account with nothing to tell them apart. A combo row is one control that carries its
/// own title, so its name is the slot.
fn slot_picker(label: &str, names: &gtk::StringList) -> adw::ComboRow {
    let row = adw::ComboRow::new();
    row.set_use_markup(false);
    row.set_title(label);
    row.set_model(Some(names));
    row
}

/// Which entry a slot opens on.
///
/// A slot can name a signature that has since been deleted only if the core failed to clear it (it
/// clears every assignment on delete), so falling back to **None** here is a display detail, not a
/// second teardown path.
fn slot_selection(selected: Option<&str>, ids: &[String]) -> u32 {
    selected
        .and_then(|id| ids.iter().position(|candidate| candidate == id))
        .and_then(|index| u32::try_from(index + 1).ok())
        .unwrap_or(0)
}

/// What picking entry `index` assigns: a signature id, or `None` for the first entry: which is a
/// real assignment ("this account sends no signature"), not the absence of one.
fn slot_choice(index: u32, ids: &[String]) -> Option<String> {
    usize::try_from(index)
        .ok()
        .filter(|index| *index > 0)
        .and_then(|index| ids.get(index - 1))
        .cloned()
}

#[cfg(test)]
#[path = "signatures_tests.rs"]
pub(crate) mod tests;
