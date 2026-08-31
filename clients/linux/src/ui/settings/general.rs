//! The **General** settings page: language, appearance, time zone and the 12/24-hour clock.
//!
//! Its own module because it is the one page with a host-owned setting beside the core-owned ones,
//! the language lives in `HostPreferences`, the rest in the core: and because its sibling `pages`
//! was at the 500-line limit.

use adw::prelude::*;
use mailcal_bindings::{Intent, TimeFormat, available_time_zones};

use super::{PageContext, choice, group, page_box};
use crate::{appearance, l10n};

pub(super) fn general(ctx: &PageContext) -> gtk::Box {
    let content = page_box(l10n::settings_category_general());
    let display = ctx.app.display_settings();
    let language = group(
        l10n::settings_language_heading(),
        l10n::settings_language_description(),
    );
    // Built from the catalog list the l10n generator emits, never a list kept here: a hand-kept
    // one is how a client silently stops offering languages the catalog ships; this picker sat at
    // English and Nederlands through five added languages. Index 0 is "System"; every locale
    // follows in catalog order, each labelled by its own endonym (`settings_language_<code>`,
    // which codegen requires every locale to carry).
    let mut language_labels = vec![l10n::settings_language_system().to_owned()];
    language_labels.extend(l10n::LOCALES.iter().map(|code| l10n::language_name(code)));
    let labels = language_labels
        .iter()
        .map(String::as_str)
        .collect::<Vec<_>>();
    let current = ctx
        .preferences
        .language()
        .and_then(|stored| l10n::LOCALES.iter().position(|code| *code == stored))
        .map_or(0, |index| u32::try_from(index + 1).unwrap_or(0));
    let (row, picker) = choice(l10n::settings_language_heading(), &labels, current);
    let preferences = ctx.preferences.clone();
    let changed = row.clone();
    picker.connect_selected_notify(move |picker| {
        let selected = picker.selected() as usize;
        let locale = (selected > 0)
            .then(|| l10n::LOCALES.get(selected - 1).copied())
            .flatten();
        preferences.set_language(locale);
        changed.set_subtitle(l10n::settings_language_restart_message());
    });
    language.add(&row);
    content.append(&language);

    let scheme = group(
        l10n::settings_appearance_heading(),
        l10n::settings_appearance_description(),
    );
    let (row, picker) = choice(
        l10n::settings_appearance_heading(),
        &appearance::PICKER_ORDER.map(appearance::label),
        appearance::selection(display.appearance),
    );
    let app = ctx.app.clone();
    picker.connect_selected_notify(move |picker| {
        let chosen = appearance::chosen(picker.selected());
        app.set_appearance(chosen);
        // The core signals Settings alone for this; it computes nothing from the appearance; so
        // repainting is this page's job.
        appearance::apply(chosen);
    });
    scheme.add(&row);
    content.append(&scheme);

    let timezone = group(
        l10n::tz_picker_title(),
        l10n::settings_timezone_description(),
    );
    let settings = ctx.app.timezone_settings();
    let zones = available_time_zones();
    let selected = zones
        .iter()
        .position(|zone| zone == &settings.active)
        .and_then(|index| u32::try_from(index).ok())
        .unwrap_or(0);
    let labels = zones.iter().map(String::as_str).collect::<Vec<_>>();
    let (row, picker) = choice(l10n::tz_picker_title(), &labels, selected);
    let app = ctx.app.clone();
    picker.connect_selected_notify(move |picker| {
        if let Some(zone) = zones.get(picker.selected() as usize) {
            app.dispatch(Intent::SetTimeZone { id: zone.clone() });
        }
    });
    timezone.add(&row);
    content.append(&timezone);

    let clock = group(
        l10n::settings_time_format_heading(),
        l10n::settings_time_format_description(),
    );
    let selected = match display.time_format {
        TimeFormat::TwentyFourHour => 0,
        TimeFormat::TwelveHour => 1,
    };
    let (row, picker) = choice(
        l10n::settings_time_format_heading(),
        &[
            l10n::settings_time_format_24(),
            l10n::settings_time_format_12(),
        ],
        selected,
    );
    let app = ctx.app.clone();
    picker.connect_selected_notify(move |picker| {
        app.set_time_format(if picker.selected() == 0 {
            TimeFormat::TwentyFourHour
        } else {
            TimeFormat::TwelveHour
        });
    });
    clock.add(&row);
    content.append(&clock);
    content
}

/// The appearance row has to be **on the page**, showing the choice the core stored; a picker that
/// silently opens on the first entry tells a user who chose Dark that they never did.
///
/// Asserted on the labels the row actually displays, rather than on `ActionRow::title()` /
/// `DropDown::selected()`: both read back what they were handed whatever became of the widget, so a
/// property assertion is a green light for a blank row. The visibility filter is not decoration,
/// a `DropDown` keeps **every** entry's label in its popover, so a plain tree walk finds "Dark"
/// whichever entry is selected, and the assertion passes for a picker sitting on the wrong one.
///
/// A function rather than a `#[test]` for the reason [`crate::ui::mailbox`]'s row tests give: GTK
/// initialises once, on one thread, and the crate keeps a single GTK test.
#[cfg(test)]
pub(crate) fn assert_the_appearance_row_shows_the_stored_choice() {
    use mailcal_bindings::Appearance;

    use crate::ui::mailbox::tests::labels;

    let window = gtk::Window::new();
    let (row, picker) = choice(
        l10n::settings_appearance_heading(),
        &appearance::PICKER_ORDER.map(appearance::label),
        appearance::selection(Appearance::Dark),
    );
    // In a group, as the page builds it: a row alone in a window belongs to no list, and
    // presenting one fails `gtk_list_box_row_grab_focus`'s precondition on the focus walk.
    let holder = adw::PreferencesGroup::new();
    holder.add(&row);
    window.set_child(Some(&holder));
    window.present();
    crate::ui::mailbox::tests::every_row_belongs_to_a_list(window.upcast_ref::<gtk::Widget>());
    let displayed = labels(row.upcast_ref::<gtk::Widget>())
        .iter()
        .filter(|label| label.is_visible())
        .map(|label| label.text().to_string())
        .collect::<Vec<_>>();

    assert!(
        displayed
            .iter()
            .any(|text| text == l10n::settings_appearance_heading()),
        "the row must name the setting, not render blank: {displayed:?}"
    );
    assert!(
        displayed
            .iter()
            .any(|text| text == l10n::settings_appearance_dark()),
        "a stored Dark must be the entry the closed picker shows: {displayed:?}"
    );
    assert!(
        !displayed
            .iter()
            .any(|text| text == l10n::settings_appearance_system()),
        "the picker is showing the first entry rather than the stored one: {displayed:?}"
    );
    assert_eq!(appearance::chosen(picker.selected()), Appearance::Dark);
}
