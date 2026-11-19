//! The pictures a style is shown back in (`docs/ai.md` rule 15): what was read, a typical reply
//! as a miniature letter, and greetings and sign-offs as bars. Every value is the core's, drawn
//! final: nothing here counts up or computes a line. A style's text is the model's, so it is
//! always set as text, never as markup.

use std::{
    f64::consts::{FRAC_PI_2, PI},
    sync::Once,
};

use adw::prelude::*;
use gtk::accessible::{Property, State};
use mailcal_bindings::{
    WritingStyleFacts, WritingStyleHabit, WritingStyleHabits, WritingStyleLetter, WritingStyleSpan,
};

use super::writing_style_words as words;
use crate::l10n;

const REVEAL_CSS: &str = "
.mailcal-reveal-letter { padding: 20px 22px 22px 22px; }
.mailcal-reveal-card { padding: 14px 16px 16px 16px; }
.mailcal-reveal-greeting { font-family: serif; font-size: 1.5em; font-weight: 500; }
.mailcal-reveal-serif { font-family: serif; font-size: 1.1em; }
.mailcal-reveal-signs-as { font-family: serif; font-size: 1.3em; font-weight: 500; }
.mailcal-reveal-habit { font-size: 1.05em; }
.mailcal-reveal-pill {
  background-color: alpha(@accent_bg_color, 0.15);
  color: @accent_color;
  border-radius: 999px;
  padding: 0 7px;
  margin: 0 2px;
  font-size: 0.8em;
  font-weight: 600;
}
.mailcal-reveal-accent { color: @accent_color; }
.mailcal-reveal-stat { padding: 0 20px; }
.mailcal-reveal-stat:first-child { padding-left: 0; }
.mailcal-reveal-language {
  border-radius: 999px;
  padding: 4px 12px 4px 10px;
  background-color: alpha(currentColor, 0.08);
}
.mailcal-reveal-dot { border-radius: 999px; min-width: 9px; min-height: 9px; }
.mailcal-reveal-dot-0 { background-color: @accent_bg_color; }
.mailcal-reveal-dot-1 { background-color: @purple_2; }
.mailcal-reveal-dot-2 { background-color: @green_3; }
.mailcal-reveal-dot-3 { background-color: @orange_2; }
.mailcal-reveal-dot-4 { background-color: @red_2; }
.mailcal-reveal-dot-5 { background-color: @yellow_4; }
.mailcal-reveal-chip { border-radius: 999px; padding: 5px 14px; }
.mailcal-reveal-avoid {
  background-color: alpha(currentColor, 0.06);
  color: alpha(currentColor, 0.65);
}
";

/// One colour per language, in the order the style lists them.
const LANGUAGE_DOTS: [&str; 6] = [
    "mailcal-reveal-dot-0",
    "mailcal-reveal-dot-1",
    "mailcal-reveal-dot-2",
    "mailcal-reveal-dot-3",
    "mailcal-reveal-dot-4",
    "mailcal-reveal-dot-5",
];

/// Installs the reveal's stylesheet once per process.
pub(super) fn install_styles() {
    static INSTALLED: Once = Once::new();
    if let Some(display) = gtk::gdk::Display::default() {
        INSTALLED.call_once(|| {
            let provider = gtk::CssProvider::new();
            provider.load_from_string(REVEAL_CSS);
            gtk::style_context_add_provider_for_display(
                &display,
                &provider,
                gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
            );
        });
    }
}

