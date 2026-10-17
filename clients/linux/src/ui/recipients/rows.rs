//! The rows a recipient field draws: a pill per finished address, and a suggestion per match.

use std::rc::Rc;

use adw::prelude::*;
use gtk::accessible::Property as AccessibleProperty;
use mailcal_bindings::RecipientMatch;

use super::field::Inner;
use crate::{
    l10n,
    ui::{icons, mailbox},
};

/// One finished recipient. Its remove control names the recipient rather than repeating a bare
/// "Remove", so it is distinguishable when a screen reader reaches the third otherwise identical
/// button.
pub(super) fn pill(address: &str, index: usize, inner: &Rc<Inner>) -> gtk::Box {
    let root = gtk::Box::new(gtk::Orientation::Horizontal, 2);
    root.add_css_class("mailcal-pill");
    // A `GtkLabel` set through `set_text` is never markup, which matters: an address is the
    // server's text and a bare ampersand in one must render rather than fail a parse.
    let label = gtk::Label::new(Some(address));
    label.set_ellipsize(gtk::pango::EllipsizeMode::Middle);
    label.set_max_width_chars(30);
    label.set_tooltip_text(Some(address));
    root.append(&label);
    let remove = gtk::Button::from_icon_name(icons::CLOSE);
    remove.add_css_class("flat");
    remove.add_css_class("circular");
    remove.set_valign(gtk::Align::Center);
    // Clicked, it leaves focus in the entry: taking it would be leaving the field, which finishes
    // the address being typed (docs/contacts.md) and rebuilds the pills under the press.
    remove.set_focus_on_click(false);
    let spoken = format!("{address}: {}", l10n::compose_remove_recipient());
    remove.set_tooltip_text(Some(&spoken));
    remove.update_property(&[AccessibleProperty::Label(&spoken)]);
    let weak = Rc::downgrade(inner);
    remove.connect_clicked(move |_| {
        let Some(inner) = weak.upgrade() else {
            return;
        };
        inner.remove(index);
    });
    root.append(&remove);
    root
}

/// One ranked suggestion. A match that came only from **sent mail** often carries no name of its
/// own; it is as valid as one from a saved card, and usually the more useful, so it shows its
/// address alone rather than being hidden. Sent mail that named the recipient by their address
/// yields a "name" equal to it, which is the same case: one line, not the address twice.
pub(super) fn suggestion_row(found: &RecipientMatch, inner: &Rc<Inner>) -> adw::ActionRow {
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
