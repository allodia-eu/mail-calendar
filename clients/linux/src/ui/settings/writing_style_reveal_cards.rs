//! The reveal's fourth and fifth pages: tone and approach as three cards, and the person's phrases
//! as chips, with what they avoid struck through.

use std::{cell::Cell, rc::Rc};

use adw::prelude::*;
use gtk::{glib, pango};
use mailcal_bindings::LanguageStyleRow;

use super::writing_style_reveal_pages::{flow_child, heading, text};
use crate::{l10n, ui::writing_style_reveal::card_text};

/// Step 4: register, structure and how the person declines and chases, one card each. A card the
/// model gave nothing for is left out.
pub(super) fn voice(body: &gtk::Box, style: &LanguageStyleRow) {
    let cards = [
        (
            l10n::reveal_register(),
            card_text(&style.register_headline, &style.register),
        ),
        (
            l10n::reveal_structure(),
            card_text(&style.structure_headline, &style.structure),
        ),
        (
            l10n::reveal_moves(),
            card_text(&style.moves_headline, &style.moves),
        ),
    ];
    for (label, (headline, description)) in cards {
        if !headline.is_empty() || !description.is_empty() {
            body.append(&card(label, &headline, &description));
        }
    }
}

/// One card: what it is about, a heading, and two lines of the description that open to the rest.
fn card(label: &str, headline: &str, description: &str) -> gtk::Box {
    let card = gtk::Box::new(gtk::Orientation::Vertical, 0);
    card.add_css_class("card");
    card.add_css_class("mailcal-reveal-card");
    let top = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    let caption = text(label);
    caption.add_css_class("caption-heading");
    caption.add_css_class("dim-label");
    caption.set_valign(gtk::Align::Center);
    top.append(&caption);
    card.append(&top);
    if !headline.is_empty() {
        let title = text(headline);
        title.add_css_class("heading");
        title.set_margin_top(9);
        title.set_margin_bottom(3);
        card.append(&title);
    }
    if !description.is_empty() {
        let body = text(description);
        let more = gtk::Button::with_label(l10n::reveal_card_more());
        more.add_css_class("flat");
        more.add_css_class("caption");
        more.set_valign(gtk::Align::Center);
        top.append(&more);
        card.append(&body);
        clamp(&body, false);
        let expanded = Rc::new(Cell::new(false));
        let (shown, open) = (body.clone(), Rc::clone(&expanded));
        more.connect_clicked(move |button| {
            open.set(!open.get());
            clamp(&shown, open.get());
            button.set_label(if open.get() {
                l10n::reveal_card_less()
            } else {
                l10n::reveal_card_more()
            });
        });
        // More is offered only when two lines hide something, which is known once the text has
        // been laid out at the card's width.
        let (button, weak) = (more.downgrade(), body.downgrade());
        body.connect_map(move |_| {
            let (button, weak, open) = (button.clone(), weak.clone(), Rc::clone(&expanded));
            glib::idle_add_local_once(move || {
                if let (Some(button), Some(body)) = (button.upgrade(), weak.upgrade()) {
                    button.set_visible(open.get() || body.layout().is_ellipsized());
                }
            });
        });
    }
    card
}

/// Two lines and an ellipsis, or the whole text.
fn clamp(label: &gtk::Label, open: bool) {
    if open {
        label.set_lines(-1);
        label.set_ellipsize(pango::EllipsizeMode::None);
    } else {
        label.set_lines(2);
        label.set_ellipsize(pango::EllipsizeMode::End);
    }
}

/// Step 5: the person's phrases, and what they avoid.
pub(super) fn phrases(body: &gtk::Box, style: &LanguageStyleRow) {
    if let Some(used) = chips(&style.phrases, false) {
        used.set_halign(gtk::Align::Center);
        body.append(&used);
    }
    if let Some(avoided) = chips(&style.avoid, true) {
        let group = gtk::Box::new(gtk::Orientation::Vertical, 8);
        group.set_margin_top(6);
        group.append(&heading(l10n::reveal_avoid()));
        avoided.set_halign(gtk::Align::Start);
        group.append(&avoided);
        body.append(&group);
    }
}

/// The phrases as chips, or nothing when there are none. What the person avoids is struck through.
fn chips(phrases: &[String], avoided: bool) -> Option<gtk::FlowBox> {
    let phrases = phrases
        .iter()
        .map(String::as_str)
        .map(str::trim)
        .filter(|phrase| !phrase.is_empty())
        .collect::<Vec<_>>();
    if phrases.is_empty() {
        return None;
    }
    let cloud = gtk::FlowBox::new();
    cloud.set_selection_mode(gtk::SelectionMode::None);
    cloud.set_max_children_per_line(24);
    cloud.set_column_spacing(8);
    cloud.set_row_spacing(8);
    for phrase in phrases {
        let chip = gtk::Label::new(Some(phrase));
        chip.set_wrap(true);
        chip.add_css_class("mailcal-reveal-chip");
        if avoided {
            chip.add_css_class("mailcal-reveal-avoid");
            chip.add_css_class("dim-label");
        } else {
            chip.add_css_class("card");
        }
        cloud.append(&flow_child(&chip));
    }
    Some(cloud)
}
