//! What a rendered widget tree actually *shows*, and what GLib said while it was built.
//!
//! Split from [`super::tests`] so each file stays under the 500-line limit: that one is the
//! crate's single GTK test and the row assertions it runs; this is the oracle every one of them
//! reads the screen through, here and in the Settings, calendar and reading tests beside it.

use adw::prelude::*;

/// Every `GtkLabel` under `root`, in tree order: the widgets that actually carry the row's text.
pub(crate) fn labels(root: &gtk::Widget) -> Vec<gtk::Label> {
    let mut found = Vec::new();
    if let Some(label) = root.downcast_ref::<gtk::Label>() {
        found.push(label.clone());
    }
    let mut child = root.first_child();
    while let Some(node) = child {
        found.extend(labels(&node));
        child = node.next_sibling();
    }
    found
}

/// What the row actually *shows*.
///
/// `ActionRow::title()` returns the string we handed it whatever happens to the label, so it
/// answers "did we ask for this text", not "is this text on screen". A markup-parsed row with
/// a bare ampersand renders **empty** while `title()` still reads back in full, so asserting
/// on the property is a green light for a blank row.
pub(crate) fn rendered_labels(root: &gtk::Widget) -> Vec<String> {
    labels(root)
        .iter()
        .map(|label| label.text().to_string())
        .collect()
}

/// Asserts every `GtkListBoxRow` under `root` is in a `GtkListBox`.
///
/// A row parented to a plain box renders, so no rendering assertion sees it; but GTK's focus
/// walk reaches it and `gtk_list_box_row_grab_focus` fails its own precondition, so the row and
/// the control it carries are skipped. `AdwPreferencesGroup` supplies the list; an `AdwActionRow`
/// or `AdwSwitchRow` appended to a `GtkBox` does not.
pub(crate) fn every_row_belongs_to_a_list(root: &gtk::Widget) {
    if let Some(row) = root.downcast_ref::<gtk::ListBoxRow>() {
        assert!(
            row.parent()
                .is_some_and(|parent| parent.is::<gtk::ListBox>()),
            "a row must sit in a list box, not a plain container: {:?} in {:?}",
            root.type_(),
            row.parent().map(|parent| parent.type_())
        );
    }
    let mut child = root.first_child();
    while let Some(widget) = child {
        every_row_belongs_to_a_list(&widget);
        child = widget.next_sibling();
    }
}

/// Runs `build`, returning every GLib log record it emitted.
///
/// The rendering assertions below cannot see the defect this guards: a row built as
/// `.subtitle(…).use_markup(false)` still *reads* correctly, because libadwaita re-applies
/// the labels when the flag flips. What it leaves behind is a `Failed to set text … from
/// markup` warning per row, on every sender or subject with an ampersand: noise in the
/// diagnostic log a user attaches to a support request. The warning is the only observable,
/// so the test has to read it.
///
/// **This does not nest, and it cannot be made to.** GLib offers no way to read the handler
/// currently installed, so the `log_unset_default_handler` below restores *GLib's* default rather
/// than whatever was there before; any handler installed earlier stops firing, silently, and a
/// test relying on one goes green while the thing it watches for is still happening. Diagnose a
/// GLib record with `scripts/dev/gtk-trace.sh` instead of a handler installed beside this one.
pub(crate) fn glib_records<T>(build: impl FnOnce() -> T) -> (T, Vec<String>) {
    let records = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
    let sink = std::sync::Arc::clone(&records);
    gtk::glib::log_set_default_handler(move |_domain, _level, message| {
        sink.lock().expect("log sink").push(message.to_owned());
    });
    let value = build();
    gtk::glib::log_unset_default_handler();
    let captured = records.lock().expect("log sink").clone();
    (value, captured)
}