/// What was read: how many messages, since when, in which languages, from which address; with
/// `ahead`, a word that pages about each language follow.
pub(super) fn read(body: &gtk::Box, facts: &WritingStyleFacts, ahead: bool) {
    let lead = text(&l10n::reveal_based_on(&facts.learned_from));
    lead.add_css_class("dim-label");
    body.append(&lead);
    let stats = gtk::Box::builder()
        .orientation(gtk::Orientation::Horizontal)
        .accessible_role(gtk::AccessibleRole::Group)
        .halign(gtk::Align::Start)
        .build();
    stats.append(&stat(
        l10n::reveal_stat_messages(),
        &facts.messages.to_string(),
    ));
    stats.append(&gtk::Separator::new(gtk::Orientation::Vertical));
    stats.append(&stat(
        l10n::reveal_stat_languages(),
        &facts.languages.len().to_string(),
    ));
    if let Some(since) = facts.since.map(words::day_of) {
        stats.append(&gtk::Separator::new(gtk::Orientation::Vertical));
        stats.append(&stat(l10n::reveal_stat_since(), &since));
        // Read as one sentence: the figures alone are numbers without their words.
        let sentence = l10n::reveal_stats_a11y(
            i64::from(facts.messages),
            &since,
            &words::languages(&facts.languages),
        );
        let mut child = stats.first_child();
        while let Some(figure) = child {
            figure.update_state(&[State::Hidden(true)]);
            child = figure.next_sibling();
        }
        stats.update_property(&[Property::Label(&sentence)]);
    }
    body.append(&stats);

    let languages = adw::WrapBox::new();
    languages.set_child_spacing(8);
    languages.set_line_spacing(8);
    for (index, language) in facts.languages.iter().enumerate() {
        let chip = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        chip.add_css_class("mailcal-reveal-language");
        let dot = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        dot.add_css_class("mailcal-reveal-dot");
        dot.add_css_class(LANGUAGE_DOTS[index % LANGUAGE_DOTS.len()]);
        dot.set_valign(gtk::Align::Center);
        chip.append(&dot);
        chip.append(&gtk::Label::new(Some(&l10n::language_name(language))));
        languages.append(&chip);
    }
    // The figures above already name the languages.
    languages
        .upcast_ref::<gtk::Widget>()
        .update_state(&[State::Hidden(true)]);
    body.append(&languages);

    if ahead {
        let intro = text(l10n::reveal_pages_intro());
        intro.add_css_class("dim-label");
        intro.set_margin_top(12);
        body.append(&intro);
    }
}

fn stat(label: &str, value: &str) -> gtk::Box {
    let column = gtk::Box::new(gtk::Orientation::Vertical, 2);
    column.add_css_class("mailcal-reveal-stat");
    let caption = gtk::Label::new(Some(label));
    caption.set_xalign(0.0);
    caption.add_css_class("caption-heading");
    caption.add_css_class("dim-label");
    let figure = gtk::Label::new(Some(value));
    figure.set_xalign(0.0);
    figure.add_css_class("title-2");
    figure.add_css_class("numeric");
    column.append(&caption);
    column.append(&figure);
    column
}

/// A typical reply as a miniature letter, above its length and shape.
pub(super) fn letter(body: &gtk::Box, letter: &WritingStyleLetter) {
    let paper = gtk::Box::new(gtk::Orientation::Vertical, 0);
    paper.add_css_class("card");
    paper.add_css_class("mailcal-reveal-letter");
    if !letter.opening.is_empty() {
        let opening = runs(&letter.opening, "mailcal-reveal-greeting");
        opening.set_margin_bottom(14);
        paper.append(&opening);
    }
    paper.append(&letter_area(
        letter
            .paragraphs
            .iter()
            .map(|paragraph| paragraph.lines.clone())
            .collect(),
    ));
    let signs_as = letter.signs_as.trim();
    if !letter.closing.is_empty() || !signs_as.is_empty() {
        let closing = gtk::Box::new(gtk::Orientation::Vertical, 2);
        closing.set_margin_top(22);
        if !letter.closing.is_empty() {
            closing.append(&runs(&letter.closing, "mailcal-reveal-serif"));
        }
        if !signs_as.is_empty() {
            let name = text(signs_as);
            name.add_css_class("mailcal-reveal-serif");
            closing.append(&name);
        }
        paper.append(&closing);
    }
    let caption = text(l10n::reveal_letter_caption());
    caption.add_css_class("caption");
    caption.add_css_class("dim-label");
    let column = gtk::Box::new(gtk::Orientation::Vertical, 8);
    column.append(&paper);
    column.append(&caption);
    body.append(&column);
    if letter.words > 0 {
        body.append(&length(letter.words));
    }
    fact(body, l10n::reveal_shape(), &letter.shape);
}

