//! The widgets every settings page is built from.
//!
//! Split from [`super`] to keep both files inside the 500-line limit. What lives here is the shape
//! of a page and the two rules a caller must not have to remember: a group's heading and
//! description are **text**, and a row's title is too.

use adw::prelude::*;

pub(super) fn page_box(title: &str) -> gtk::Box {
    let content = dialog_box();
    let heading = gtk::Label::new(Some(title));
    heading.add_css_class("title-1");
    heading.set_xalign(0.0);
    content.append(&heading);
    content
}

pub(super) fn dialog_box() -> gtk::Box {
    let content = gtk::Box::new(gtk::Orientation::Vertical, 18);
    content.set_margin_start(28);
    content.set_margin_end(28);
    content.set_margin_top(24);
    content.set_margin_bottom(28);
    content
}

/// A settings group whose heading and description are **text**, not Pango markup.
///
/// `AdwPreferencesGroup` parses both as markup and offers no `use-markup` to turn it off: the
/// opt-out its *rows* have does not exist here. A bare ampersand fails the parse and the label
/// renders **empty**, so "Allodia Mail & Calendar" is a blank heading, and any catalog string that
/// ever gains an `&` goes the same way. Escaping is the only lever, and it has to be applied
/// here rather than per caller.
pub(super) fn group(title: &str, description: &str) -> adw::PreferencesGroup {
    adw::PreferencesGroup::builder()
        .title(gtk::glib::markup_escape_text(title).as_str())
        .description(gtk::glib::markup_escape_text(description).as_str())
        .build()
}

pub(super) fn choice(
    title: &str,
    labels: &[&str],
    selected: u32,
) -> (adw::ActionRow, gtk::DropDown) {
    let row = adw::ActionRow::builder()
        .use_markup(false)
        .title(title)
        .build();
    let picker = gtk::DropDown::from_strings(labels);
    picker.set_selected(selected);
    picker.set_valign(gtk::Align::Center);
    row.add_suffix(&picker);
    (row, picker)
}
