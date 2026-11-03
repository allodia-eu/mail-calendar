//! An event's video meetings in its detail (`docs/conferencing.md`): Join first, then what a
//! person needs to get in another way, the meeting ID, the passcode and the numbers to dial. The
//! decisions are `meeting.rs`.
//!
//! Every string here came from whoever wrote the event, so every row is built with markup off, and
//! the links open through the portal launcher on a click, never in a view of the app's own.
//! Nothing here logs: a Zoom link carries its passcode.

use adw::prelude::*;
use gtk::{accessible::Property as AccessibleProperty, gio};
use mailcal_bindings::{ConferenceDialIn, EventConference};

use super::meeting;
use crate::{l10n, ui::icons};

/// Accessible names the widget test finds a meeting's controls by. The first meeting carries them
/// bare; a later one suffixed with its place in the core's order.
pub(super) const JOIN: &str = "event-join";
pub(super) const COPY_LINK: &str = "event-meeting-copy-link";

/// One meeting, as a group of its own: its heading with Join, then each row it has a value for.
pub(super) fn conference_group(
    conference: &EventConference,
    index: usize,
) -> adw::PreferencesGroup {
    let name = |base: &str| {
        if index == 0 {
            base.to_owned()
        } else {
            format!("{base}-{index}")
        }
    };
    // Already through the link policy, and only for a meeting that is ready: the core's answer.
    let join = conference.join_url.as_deref();
    let group = adw::PreferencesGroup::new();
    group.set_margin_start(18);
    group.set_margin_end(18);

    let heading = adw::ActionRow::builder()
        .title(meeting::title(conference))
        .use_markup(false)
        .build();
    // The heading already names the meeting, so the camera says nothing of its own.
    heading.add_prefix(
        &gtk::Image::builder()
            .icon_name(icons::VIDEO_MEETING)
            .accessible_role(gtk::AccessibleRole::Presentation)
            .build(),
    );
    if let Some(status) = meeting::status(conference) {
        heading.set_subtitle(status);
    }
    if let Some(link) = join {
        let button = gtk::Button::with_label(l10n::event_join());
        button.add_css_class("suggested-action");
        button.set_valign(gtk::Align::Center);
        button.set_widget_name(&name(JOIN));
        let link = link.to_owned();
        button.connect_clicked(move |button| launch(button, &link));
        heading.add_suffix(&button);
    }
    group.add(&heading);

    // The link Join opens, so a meeting with nothing to join offers nothing to copy either.
    if let Some(link) = join {
        let copy = adw::ButtonRow::builder()
            .title(l10n::action_copy_link())
            .start_icon_name(icons::COPY)
            .build();
        copy.set_widget_name(&name(COPY_LINK));
        let link = link.to_owned();
        copy.connect_activated(move |row| copy_link(row, &link));
        group.add(&copy);
    }
    if let Some(id) = meeting::shown(conference.meeting_id.as_deref()) {
        group.add(&value_row(l10n::event_meeting_id(), id));
    }
    if let Some(passcode) = meeting::shown(conference.passcode.as_deref()) {
        group.add(&value_row(l10n::event_meeting_passcode(), passcode));
    }
    // In the core's order, one row each: a server can list one number twice with a different PIN.
    for number in &conference.dial_in {
        group.add(&dial_in_row(number));
    }
    if let Some(notes) = meeting::shown(conference.notes.as_deref()) {
        let label = gtk::Label::new(Some(notes));
        label.set_wrap(true);
        label.set_wrap_mode(gtk::pango::WrapMode::WordChar);
        label.set_xalign(0.0);
        label.set_selectable(true);
        label.add_css_class("dim-label");
        label.set_margin_top(8);
        label.set_margin_bottom(8);
        label.set_margin_start(12);
        label.set_margin_end(12);
        group.add(&label);
    }
    group
}

// A caption over a value a person may want to copy: the subtitle is selectable.
fn value_row(label: &str, value: &str) -> adw::ActionRow {
    let row = adw::ActionRow::builder()
        .title(label)
        .subtitle(value)
        .use_markup(false)
        .subtitle_selectable(true)
        .build();
    row.add_css_class("property");
    row
}

// "Join by phone" over the number and its PIN, with a button that dials only when the core gave
// the number a URI to dial; a number the link policy refused is text.
fn dial_in_row(number: &ConferenceDialIn) -> adw::ActionRow {
    let subtitle = match meeting::shown(number.pin.as_deref()) {
        Some(pin) => format!("{} · {}", number.number, l10n::event_meeting_pin(pin)),
        None => number.number.clone(),
    };
    let row = adw::ActionRow::builder()
        .title(l10n::event_meeting_dial_in())
        .subtitle(&subtitle)
        .use_markup(false)
        .subtitle_selectable(true)
        .build();
    row.add_css_class("property");
    if let Some(dial) = number.uri.as_deref() {
        let button = gtk::Button::from_icon_name(icons::DIAL_IN);
        button.add_css_class("flat");
        button.set_valign(gtk::Align::Center);
        button.set_tooltip_text(Some(&number.number));
        button.update_property(&[AccessibleProperty::Label(&number.number)]);
        let dial = dial.to_owned();
        button.connect_clicked(move |button| launch(button, &dial));
        row.add_suffix(&button);
    }
    row
}

// `GtkUriLauncher`, as every other link here uses: it goes through the portal in a sandboxed build
// and hands the link to the service's app, the browser or the dialler.
fn launch(widget: &impl IsA<gtk::Widget>, uri: &str) {
    let window = widget.root().and_downcast::<gtk::Window>();
    gtk::UriLauncher::new(uri).launch(window.as_ref(), gio::Cancellable::NONE, |_| ());
}

fn copy_link(row: &adw::ButtonRow, link: &str) {
    row.clipboard().set_text(link);
    if let Some(toasts) = row
        .ancestor(adw::ToastOverlay::static_type())
        .and_downcast::<adw::ToastOverlay>()
    {
        toasts.add_toast(adw::Toast::new(l10n::event_meeting_link_copied()));
    }
}
