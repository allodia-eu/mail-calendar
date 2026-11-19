//! Tone and approach as cards, and the person's phrases as chips with what they avoid struck
//! through; and the one function that draws any per-language section, which the reveal pages
//! and the detail share.

use adw::prelude::*;
use mailcal_bindings::{
    WritingStyleCard, WritingStyleLanguage, WritingStylePhrases, WritingStyleRevealPage,
};

use super::{
    writing_style_reveal_pages::{self as pages, heading, text},
    writing_style_words as words,
};
use crate::l10n;

/// Draws the section `page` of `language` into `body`, or says the language has nothing for it.
pub(super) fn section(
    body: &gtk::Box,
    page: WritingStyleRevealPage,
    language: &WritingStyleLanguage,
) {
    let drew = match page {
        WritingStyleRevealPage::Letter => language
            .letter
            .as_ref()
            .map(|letter| pages::letter(body, letter)),
        WritingStyleRevealPage::Habits => language
            .habits
            .as_ref()
            .map(|habits| pages::habits(body, habits)),
        WritingStyleRevealPage::Voice => {
            (!language.cards.is_empty()).then(|| voice(body, &language.cards))
        }
        WritingStyleRevealPage::Phrases => language
            .phrases
            .as_ref()
            .map(|phrases| self::phrases(body, phrases)),
        WritingStyleRevealPage::Read | WritingStyleRevealPage::Name => Some(()),
    };
    if drew.is_none() {
        let nothing = text(l10n::reveal_nothing_in_language());
        nothing.add_css_class("dim-label");
        body.append(&nothing);
    }
}

/// One card per topic: what it is about, its heading, and the text under it.
fn voice(body: &gtk::Box, cards: &[WritingStyleCard]) {
    for card in cards {
        let frame = gtk::Box::new(gtk::Orientation::Vertical, 0);
        frame.add_css_class("card");
        frame.add_css_class("mailcal-reveal-card");
        let caption = text(words::topic(card.topic));
        caption.add_css_class("caption-heading");
        caption.add_css_class("dim-label");
        frame.append(&caption);
        let title = text(&card.heading);
        title.add_css_class("heading");
        title.set_margin_top(8);
        frame.append(&title);
        if !card.body.trim().is_empty() {
            let said = text(&card.body);
            said.set_margin_top(4);
            frame.append(&said);
        }
        body.append(&frame);
    }
}

/// The phrases used, and what the person avoids.
fn phrases(body: &gtk::Box, phrases: &WritingStylePhrases) {
    if !phrases.used.is_empty() {
        body.append(&chips(&phrases.used, false));
    }
    if !phrases.avoided.is_empty() {
        let group = gtk::Box::new(gtk::Orientation::Vertical, 8);
        group.set_margin_top(6);
        group.append(&heading(l10n::reveal_avoid()));
        group.append(&chips(&phrases.avoided, true));
        body.append(&group);
    }
}

fn chips(phrases: &[String], avoided: bool) -> adw::WrapBox {
    let cloud = adw::WrapBox::new();
    cloud.set_child_spacing(8);
    cloud.set_line_spacing(8);
    for phrase in phrases {
        let chip = gtk::Label::new(None);
        chip.set_wrap(true);
        chip.add_css_class("mailcal-reveal-chip");
        if avoided {
            // Struck through by Pango: GTK's style sheets draw no line through a label's text.
            chip.set_markup(&format!("<s>{}</s>", gtk::glib::markup_escape_text(phrase)));
            chip.add_css_class("mailcal-reveal-avoid");
        } else {
            chip.set_text(phrase);
            chip.add_css_class("card");
        }
        cloud.append(&chip);
    }
    cloud
}