/// "Usually about 70 words", the number drawn large inside the sentence the catalog words.
fn length(words: u32) -> gtk::Label {
    let count = i64::from(words);
    let sentence = l10n::reveal_length(count);
    let number = count.to_string();
    let label = text("");
    match sentence.split_once(number.as_str()) {
        Some((before, after)) => label.set_markup(&format!(
            "{}<span size=\"xx-large\" weight=\"bold\">{number}</span>{}",
            gtk::glib::markup_escape_text(before),
            gtk::glib::markup_escape_text(after)
        )),
        None => label.set_text(&sentence),
    }
    label
}

/// A label and what was noticed under it, or nothing for an empty value.
pub(super) fn fact(body: &gtk::Box, label: &str, value: &str) {
    let value = value.trim();
    if value.is_empty() {
        return;
    }
    let column = gtk::Box::new(gtk::Orientation::Vertical, 3);
    let caption = text(label);
    caption.add_css_class("caption-heading");
    caption.add_css_class("dim-label");
    column.append(&caption);
    column.append(&text(value));
    body.append(&column);
}

/// The letter's grey lines, each as wide as its share of the letter.
fn letter_area(paragraphs: Vec<Vec<f64>>) -> gtk::DrawingArea {
    const LINE: f64 = 7.0;
    const GAP: f64 = 7.0;
    const PARAGRAPH: f64 = 12.0;
    let rows = i32::try_from(paragraphs.iter().map(Vec::len).sum::<usize>()).unwrap_or(12);
    let count = i32::try_from(paragraphs.len()).unwrap_or(6).max(1);
    let area = gtk::DrawingArea::builder()
        .accessible_role(gtk::AccessibleRole::Presentation)
        .content_height((rows * 14 - count * 7 + (count - 1) * 12).max(7))
        .hexpand(true)
        .build();
    area.set_draw_func(move |area, context, width, _| {
        set_colour(context, &area.color(), 0.16);
        let width = f64::from(width);
        let mut top = 0.0;
        for paragraph in &paragraphs {
            for share in paragraph {
                rounded(
                    context,
                    top,
                    width * share.clamp(0.0, 1.0),
                    LINE,
                    LINE / 2.0,
                );
                top += LINE + GAP;
            }
            top += PARAGRAPH - GAP;
        }
        let _ = context.fill();
    });
    area
}

/// Greetings and sign-offs, each over a bar as long as its part of its list.
pub(super) fn habits(body: &gtk::Box, habits: &WritingStyleHabits) {
    habit_group(body, l10n::reveal_greetings(), &habits.greetings);
    habit_group(body, l10n::reveal_sign_offs(), &habits.sign_offs);
    let signs_as = habits.signs_as.trim();
    if !signs_as.is_empty() {
        let line = gtk::Box::new(gtk::Orientation::Horizontal, 12);
        let caption = gtk::Label::new(Some(l10n::reveal_signs_as()));
        caption.add_css_class("dim-label");
        caption.set_valign(gtk::Align::Center);
        let name = text(signs_as);
        name.add_css_class("mailcal-reveal-signs-as");
        line.append(&caption);
        line.append(&name);
        body.append(&line);
    }
}

fn habit_group(body: &gtk::Box, label: &str, habits: &[WritingStyleHabit]) {
    if habits.is_empty() {
        return;
    }
    let group = gtk::Box::new(gtk::Orientation::Vertical, 6);
    group.append(&heading(label));
    for (index, habit) in habits.iter().enumerate() {
        group.append(&habit_bar(habit, index == 0));
    }
    body.append(&group);
}

