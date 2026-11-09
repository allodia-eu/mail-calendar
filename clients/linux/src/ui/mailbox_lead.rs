//! What every mail row leads with: a disclosure column, the unread dot, then the avatar carrying
//! the row's action. A conversation puts its chevron in the column; a message leaves it empty at
//! the same width, so dots, avatars and subjects stand in one column down the whole list, as the
//! folder pane's do and as the other desktops' lists do.

use std::sync::Once;

use adw::prelude::*;
use gtk::accessible::Property as AccessibleProperty;

use super::avatar;
use crate::{l10n, ui::icons};

/// A chevron's style class: its glyph's own size, with a target no smaller than the theme's.
const DISCLOSURE: &str = "mailcal-list-disclosure";

thread_local! {
    /// Every chevron in the list and every empty slot standing in for one, all as wide as the
    /// widest.
    static COLUMN: gtk::SizeGroup = gtk::SizeGroup::new(gtk::SizeGroupMode::Horizontal);
}

fn install_styles() {
    static INSTALLED: Once = Once::new();
    if let Some(display) = gtk::gdk::Display::default() {
        INSTALLED.call_once(|| {
            let provider = gtk::CssProvider::new();
            provider.load_from_string(&format!(
                "button.{DISCLOSURE} {{ min-width: 24px; min-height: 24px; padding: 0; }}"
            ));
            gtk::style_context_add_provider_for_display(
                &display,
                &provider,
                gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
            );
        });
    }
}

/// The row's lead: `disclosure`, the dot, then `open`, the avatar's button. One box of this
/// file's own, so a message row and a conversation row space the three alike; each kind of
/// libadwaita row arranges its own prefixes otherwise.
pub(super) fn lead(
    disclosure: &impl IsA<gtk::Widget>,
    unread: bool,
    open: &gtk::Button,
) -> gtk::Box {
    let lead = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    lead.set_valign(gtk::Align::Center);
    lead.append(disclosure);
    lead.append(&avatar::unread_dot(unread));
    lead.append(open);
    lead
}

/// The disclosure column of a row with nothing to open: empty, and as wide as a chevron.
pub(super) fn slot() -> gtk::Box {
    let slot = gtk::Box::builder()
        .accessible_role(gtk::AccessibleRole::Presentation)
        .build();
    COLUMN.with(|column| column.add_widget(&slot));
    slot
}

/// A conversation's chevron, leading its row in place of libadwaita's own arrow at the end, and
/// pointing the way the folder pane's do: along the row while shut, down while open.
///
/// A button of its own, so folding a conversation away is not also selecting it: a `GtkButton`
/// consumes its own click, and the row's header beside it still opens the conversation.
pub(super) fn chevron(row: &adw::ExpanderRow) -> gtk::Button {
    install_styles();
    hide_stock_arrow(row);
    let button = gtk::Button::new();
    button.add_css_class("flat");
    button.add_css_class(DISCLOSURE);
    button.set_valign(gtk::Align::Center);
    COLUMN.with(|column| column.add_widget(&button));
    show_state(&button, row.is_expanded());
    let toggled = row.downgrade();
    button.connect_clicked(move |_| {
        if let Some(row) = toggled.upgrade() {
            row.set_expanded(!row.is_expanded());
        }
    });
    // Weak, for the reason the row's own handlers are: the button lives inside the row.
    let drawn = button.downgrade();
    row.connect_expanded_notify(move |row| {
        if let Some(button) = drawn.upgrade() {
            show_state(&button, row.is_expanded());
        }
    });
    button
}

fn show_state(button: &gtk::Button, expanded: bool) {
    button.set_icon_name(if expanded {
        icons::EXPANDED
    } else {
        icons::COLLAPSED
    });
    let spoken = if expanded {
        l10n::a11y_collapse_conversation()
    } else {
        l10n::a11y_expand_conversation()
    };
    button.set_tooltip_text(Some(spoken));
    button.update_property(&[AccessibleProperty::Label(spoken)]);
}

/// Takes libadwaita's arrow off the row's end: the chevron leading the row says the same thing,
/// and two arrows on one row, pointing different ways, said it twice. The arrow is presentation
/// only, so nothing an assistive technology reads goes with it.
fn hide_stock_arrow(row: &adw::ExpanderRow) {
    let mut stack = vec![row.clone().upcast::<gtk::Widget>()];
    while let Some(widget) = stack.pop() {
        if widget.has_css_class("expander-row-arrow") {
            widget.set_visible(false);
            return;
        }
        let mut child = widget.first_child();
        while let Some(next) = child {
            child = next.next_sibling();
            stack.push(next);
        }
    }
}
