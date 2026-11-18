//! The Signatures settings category, drawn from the core's `signatures_page`
//! (`docs/signatures.md`): the library first, since an account's picker with nothing to pick
//! means nothing, then each account's **For new messages** and **For replies or forwards**
//! pickers. **None** is a real choice in both rather than a separate switch, so no second control
//! can disagree with them. Every control is drawn through [`controls`], and the page keeps no copy
//! of the library and no rule of its own.
//!
//! The window asks the page to follow each `Surface::Signatures` and `Surface::Settings` signal:
//! the page is drawn again in place when what it reads differs from what it drew, and waits while
//! a picker's list is open, which a redraw would close under the pointer. The signature editor
//! replaces the window's content in the same toplevel, since creating a second one can race GTK's
//! AT-SPI root enumeration; a redraw of the list never touches it, and it follows the same reads.

use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};

use adw::prelude::*;
use gtk::glib;
use mailcal_bindings::{
    AccountSignatures, Intent, SignatureEntry, SignatureIntent, SignaturePicker, SignatureSlotKind,
    SignaturesPage,
};

use super::{PageContext, SettingsWindow, group, page_box, redraw, signature_editor};
use crate::{
    l10n,
    ui::{controls, mailbox::plain_text_row},
};

/// The Signatures page the window drew last, and the last read it was asked to follow.
#[derive(Clone, Default)]
pub(super) struct Shown {
    view: Rc<RefCell<Option<Rc<View>>>>,
    read: Rc<Cell<u64>>,
}

impl std::fmt::Debug for Shown {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("Shown")
    }
}

impl SettingsWindow {
    /// Brings Settings → Signatures, and an editor open on it, to the core's page once for each
    /// read the model counted.
    pub(in crate::ui) fn follow_signatures(&self, read: u64) {
        if self.signatures.read.replace(read) == read {
            return;
        }
        let view = self.signatures.view.borrow().clone();
        if let Some(view) = view {
            view.follow();
        }
    }
}

pub(super) fn signatures(ctx: &PageContext) -> gtk::Box {
    let content = page_box(l10n::settings_category_signatures());
    let body = gtk::Box::new(gtk::Orientation::Vertical, 18);
    content.append(&body);
    let view = Rc::new(View {
        ctx: ctx.clone(),
        body,
        drawn: RefCell::new(None),
        waiting_on: RefCell::new(None),
        editor: RefCell::new(None),
    });
    view.draw(ctx.app.signatures_page());
    ctx.signatures.view.replace(Some(view));
    content
}

struct View {
    ctx: PageContext,
    /// Everything under the heading, drawn again whole.
    body: gtk::Box,
    drawn: RefCell<Option<SignaturesPage>>,
    /// The open list a redraw waits for.
    waiting_on: RefCell<Option<glib::WeakRef<gtk::Popover>>>,
    editor: RefCell<Option<Rc<signature_editor::Editor>>>,
}

impl View {
    fn follow(self: &Rc<Self>) {
        if !self.ctx.window.is_visible() {
            return;
        }
        let page = self.ctx.app.signatures_page();
        let editor = self.editor.borrow().clone();
        if let Some(editor) = editor {
            editor.follow(&page);
        }
        if self.drawn.borrow().as_ref() == Some(&page) {
            return;
        }
        match redraw::open_popover(self.body.upcast_ref()) {
            Some(open) => self.wait_for(&open),
            None => self.draw(page),
        }
    }

    /// Follows again once `open` closes: on the next turn of the loop, since the row that owns the
    /// list is still handling the pick when it closes.
    fn wait_for(self: &Rc<Self>, open: &gtk::Popover) {
        let mut waiting = self.waiting_on.borrow_mut();
        if waiting.as_ref().and_then(glib::WeakRef::upgrade).as_ref() == Some(open) {
            return;
        }
        let view = Rc::downgrade(self);
        open.connect_closed(move |_| {
            let view = view.clone();
            glib::idle_add_local_once(move || {
                if let Some(view) = view.upgrade() {
                    view.follow();
                }
            });
        });
        *waiting = Some(open.downgrade());
    }

    fn draw(self: &Rc<Self>, page: SignaturesPage) {
        while let Some(child) = self.body.first_child() {
            self.body.remove(&child);
        }
        let edit = Rc::downgrade(self);
        let delete = Rc::downgrade(self);
        let app = self.ctx.app.clone();
        let actions = Actions {
            edit: Rc::new(move |id| {
                if let Some(view) = edit.upgrade() {
                    view.edit(id.as_deref());
                }
            }),
            delete: Rc::new(move |id| {
                if let Some(view) = delete.upgrade() {
                    view.confirm_delete(id);
                }
            }),
            assign: Rc::new(move |account, slot, signature| {
                app.dispatch(Intent::Signature {
                    intent: SignatureIntent::Assign {
                        account,
                        slot,
                        signature,
                    },
                });
            }),
        };
        page_content(&self.body, &page, &actions);
        self.drawn.replace(Some(page));
    }

    /// Opens the editor on the signature `id`, or on a new one.
    fn edit(self: &Rc<Self>, id: Option<&str>) {
        let page = self.ctx.app.signatures_page();
        let entry = id.and_then(|id| page.signatures.iter().find(|entry| entry.id == id));
        if id.is_some() && entry.is_none() {
            return;
        }
        let view = Rc::downgrade(self);
        let editor = signature_editor::open(&self.ctx, &page, entry, move || {
            if let Some(view) = view.upgrade() {
                view.editor.replace(None);
            }
        });
        self.editor.replace(Some(editor));
    }

