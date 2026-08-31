//! One recipient field; To, Cc or Bcc; as pills plus the token being typed, with autosuggest.
//!
//! The value stays **one comma-separated string**, which is what the composer's send path parses.
//! Everything drawn here is a view of it ([`super::tokens`] owns the split), so nothing can be on
//! screen that would not be sent.
//!
//! Two things the pills buy over the plain entry they replace: each address is visibly one thing,
//! where `a@x.test, b@y.test` is a wall of text whose only boundary is a comma the reader has to
//! find and which offers nothing to click to remove a wrong address; and the caret ends up where
//! you would put it, because accepting a suggestion turns that address into a pill and empties the
//! input: the structural fix for "the next keystroke lands inside the address just inserted".
//!
//! The suggestion list is a **popover that does not take focus**. `autohide` would put the grab on
//! the popover, so the next keystroke would never reach the entry underneath it.

use std::{
    cell::{Cell, RefCell},
    rc::{Rc, Weak},
    sync::Arc,
    time::Duration,
};

use adw::prelude::*;
use gtk::{accessible::Property as AccessibleProperty, gdk, glib};
use mailcal_bindings::{MailcalApp, RecipientMatch};

use super::tokens;
use crate::{l10n, ui::mailbox};

/// How long the field waits after the last keystroke before asking the core.
///
/// Long enough that typing a word costs one query rather than one per character, short enough that
/// the list still arrives while the user is looking at the field.
const DEBOUNCE: Duration = Duration::from_millis(120);

/// What the composer is told when the field's value changes, however it changed.
type ChangeHandler = Box<dyn Fn(&str)>;

/// A To/Cc/Bcc field: finished recipients as pills, the one in progress as text.
pub(crate) struct RecipientField {
    root: gtk::Box,
    inner: Rc<Inner>,
}

struct Inner {
    pills: gtk::FlowBox,
    entry: gtk::Entry,
    popover: gtk::Popover,
    list: gtk::ListBox,
    /// The whole field, comma-separated; the composer's source of truth.
    text: RefCell<String>,
    /// Bumped on every change, so a debounced query whose token has been superseded can never
    /// land on top of a newer one.
    generation: Cell<u64>,
    /// Suppresses the edit handler while the input is being re-seeded programmatically.
    seeding: Cell<bool>,
    /// The addresses currently offered, in the order the list draws them.
    offered: RefCell<Vec<String>>,
    /// The core lookup. `None` disables autosuggest, which is what a client with no booted core
    /// gets; the field still works, it just offers nothing.
    app: Option<Arc<MailcalApp>>,
    on_changed: RefCell<Option<ChangeHandler>>,
}

impl RecipientField {
    pub(crate) fn new(label: &str, app: Option<Arc<MailcalApp>>) -> Self {
        mailbox::install_styles();

        // Above the input rather than inline before it: GTK 4.14 has no wrapping box, and a row of
        // pills sharing one line with the entry squeezes the entry to nothing on the fourth
        // recipient; exactly when the field is hardest to use.
        let pills = gtk::FlowBox::new();
        pills.set_selection_mode(gtk::SelectionMode::None);
        pills.set_max_children_per_line(32);
        pills.set_column_spacing(4);
        pills.set_row_spacing(4);
        pills.set_visible(false);
        pills.update_property(&[AccessibleProperty::Label(label)]);

        let entry = gtk::Entry::new();
        entry.set_hexpand(true);
        // The caption beside it is a sibling label, which is not a programmatic association:
        // without this the three fields are three unnamed entries and a screen reader
        // cannot tell To from Bcc.
        entry.update_property(&[AccessibleProperty::Label(label)]);

        let list = gtk::ListBox::new();
        list.set_selection_mode(gtk::SelectionMode::Single);
        // Not focusable, so a click on a suggestion never moves focus off the entry; the trap
        // where the field's own focus-out closes the list before the click can land on it.
        list.set_focusable(false);
        list.update_property(&[AccessibleProperty::Label(l10n::compose_suggestions())]);
        let scroll = gtk::ScrolledWindow::new();
        scroll.set_policy(gtk::PolicyType::Never, gtk::PolicyType::Automatic);
        scroll.set_max_content_height(220);
        scroll.set_propagate_natural_height(true);
        scroll.set_child(Some(&list));
        let popover = gtk::Popover::new();
        popover.set_autohide(false);
        popover.set_has_arrow(false);
        popover.set_position(gtk::PositionType::Bottom);
        popover.add_css_class("menu");
        popover.set_child(Some(&scroll));
        popover.set_parent(&entry);

        let root = gtk::Box::new(gtk::Orientation::Vertical, 4);
        root.append(&pills);
        root.append(&entry);

        let inner = Rc::new(Inner {
            pills,
            entry,
            popover,
            list,
            text: RefCell::new(String::new()),
            generation: Cell::new(0),
            seeding: Cell::new(false),
            offered: RefCell::new(Vec::new()),
            app,
            on_changed: RefCell::new(None),
        });
        connect(&inner);
        Self { root, inner }
    }

