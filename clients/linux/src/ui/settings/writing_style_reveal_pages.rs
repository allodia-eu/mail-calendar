//! The reveal's first three pages: what was read, a typical reply as a miniature letter, and the
//! greetings and sign-offs as bars. What each picture is drawn from is
//! `crate::ui::writing_style_reveal`; a style's text is the model's, so it is never markup.

use std::{
    f64::consts::{FRAC_PI_2, PI},
    sync::Once,
};

use adw::prelude::*;
use gtk::{
    accessible::{Property, State},
    glib,
};
use mailcal_bindings::{HabitRow, LanguageStyleRow, WritingStyleDetail};

use crate::{
    l10n,
    ui::{
        timestamps,
        writing_style_reveal::{frequency_text, letter_lines, reveal_runs, stats_label},
    },
};

const REVEAL_CSS: &str = "
.mailcal-reveal-letter { padding: 16px 18px 18px 18px; }
.mailcal-reveal-card { padding: 13px 16px 14px 16px; }
.mailcal-reveal-greeting { font-family: serif; font-size: 1.6em; font-weight: 500; }
.mailcal-reveal-serif { font-family: serif; font-size: 1.1em; }
.mailcal-reveal-signs-as { font-family: serif; font-size: 1.3em; font-weight: 500; }
.mailcal-reveal-habit { font-size: 1.05em; }
.mailcal-reveal-pill {
  background-color: alpha(@accent_bg_color, 0.15);
  color: @accent_color;
  border-radius: 999px;
  padding: 0 7px;
  margin: 0 1px;
  font-size: 0.8em;
  font-weight: 600;
}
.mailcal-reveal-accent { color: @accent_color; }
.mailcal-reveal-stat { padding: 0 22px; }
.mailcal-reveal-stat:first-child { padding-left: 0; }
.mailcal-reveal-language {
  border-radius: 999px;
  padding: 4px 12px 4px 10px;
  background-color: alpha(currentColor, 0.08);
}
.mailcal-reveal-dot { border-radius: 999px; min-width: 8px; min-height: 8px; }
.mailcal-reveal-dot-0 { background-color: @accent_bg_color; }
.mailcal-reveal-dot-1 { background-color: @purple_3; }
.mailcal-reveal-dot-2 { background-color: @green_4; }
.mailcal-reveal-dot-3 { background-color: @orange_3; }
.mailcal-reveal-dot-4 { background-color: @red_3; }
.mailcal-reveal-dot-5 { background-color: @yellow_5; }
.mailcal-reveal-chip { border-radius: 999px; padding: 4px 14px; }
.mailcal-reveal-avoid {
  text-decoration-line: line-through;
  text-decoration-color: alpha(@error_color, 0.7);
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

/// Installs the reveal's stylesheet once per display.
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

/// Step 1: how many messages in how many languages since when, from which account.
pub(super) fn read(
    body: &gtk::Box,
    detail: &WritingStyleDetail,
    address: Option<&str>,
    zone: &str,
) {
    let locale = l10n::active_locale();
    if let Some(address) = address {
        let lead = text(&l10n::reveal_based_on(address));
        lead.add_css_class("dim-label");
        body.append(&lead);
    }
    let stats = gtk::Box::builder()
        .orientation(gtk::Orientation::Horizontal)
        .accessible_role(gtk::AccessibleRole::Group)
        .halign(gtk::Align::Start)
        .build();
    stats.append(&stat(
        l10n::reveal_stat_messages(),
        &detail.row.messages.to_string(),
    ));
    stats.append(&gtk::Separator::new(gtk::Orientation::Vertical));
    stats.append(&stat(
        l10n::reveal_stat_languages(),
        &detail.languages.len().to_string(),
    ));
    if let Some(since) = detail
        .row
        .oldest
        .and_then(|oldest| timestamps::since_figure(oldest, jiff::Timestamp::now(), zone, locale))
    {
        stats.append(&gtk::Separator::new(gtk::Orientation::Vertical));
        stats.append(&stat(l10n::reveal_stat_since(), &since));
    }
    // Read as one sentence when there is a date to say it with: the figures alone are numbers
    // without their words.
    if let Some(sentence) = stats_label(detail, zone, locale) {
        let mut child = stats.first_child();
        while let Some(figure) = child {
            figure.update_state(&[State::Hidden(true)]);
            child = figure.next_sibling();
        }
        stats.update_property(&[Property::Label(&sentence)]);
    }
    body.append(&stats);

    let languages = gtk::FlowBox::new();
    languages.set_selection_mode(gtk::SelectionMode::None);
    languages.set_max_children_per_line(12);
    languages.set_column_spacing(8);
    languages.set_row_spacing(8);
    languages.set_halign(gtk::Align::Start);
    for (index, language) in detail.languages.iter().enumerate() {
        let chip = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        chip.add_css_class("mailcal-reveal-language");
        let dot = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        dot.add_css_class("mailcal-reveal-dot");
        dot.add_css_class(LANGUAGE_DOTS[index % LANGUAGE_DOTS.len()]);
        dot.set_valign(gtk::Align::Center);
        chip.append(&dot);
        chip.append(&gtk::Label::new(Some(&l10n::language_name(
            &language.language,
        ))));
        languages.append(&flow_child(&chip));
    }
    // The figures above already name the languages.
    languages.update_state(&[State::Hidden(true)]);
    body.append(&languages);

    let intro = text(l10n::reveal_pages_intro());
    intro.add_css_class("dim-label");
    intro.set_margin_top(12);
    body.append(&intro);
}

fn stat(label: &str, value: &str) -> gtk::Box {
    let column = gtk::Box::new(gtk::Orientation::Vertical, 2);
    column.add_css_class("mailcal-reveal-stat");
    let caption = text(label);
    caption.add_css_class("caption-heading");
    caption.add_css_class("dim-label");
    let figure = text(value);
    figure.add_css_class("title-1");
    figure.add_css_class("numeric");
    column.append(&caption);
    column.append(&figure);
    column
}

/// Step 2: a typical reply as a miniature letter, above its length, shape and punctuation.
pub(super) fn letter(body: &gtk::Box, style: &LanguageStyleRow) {
    let letter = gtk::Box::new(gtk::Orientation::Vertical, 0);
    letter.add_css_class("card");
    letter.add_css_class("mailcal-reveal-letter");
    if let Some(greeting) = style.greetings.first() {
        let opening = runs(&greeting.text, "mailcal-reveal-greeting");
        opening.set_margin_bottom(12);
        letter.append(&opening);
    }
    letter.append(&letter_area(letter_lines(
        style.typical_words,
        style.typical_paragraphs,
    )));
    let signs_as = style.signs_as.trim();
    if !style.sign_offs.is_empty() || !signs_as.is_empty() {
        let closing = gtk::Box::new(gtk::Orientation::Vertical, 2);
        closing.set_margin_top(22);
        if let Some(sign_off) = style.sign_offs.first() {
            closing.append(&runs(&sign_off.text, "mailcal-reveal-serif"));
        }
        if !signs_as.is_empty() {
            let name = text(signs_as);
            name.add_css_class("mailcal-reveal-serif");
            closing.append(&name);
        }
        letter.append(&closing);
    }
    let caption = text(l10n::reveal_letter_caption());
    caption.add_css_class("caption");
    caption.add_css_class("dim-label");
    let column = gtk::Box::new(gtk::Orientation::Vertical, 8);
    column.append(&letter);
    column.append(&caption);
    body.append(&column);

    if style.typical_words > 0 {
        body.append(&length(style.typical_words));
    }
    fact(body, l10n::reveal_shape(), &style.shape);
    fact(body, l10n::reveal_punctuation(), &style.punctuation);
}

/// "Usually about 70 words", the number drawn large in the sentence the catalog words.
fn length(words: u32) -> gtk::Label {
    let count = i64::from(words);
    let sentence = l10n::reveal_length(count);
    let number = count.to_string();
    let label = text("");
    match sentence.split_once(number.as_str()) {
        Some((before, after)) => label.set_markup(&format!(
            "{}<span size=\"xx-large\" weight=\"bold\">{number}</span>{}",
            glib::markup_escape_text(before),
            glib::markup_escape_text(after)
        )),
        None => label.set_text(&sentence),
    }
    label
}

/// A label and what was noticed under it, or nothing for a field the model left empty.
fn fact(body: &gtk::Box, label: &str, value: &str) {
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

/// The letter's grey lines, as wide as the share `lines` gives each of the letter's width.
fn letter_area(lines: Vec<Vec<f64>>) -> gtk::DrawingArea {
    const LINE: f64 = 7.0;
    const GAP: f64 = 7.0;
    const PARAGRAPH: f64 = 12.0;
    let rows = i32::try_from(lines.iter().map(Vec::len).sum::<usize>()).unwrap_or(12);
    let paragraphs = i32::try_from(lines.len()).unwrap_or(6).max(1);
    let area = gtk::DrawingArea::builder()
        .accessible_role(gtk::AccessibleRole::Presentation)
        .content_height(rows * 14 - paragraphs * 7 + (paragraphs - 1) * 12)
        .hexpand(true)
        .build();
    area.set_draw_func(move |area, context, width, _| {
        set_colour(context, &area.color(), 0.16);
        let width = f64::from(width);
        let mut top = 0.0;
        for paragraph in &lines {
            for share in paragraph {
                rounded(context, top, width * share, LINE, LINE / 2.0);
                top += LINE + GAP;
            }
            top += PARAGRAPH - GAP;
        }
        let _ = context.fill();
    });
    area
}

/// Step 3: greetings and sign-offs, each over a bar as long as its part of its list.
pub(super) fn habits(body: &gtk::Box, style: &LanguageStyleRow) {
    habit_group(body, l10n::reveal_greetings(), &style.greetings);
    habit_group(body, l10n::reveal_sign_offs(), &style.sign_offs);
    let signs_as = style.signs_as.trim();
    if !signs_as.is_empty() {
        let line = gtk::Box::new(gtk::Orientation::Horizontal, 12);
        let caption = text(l10n::reveal_signs_as());
        caption.set_hexpand(false);
        caption.add_css_class("dim-label");
        caption.set_valign(gtk::Align::Center);
        let name = text(signs_as);
        name.add_css_class("mailcal-reveal-signs-as");
        line.append(&caption);
        line.append(&name);
        body.append(&line);
    }
}

fn habit_group(body: &gtk::Box, label: &str, habits: &[HabitRow]) {
    let habits = habits
        .iter()
        .filter(|habit| !habit.text.trim().is_empty())
        .collect::<Vec<_>>();
    if habits.is_empty() {
        return;
    }
    let group = gtk::Box::new(gtk::Orientation::Vertical, 6);
    group.append(&heading(label));
    for (index, habit) in habits.into_iter().enumerate() {
        group.append(&habit_bar(habit, index == 0));
    }
    body.append(&group);
}

/// One habit: its words and how often, over a bar as long as its part of its list. The first,
/// most used, is drawn in the accent.
fn habit_bar(habit: &HabitRow, top: bool) -> gtk::Overlay {
    let share = f64::from(habit.relative.min(100)) / 100.0;
    let bar = gtk::DrawingArea::builder()
        .accessible_role(gtk::AccessibleRole::Presentation)
        .content_height(34)
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
    let words = runs(habit.text.trim(), "mailcal-reveal-habit");
    words.set_hexpand(true);
    words.set_valign(gtk::Align::Center);
    // One word that never wraps: a wrapping label beside an expanding one is given its minimum,
    // a single character, and the column it then makes pushes the greeting off its bar.
    let frequency = gtk::Label::new(Some(frequency_text(habit.frequency)));
    frequency.add_css_class("caption");
    frequency.add_css_class("dim-label");
    frequency.set_valign(gtk::Align::Center);
    line.append(&words);
    line.append(&frequency);
    let overlay = gtk::Overlay::new();
    overlay.set_child(Some(&bar));
    overlay.add_overlay(&line);
    overlay.set_measure_overlay(&line, true);
    overlay
}

/// A greeting or sign-off, a placeholder such as `[Name]` drawn as a small pill in the accent.
pub(super) fn runs(words: &str, class: &str) -> gtk::Box {
    let line = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    for run in reveal_runs(words) {
        let label = gtk::Label::new(Some(&run.text));
        label.set_valign(gtk::Align::Center);
        label.add_css_class(if run.placeholder {
            "mailcal-reveal-pill"
        } else {
            class
        });
        line.append(&label);
    }
    line
}

/// Text that wraps, from the leading edge. Never markup: set as text.
pub(super) fn text(value: &str) -> gtk::Label {
    let label = gtk::Label::new(Some(value));
    label.set_wrap(true);
    label.set_wrap_mode(gtk::pango::WrapMode::WordChar);
    label.set_xalign(0.0);
    label.set_hexpand(true);
    label
}

/// A small heading, one a screen reader announces as a heading.
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

/// A child for a flow box that the keyboard passes over: the chips are words, not controls.
pub(super) fn flow_child(content: &impl IsA<gtk::Widget>) -> gtk::FlowBoxChild {
    let child = gtk::FlowBoxChild::new();
    child.set_child(Some(content));
    child.set_focusable(false);
    child
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