    /// Asks before deleting, and says what it costs beyond this list: every account using the
    /// signature loses it, which the core does in one place.
    fn confirm_delete(&self, id: String) {
        let app = self.ctx.app.clone();
        controls::confirm(
            &self.ctx.window,
            l10n::settings_signatures_delete_title(),
            l10n::settings_signatures_delete_message(),
            l10n::settings_signatures_delete(),
            move |confirmed| {
                if confirmed {
                    app.dispatch(Intent::Signature {
                        intent: SignatureIntent::Delete { id: id.clone() },
                    });
                }
            },
        );
    }
}

/// What the page's controls ask for: the editor on a signature or on a new one, a deletion, and
/// an account's slot set.
struct Actions {
    edit: Rc<dyn Fn(Option<String>)>,
    delete: Rc<dyn Fn(String)>,
    assign: Rc<dyn Fn(String, SignatureSlotKind, Option<String>)>,
}

/// Draws `page` into `body`.
fn page_content(body: &gtk::Box, page: &SignaturesPage, actions: &Actions) {
    let library = group(
        l10n::settings_signatures_library_heading(),
        l10n::settings_signatures_library_description(),
    );
    if page.signatures.is_empty() {
        library.add(&named_row(l10n::settings_signatures_empty()));
    }
    for entry in &page.signatures {
        library.add(&signature_row(entry, actions));
    }
    body.append(&library);

    let edit = actions.edit.clone();
    let add = controls::button(
        l10n::settings_signatures_add(),
        page.add,
        false,
        move || {
            edit(None);
        },
    );
    add.add_css_class("suggested-action");
    add.set_halign(gtk::Align::Start);
    body.append(&add);

    let defaults = group(
        l10n::settings_signatures_defaults_heading(),
        l10n::settings_signatures_defaults_description(),
    );
    if page.accounts.is_empty() {
        defaults.add(&named_row(l10n::settings_accounts_empty()));
    }
    let names = std::iter::once(l10n::settings_signatures_none())
        .chain(page.signatures.iter().map(|entry| entry.name.as_str()))
        .collect::<Vec<_>>();
    let ids = page
        .signatures
        .iter()
        .map(|entry| entry.id.clone())
        .collect::<Rc<[_]>>();
    for account in &page.accounts {
        // With one account the address is still shown: the setting is per account, and a person
        // who later adds a second must not have to learn that again.
        let heading = named_row(&account.email);
        heading.add_css_class("heading");
        defaults.add(&heading);
        for (label, slot, picker) in slots(account) {
            defaults.add(&slot_row(
                label, account, slot, picker, &names, &ids, actions,
            ));
        }
    }
    body.append(&defaults);
}

/// One library row: the name, Edit, Delete. The row itself is not activatable; Delete sits
/// beside it, and a stray click that opens an editor is recoverable while one that deletes is not.
fn signature_row(entry: &SignatureEntry, actions: &Actions) -> adw::ActionRow {
    let row = named_row(&entry.name);
    let (edit, id) = (actions.edit.clone(), entry.id.clone());
    let edit = controls::button(
        l10n::settings_signatures_edit(),
        entry.edit,
        false,
        move || {
            edit(Some(id.clone()));
        },
    );
    row.add_suffix(&edit);
    let (delete, id) = (actions.delete.clone(), entry.id.clone());
    let delete = controls::button(
        l10n::settings_signatures_delete(),
        entry.delete,
        true,
        move || delete(id.clone()),
    );
    row.add_suffix(&delete);
    row
}

fn slots(account: &AccountSignatures) -> [(&'static str, SignatureSlotKind, &SignaturePicker); 2] {
    [
        (
            l10n::settings_signatures_new_message_label(),
            SignatureSlotKind::NewMessage,
            &account.new_message,
        ),
        (
            l10n::settings_signatures_reply_forward_label(),
            SignatureSlotKind::ReplyForward,
            &account.reply_forward,
        ),
    ]
}

/// One slot's picker, titled by the slot rather than by the account: two rows holding the same
/// signature would otherwise be indistinguishable.
fn slot_row(
    label: &str,
    account: &AccountSignatures,
    slot: SignatureSlotKind,
    picker: &SignaturePicker,
    names: &[&str],
    ids: &Rc<[String]>,
    actions: &Actions,
) -> adw::ComboRow {
    let selected = slot_selection(picker.selected.as_deref(), ids);
    let (assign, ids, account) = (
        actions.assign.clone(),
        Rc::clone(ids),
        account.account_id.clone(),
    );
    controls::combo_row(label, names, selected, picker.control, move |index| {
        assign(account.clone(), slot, slot_choice(index, &ids));
    })
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

/// The entry a slot is drawn on: **None** first, then the library.
fn slot_selection(selected: Option<&str>, ids: &[String]) -> u32 {
    selected
        .and_then(|id| ids.iter().position(|candidate| candidate == id))
        .and_then(|index| u32::try_from(index + 1).ok())
        .unwrap_or(0)
}

/// What picking entry `index` assigns: a signature id, or `None` for the first entry, which is a
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
