//! The mail-search chrome over the message list: the field, the scope filter under it, and the
//! horizon line under that.
//!
//! The field is always on screen, as it is beside the contacts list and in the other desktop
//! clients; search is a mode over the list, so leaving it is emptying the field rather than
//! dismissing a surface. The filter and the horizon appear only while a search is running: both
//! describe a search, and neither has anything to say about a folder.

use adw::prelude::*;
use gtk::{accessible::Property as AccessibleProperty, glib::SignalHandlerId};
use mailcal_bindings::{MailboxListSnapshot, SearchScope};

use super::{
    super::AppInput,
    model::{SearchState, horizon_label, scope_label},
};
use crate::l10n;

/// How long typing has to settle before the query is dispatched.
///
/// A search is a full-text query per account plus a store read per hit; roughly a second on a
/// real multi-account device; so one per keystroke stacks searches that then fight each other for
/// the same store (`docs/search.md`). `GtkSearchEntry` already debounces; this only moves its
/// 150 ms default onto the number the contract names, so every client waits the same beat.
const SEARCH_DELAY_MS: u32 = 250;

pub(crate) struct SearchBar {
    root: gtk::Box,
    filter: gtk::Revealer,
    current: gtk::ToggleButton,
    /// The narrowing side's caption, which every render re-reads from the snapshot.
    current_label: gtk::Label,
    all: gtk::ToggleButton,
    /// Both toggles' handlers. The two are grouped, so activating one deactivates the other and
    /// fires on both; rendering the core's scope has to silence the pair, not just the one set.
    scope_handlers: [SignalHandlerId; 2],
    horizon_row: gtk::Box,
    horizon: gtk::Label,
}

impl SearchBar {
    pub(crate) fn new(sender: &relm4::Sender<AppInput>) -> Self {
        let entry = gtk::SearchEntry::new();
        entry.set_placeholder_text(Some(l10n::search_placeholder()));
        entry.update_property(&[AccessibleProperty::Label(l10n::search_placeholder())]);
        entry.set_search_delay(SEARCH_DELAY_MS);
        entry.set_margin_top(6);
        entry.set_margin_bottom(6);
        entry.set_margin_start(12);
        entry.set_margin_end(12);
        let input = sender.clone();
        entry.connect_search_changed(move |entry| {
            input.emit(AppInput::SearchMail(entry.text().to_string()));
        });
        // Escape. `GtkSearchEntry` raises it as a signal of its own, so this needs no key
        // controller: and therefore none of the capture-phase trouble a `GtkEntry` brings.
        //
        // The field is emptied here rather than on the next render, because this is the only
        // moment the query changes without a keystroke behind it. Leaving is dispatched outright
        // instead of via the clear's own `search-changed`: that one is on the debounce timer, and
        // a user who has pressed Escape may not watch the list think first.
        let input = sender.clone();
        entry.connect_stop_search(move |entry| {
            entry.set_text("");
            input.emit(AppInput::SearchMail(String::new()));
        });

        let current = gtk::ToggleButton::new();
        let current_label = scope_button_label(&current);
        // The generic name until the first render reads the snapshot: the narrowing side always
        // ends up naming the view on screen, and "All mail" sitting on it would name the other.
        set_scope_text(&current, &current_label, l10n::search_scope_folder());
        let all = gtk::ToggleButton::new();
        // Set once and never again: the widening side says the same thing whatever is on screen.
        set_scope_text(&all, &scope_button_label(&all), l10n::search_scope_all());
        // Grouped, so the pair behaves as one two-way choice: the core always has a scope, and an
        // ungrouped toggle would let the user click the active side off into a state it has not
        // got. `all` is the core's default and so opens active.
        all.set_group(Some(&current));
        all.set_active(true);
        let scope_handlers = [
            connect_scope(&current, sender, SearchScope::CurrentFolder),
            connect_scope(&all, sender, SearchScope::AllFolders),
        ];
        let scope = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        scope.set_homogeneous(true);
        scope.add_css_class("linked");
        scope.append(&current);
        scope.append(&all);

        let horizon = gtk::Label::new(None);
        horizon.set_xalign(0.0);
        horizon.set_wrap(true);
        horizon.add_css_class("caption");
        horizon.add_css_class("dim-label");
        // A statement the user cannot act on is half the value: search finds only what sync depth
        // kept, and the depth is a setting, so the line carries the way to it.
        let change = gtk::Button::with_label(l10n::search_horizon_change());
        change.add_css_class("flat");
        change.add_css_class("link");
        // The sentence wraps to two lines in a narrow pane; bottom-aligned, the link reads as the
        // end of it rather than floating beside the break.
        change.set_valign(gtk::Align::End);
        let input = sender.clone();
        change.connect_clicked(move |_| input.emit(AppInput::OpenSyncDepthSettings));
        let horizon_row = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        horizon_row.append(&horizon);
        horizon_row.append(&change);

        let revealed = gtk::Box::new(gtk::Orientation::Vertical, 6);
        revealed.set_margin_bottom(6);
        revealed.set_margin_start(12);
        revealed.set_margin_end(12);
        revealed.append(&scope);
        revealed.append(&horizon_row);
        let filter = gtk::Revealer::new();
        filter.set_child(Some(&revealed));

        let root = gtk::Box::new(gtk::Orientation::Vertical, 0);
        root.append(&entry);
        root.append(&filter);

        Self {
            root,
            filter,
            current,
            current_label,
            all,
            scope_handlers,
            horizon_row,
            horizon,
        }
    }

