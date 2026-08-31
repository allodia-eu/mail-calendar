//! The event detail's attendee list.
//!
//! Read-only, like every other client: changing an attendee list means sending iTIP updates to the
//! people on it, which is a separate feature.

use adw::prelude::*;
use mailcal_bindings::{EventAttendee, ResponseStatus};

use crate::l10n;

/// The attendee list under its own heading, or `None` when nobody was invited.
///
/// `use_markup(false)`, like every other row here: a display name is attacker-controlled, and an
/// ampersand in one would otherwise be parsed as Pango markup and swallow the rest of the line.
pub(super) fn attendee_group(attendees: &[EventAttendee]) -> Option<adw::PreferencesGroup> {
    if attendees.is_empty() {
        return None;
    }
    let group = adw::PreferencesGroup::builder()
        .title(l10n::event_attendees())
        .build();
    for attendee in attendees {
        group.add(&attendee_row(attendee));
    }
    Some(group)
}

/// The second line under an attendee: their address (when the first line used their name) and
/// whether they called the meeting. Empty when there is nothing left to say.
///
/// A plain function, so the rule is testable without building a row; the twin of Android's
/// `attendeeSubtitle`.
fn attendee_subtitle(attendee: &EventAttendee) -> String {
    let mut parts: Vec<&str> = Vec::new();
    // Only when the title used the name: an attendee with no display name is already shown by
    // address, and repeating it here would print the same address twice.
    if !attendee.name.is_empty() {
        parts.push(&attendee.email);
    }
    if attendee.is_organizer {
        parts.push(l10n::event_attendee_organizer());
    }
    parts.join(" · ")
}

fn attendee_row(attendee: &EventAttendee) -> adw::ActionRow {
    // Setters, not the property builder: `g_object_new` applies properties in its own order, so
    // `.use_markup(false)` written last still lands after the title, and the markup-parsed first
    // attempt has already logged for every name carrying an ampersand (`../../AGENTS.md` →
    // Client conventions). libadwaita re-applies the labels when the flag flips, so the row reads
    // correctly either way and only the log record tells them apart.
    let row = crate::ui::mailbox::plain_text_row();
    row.set_title(if attendee.name.is_empty() {
        &attendee.email
    } else {
        &attendee.name
    });
    row.set_subtitle(&attendee_subtitle(attendee));
    let answer = gtk::Label::new(Some(attendee_response(attendee.response)));
    answer.add_css_class("dim-label");
    row.add_suffix(&answer);
    row
}

/// How one attendee answered. Third person; somebody else's answer, unlike the invitation card's
/// "You accepted".
fn attendee_response(response: ResponseStatus) -> &'static str {
    match response {
        ResponseStatus::Accepted => l10n::event_attendee_accepted(),
        ResponseStatus::Declined => l10n::event_attendee_declined(),
        ResponseStatus::Tentative => l10n::event_attendee_tentative(),
        ResponseStatus::Delegated => l10n::event_attendee_delegated(),
        ResponseStatus::NeedsAction => l10n::event_attendee_needs_action(),
    }
}

#[cfg(test)]
#[path = "attendees_tests.rs"]
pub(crate) mod tests;