    pub(crate) fn widget(&self) -> &gtk::Box {
        &self.root
    }

    /// Seeds a field the composer was **opened** with; every address in it is finished.
    pub(crate) fn seed(&self, value: &str) {
        self.inner.apply(&tokens::seeded(value));
    }

    /// The whole field, comma-separated; exactly what is on screen.
    pub(crate) fn text(&self) -> String {
        self.inner.text.borrow().clone()
    }

    /// Called whenever the value changes, however it changed.
    pub(crate) fn connect_changed(&self, on_changed: impl Fn(&str) + 'static) {
        self.inner.on_changed.replace(Some(Box::new(on_changed)));
    }

    pub(crate) fn focus_on_activate(&self) {
        self.inner.entry.set_activates_default(true);
    }

    /// Puts the caret in this field's entry; the composer opens with it in To on a new message
    /// the caller did not address (docs/contacts.md §4).
    ///
    /// Deferred to the entry's `map` when it has not been shown yet: the composer asks for this
    /// while it is still assembling the pane, and `grab_focus` on a widget with no window to take
    /// focus in answers `false` and does nothing: silently, which is how a field ends up looking
    /// ready without being it.
    pub(crate) fn focus_entry(&self) {
        if self.inner.entry.is_mapped() {
            self.inner.entry.grab_focus();
        } else {
            self.inner.entry.connect_map(|entry| {
                entry.grab_focus();
            });
        }
    }
}

/// A popover is parented to the entry but lives in its own surface, so it does **not** go away
/// with the widget tree that owned it: one still up when a draft is cancelled would be left
/// floating over the reading pane, and GTK warns loudly about a popover finalized with a parent
/// still set. Dropping the field is the one thing every caller already does, so the unparenting
/// hangs off that rather than off a `teardown` somebody has to remember.
impl Drop for Inner {
    fn drop(&mut self) {
        self.popover.popdown();
        self.popover.unparent();
    }
}

fn connect(inner: &Rc<Inner>) {
    let weak = Rc::downgrade(inner);
    inner.entry.connect_changed(move |entry| {
        let Some(inner) = weak.upgrade() else {
            return;
        };
        if inner.seeding.get() {
            return;
        }
        // Only the trailing token is the user's to edit; the recipients already finished are
        // carried over verbatim rather than re-parsed out of the box.
        let text = inner.text.borrow().clone();
        let committed = tokens::committed(&text);
        inner.apply(&tokens::field_text(&committed, &entry.text()));
    });

    let keys = gtk::EventControllerKey::new();
    // Capture, not bubble: the entry claims Return for its own `activate` before a bubbling
    // controller ever sees it, so Enter would move focus instead of accepting the highlighted
    // suggestion. Every other key is passed straight through, so typing still reaches the text.
    keys.set_propagation_phase(gtk::PropagationPhase::Capture);
    let weak = Rc::downgrade(inner);
    keys.connect_key_pressed(move |_, key, _, _| {
        let Some(inner) = weak.upgrade() else {
            return glib::Propagation::Proceed;
        };
        inner.key_pressed(key)
    });
    inner.entry.add_controller(keys);

    let focus = gtk::EventControllerFocus::new();
    let weak: Weak<Inner> = Rc::downgrade(inner);
    focus.connect_leave(move |_| {
        if let Some(inner) = weak.upgrade() {
            inner.popover.popdown();
        }
    });
    inner.entry.add_controller(focus);
}