    pub(crate) fn widget(&self) -> &gtk::Box {
        &self.root
    }

    /// Brings the chrome to what the model and the core's snapshot say.
    ///
    /// Every control is driven from state rather than left where the user put it, which is what
    /// keeps the filter honest: the core resets the scope when a search ends, and a toggle still
    /// showing the old narrowing would describe a search that is no longer running.
    ///
    /// **The field is the exception, and deliberately so.** A search is asynchronous: the query
    /// goes out, the core searches, and the snapshot arrives a moment later; by which time the
    /// typing has moved on. Writing the model's query back into the field then discards whatever
    /// arrived since and leaves the cursor at position 0, so the next keystroke lands at the front
    /// of the word. Nothing but the field itself ever sets the query, so there is nothing to
    /// mirror; the one place the model clears it without a keystroke; Escape; clears the field
    /// there, in the same action.
    pub(crate) fn render(&self, state: &SearchState, snapshot: &MailboxListSnapshot) {
        self.filter.set_reveal_child(state.is_active());
        set_scope_text(&self.current, &self.current_label, &scope_label(snapshot));
        self.set_scope(state.scope());
        match horizon_label(snapshot.search_horizon.as_ref()) {
            Some(text) => {
                self.horizon.set_text(&text);
                self.horizon_row.set_visible(true);
            }
            None => self.horizon_row.set_visible(false),
        }
    }

    /// Moves the filter to the core's scope without dispatching one back.
    fn set_scope(&self, scope: SearchScope) {
        let current = matches!(scope, SearchScope::CurrentFolder);
        if self.current.is_active() == current {
            return;
        }
        for (button, handler) in [&self.current, &self.all]
            .into_iter()
            .zip(&self.scope_handlers)
        {
            button.block_signal(handler);
        }
        self.current.set_active(current);
        self.all.set_active(!current);
        for (button, handler) in [&self.current, &self.all]
            .into_iter()
            .zip(&self.scope_handlers)
        {
            button.unblock_signal(handler);
        }
    }
}

/// Gives a toggle a caption of our own. A folder's name is the server's and as long as the server
/// likes, in a pane the user can drag down to 260 px; so it ellipsizes rather than pushing the
/// other side of the filter off.
fn scope_button_label(button: &gtk::ToggleButton) -> gtk::Label {
    let label = gtk::Label::new(None);
    label.set_ellipsize(gtk::pango::EllipsizeMode::End);
    label.set_max_width_chars(16);
    button.set_child(Some(&label));
    label
}

/// Sets a toggle's caption **and** the name an assistive technology announces for it.
///
/// Both, always. A `GtkButton` names itself from a label handed to `set_label`; a child put there
/// directly carries no such relation, so the button reaches the accessibility bus **unnamed**; a
/// screen reader offering "toggle button" twice with nothing to tell them apart. GTK exposes no
/// getter for an accessible label, so no widget test can see this: the oracle is the AT-SPI run
/// (`scripts/dev/test-linux-ui.sh`).
fn set_scope_text(button: &gtk::ToggleButton, label: &gtk::Label, text: &str) {
    label.set_text(text);
    button.update_property(&[AccessibleProperty::Label(text)]);
}

/// Dispatches `scope` when `button` becomes the active side. Only on becoming active: a grouped
/// toggle fires on the side being switched *off* too, which would send the scope the user just
/// left.
fn connect_scope(
    button: &gtk::ToggleButton,
    sender: &relm4::Sender<AppInput>,
    scope: SearchScope,
) -> SignalHandlerId {
    let input = sender.clone();
    button.connect_toggled(move |button| {
        if button.is_active() {
            input.emit(AppInput::SetSearchScope(scope));
        }
    })
}

#[cfg(test)]
#[path = "bar_tests.rs"]
pub(crate) mod tests;