/// One habit: its words and how often, over a bar as long as its part of its list. The most used
/// is drawn in the accent.
fn habit_bar(habit: &WritingStyleHabit, top: bool) -> gtk::Overlay {
    let share = f64::from(habit.relative.min(100)) / 100.0;
    let bar = gtk::DrawingArea::builder()
        .accessible_role(gtk::AccessibleRole::Presentation)
        .content_height(36)
        .hexpand(true)
        .build();
    if top {
        bar.add_css_class("mailcal-reveal-accent");
    }
    bar.set_draw_func(move |area, context, width, height| {
        let colour = area.color();
        let (width, height) = (f64::from(width), f64::from(height));
        set_colour(context, &colour, 0.06);
        rounded(context, 0.0, width, height, 9.0);
        let _ = context.fill();
        set_colour(context, &colour, if top { 0.22 } else { 0.12 });
        rounded(context, 0.0, width * share, height, 9.0);
        let _ = context.fill();
    });
    let line = gtk::Box::new(gtk::Orientation::Horizontal, 12);
    line.set_margin_start(12);
    line.set_margin_end(12);
    line.set_margin_top(4);
    line.set_margin_bottom(4);
    let spoken = runs(&habit.runs, "mailcal-reveal-habit");
    spoken.set_hexpand(true);
    spoken.set_valign(gtk::Align::Center);
    // One word that never wraps: a wrapping label beside an expanding one is given its minimum,
    // and the column it then makes pushes the greeting off its bar.
    let frequency = gtk::Label::new(Some(words::frequency(habit.frequency)));
    frequency.add_css_class("caption");
    frequency.add_css_class("dim-label");
    frequency.set_valign(gtk::Align::Center);
    line.append(&spoken);
    line.append(&frequency);
    let overlay = gtk::Overlay::new();
    overlay.set_child(Some(&bar));
    overlay.add_overlay(&line);
    overlay.set_measure_overlay(&line, true);
    overlay
}

/// A greeting or sign-off, a placeholder drawn as a small pill in the accent.
pub(super) fn runs(spans: &[WritingStyleSpan], class: &str) -> gtk::Box {
    let line = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    for span in spans {
        let label = gtk::Label::new(Some(&span.text));
        label.set_valign(gtk::Align::Center);
        label.add_css_class(if span.placeholder {
            "mailcal-reveal-pill"
        } else {
            class
        });
        line.append(&label);
    }
    line
}

/// Text that wraps, from the leading edge.
pub(super) fn text(value: &str) -> gtk::Label {
    let label = gtk::Label::new(Some(value));
    label.set_wrap(true);
    label.set_wrap_mode(gtk::pango::WrapMode::WordChar);
    label.set_xalign(0.0);
    label.set_hexpand(true);
    label
}

/// A small heading, which a screen reader announces as one.
pub(super) fn heading(value: &str) -> gtk::Label {
    let label = gtk::Label::builder()
        .accessible_role(gtk::AccessibleRole::Heading)
        .label(value)
        .xalign(0.0)
        .wrap(true)
        .build();
    label.add_css_class("heading");
    label
}

fn set_colour(context: &gtk::cairo::Context, colour: &gtk::gdk::RGBA, alpha: f64) {
    context.set_source_rgba(
        f64::from(colour.red()),
        f64::from(colour.green()),
        f64::from(colour.blue()),
        alpha,
    );
}

/// A rectangle from the leading edge, `width` wide, with rounded corners.
fn rounded(context: &gtk::cairo::Context, top: f64, width: f64, height: f64, radius: f64) {
    let radius = radius.min(height / 2.0).min(width / 2.0);
    if width <= 0.0 {
        return;
    }
    context.new_sub_path();
    context.arc(width - radius, top + radius, radius, -FRAC_PI_2, 0.0);
    context.arc(
        width - radius,
        top + height - radius,
        radius,
        0.0,
        FRAC_PI_2,
    );
    context.arc(radius, top + height - radius, radius, FRAC_PI_2, PI);
    context.arc(radius, top + radius, radius, PI, PI + FRAC_PI_2);
    context.close_path();
}