impl Inner {
    /// The one place the field's value moves. A keystroke, an accepted suggestion and a removed
    /// pill all compute the new string with [`tokens`] and come through here.
    fn apply(self: &Rc<Self>, value: &str) {
        if *self.text.borrow() == value {
            return;
        }
        self.text.replace(value.to_owned());
        self.rebuild_pills();

        // Re-seed the input only when the TOKEN changed, and compare **trimmed**; that is the
        // whole point of the comparison. `current_token` trims, so the token derived from the field
        // has lost the space the user just typed; compare raw and typing "John " re-seeds the input
        // as "John", eating it. Every space then goes silently, "John Smith" arrives as
        // "JohnSmith", and a name-based query can never match anything.
        let token = tokens::current_token(value).to_owned();
        if self.entry.text().trim() != token {
            self.seeding.set(true);
            self.entry.set_text(&token);
            // The caret goes to the end after any programmatic change (docs/contacts.md §4).
            self.entry.set_position(-1);
            self.seeding.set(false);
        }

        if let Some(on_changed) = self.on_changed.borrow().as_ref() {
            on_changed(value);
        }
        let generation = self.generation.get().wrapping_add(1);
        self.generation.set(generation);
        self.query(token, generation);
    }

    fn accept(self: &Rc<Self>, row: i32) {
        let Ok(index) = usize::try_from(row) else {
            return;
        };
        // The address goes in bare, never as `Name <address>`: see `tokens::accept`.
        let Some(email) = self.offered.borrow().get(index).cloned() else {
            return;
        };
        let field = self.text.borrow().clone();
        self.apply(&tokens::accept(&field, &email));
        self.entry.grab_focus();
    }

    fn key_pressed(self: &Rc<Self>, key: gdk::Key) -> glib::Propagation {
        if !self.popover.is_visible() {
            return glib::Propagation::Proceed;
        }
        match key {
            // An overlay with no keyboard way out is a trap, and the composer binds Escape to
            // nothing else; it is a pane, not a dialog.
            gdk::Key::Escape => {
                self.popover.popdown();
                glib::Propagation::Stop
            }
            gdk::Key::Down => self.step_selection(1),
            gdk::Key::Up => self.step_selection(-1),
            gdk::Key::Return | gdk::Key::KP_Enter => match self.list.selected_row() {
                Some(row) => {
                    self.accept(row.index());
                    glib::Propagation::Stop
                }
                // Nothing picked yet, so Enter is the composer's, not the list's.
                None => glib::Propagation::Proceed,
            },
            _ => glib::Propagation::Proceed,
        }
    }

    fn step_selection(&self, step: i32) -> glib::Propagation {
        let count = i32::try_from(self.offered.borrow().len()).unwrap_or(0);
        if count == 0 {
            return glib::Propagation::Proceed;
        }
        let next = match self.list.selected_row() {
            Some(row) => (row.index() + step).clamp(0, count - 1),
            // Arriving from the entry: Down takes the first, Up the last.
            None if step > 0 => 0,
            None => count - 1,
        };
        self.list.select_row(self.list.row_at_index(next).as_ref());
        glib::Propagation::Stop
    }

    fn rebuild_pills(self: &Rc<Self>) {
        while let Some(child) = self.pills.first_child() {
            self.pills.remove(&child);
        }
        let text = self.text.borrow().clone();
        let recipients = tokens::committed(&text);
        for (index, address) in recipients.iter().enumerate() {
            self.pills.append(&pill(address, index, self));
        }
        self.pills.set_visible(!recipients.is_empty());
    }

    /// Debounced, off-the-UI-thread lookup.
    ///
    /// `recipient_suggestions` is network-free but blocks on the core's runtime and reaches the
    /// store's connection thread three times, so a per-keystroke call on the GLib loop stalls the
    /// composer whenever a sync holds that connection.
    fn query(self: &Rc<Self>, token: String, generation: u64) {
        let Some(app) = self.app.clone() else {
            self.hide_suggestions();
            return;
        };
        if token.is_empty() {
            self.hide_suggestions();
            return;
        }
        // Weak, and re-checked after every await: the draft can be cancelled while the debounce
        // or the lookup is still running, and a strong reference would keep the field alive just
        // long enough to raise a popover over whatever replaced the composer.
        let inner = Rc::downgrade(self);
        glib::MainContext::default().spawn_local(async move {
            glib::timeout_future(DEBOUNCE).await;
            if inner
                .upgrade()
                .is_none_or(|inner| inner.generation.get() != generation)
            {
                return;
            }
            let (sender, receiver) = relm4::channel::<Vec<RecipientMatch>>();
            std::thread::spawn(move || {
                let _ = sender.send(app.recipient_suggestions(token));
            });
            let Some(matches) = receiver.recv().await else {
                return;
            };
            let Some(inner) = inner.upgrade() else {
                return;
            };
            if inner.generation.get() != generation {
                return;
            }
            inner.show_suggestions(&matches);
        });
    }

    fn show_suggestions(self: &Rc<Self>, matches: &[RecipientMatch]) {
        mailbox::clear(&self.list);
        let mut offered = Vec::with_capacity(matches.len());
        for found in matches {
            self.list.append(&suggestion_row(found, self));
            offered.push(found.email.clone());
        }
        let show = tokens::should_show_suggestions(&self.text.borrow(), &offered);
        self.offered.replace(offered);
        // `root()` is `None` once the composer's tree has been taken down. Raising a popover from
        // a detached widget is not a no-op; GTK asks GDK for a popup surface whose parent has
        // none, and the process dies on the assertion.
        if !show || self.entry.root().is_none() {
            self.popover.popdown();
            return;
        }
        // Sized to the entry rather than to its longest address, which would hang a wide list off
        // the edge of a narrow composer pane.
        let width = self.entry.width();
        if width > 0 {
            self.popover.set_size_request(width, -1);
        }
        self.list.select_row(None::<&gtk::ListBoxRow>);
        self.popover.popup();
    }

    fn hide_suggestions(&self) {
        self.offered.replace(Vec::new());
        self.popover.popdown();
    }
}

/// One finished recipient. Its remove control names the recipient rather than repeating a bare
/// "Remove", so it is distinguishable when a screen reader reaches the third otherwise identical
/// button.
fn pill(address: &str, index: usize, inner: &Rc<Inner>) -> gtk::Box {
    let root = gtk::Box::new(gtk::Orientation::Horizontal, 2);
    root.add_css_class("mailcal-pill");
    // A `GtkLabel` set through `set_text` is never markup, which matters: an address is the
    // server's text and a bare ampersand in one must render rather than fail a parse.
    let label = gtk::Label::new(Some(address));
    label.set_ellipsize(gtk::pango::EllipsizeMode::Middle);
    label.set_max_width_chars(30);
    label.set_tooltip_text(Some(address));
    root.append(&label);
    let remove = gtk::Button::from_icon_name("window-close-symbolic");
    remove.add_css_class("flat");
    remove.add_css_class("circular");
    remove.set_valign(gtk::Align::Center);
    let spoken = format!("{address}: {}", l10n::compose_remove_recipient());
    remove.set_tooltip_text(Some(&spoken));
    remove.update_property(&[AccessibleProperty::Label(&spoken)]);
    let weak = Rc::downgrade(inner);
    remove.connect_clicked(move |_| {
        let Some(inner) = weak.upgrade() else {
            return;
        };
        let field = inner.text.borrow().clone();
        inner.apply(&tokens::remove(&field, index));
    });
    root.append(&remove);
    root
}

/// One ranked suggestion. A match that came only from **sent mail** often carries no name of its
/// own; it is as valid as one from a saved card, and usually the more useful, so it shows its
/// address alone rather than being hidden. Sent mail that named the recipient by their address
/// yields a "name" equal to it, which is the same case: one line, not the address twice.
fn suggestion_row(found: &RecipientMatch, inner: &Rc<Inner>) -> adw::ActionRow {
    let row = mailbox::plain_text_row();
    if found.display_name.is_empty() || found.display_name == found.email {
        row.set_title(&found.email);
    } else {
        row.set_title(&found.display_name);
        row.set_subtitle(&found.email);
        // The address as the **description**, not the label: an `AdwActionRow` labels itself from
        // its title through a `labelled-by` relation, which by the ARIA rules GTK follows beats an
        // explicit label; so setting one changes nothing a screen reader hears. A description is
        // announced after the name, which is where a clarifying address belongs anyway; without it
        // a suggestion is spoken as a name alone, and the address is the half that settles whether
        // it is the right person.
        row.upcast_ref::<gtk::Widget>()
            .update_property(&[AccessibleProperty::Description(&found.email)]);
    }
    row.set_title_lines(1);
    row.set_subtitle_lines(1);
    row.set_activatable(true);
    row.set_focusable(false);
    // The row's own `activated`, as every other list in this client uses: a `GtkListBox`
    // `row-activated` handler beside it would fire on the same click and insert the address twice.
    let weak = Rc::downgrade(inner);
    row.connect_activated(move |row| {
        if let Some(inner) = weak.upgrade() {
            inner.accept(row.index());
        }
    });
    row
}

#[cfg(test)]
#[path = "field_tests.rs"]
pub(crate) mod tests;
