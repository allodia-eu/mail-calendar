//! What a found card offers the account for: mail, calendar and contacts, each a choice the
//! core decided how to start (`docs/account-autodetect.md` rule 8), and the servers detection
//! found for them.

use adw::prelude::*;
use mailcal_bindings::{AccountCapability, SetupChoice};

use super::setup_widgets::{caption, entry, section};
use crate::l10n;

/// The choices the core offered, in its order, and the endpoints it found. Empty URLs are ones
/// detection did not find.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct UseOffer {
    pub(super) choices: Vec<SetupChoice>,
    pub(super) caldav_url: String,
    pub(super) carddav_url: String,
}

/// What the person chose, as the account is set up with it.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct ChosenUses {
    /// `None` when the card offered no choice, so the servers given decide.
    pub(super) uses: Option<Vec<AccountCapability>>,
    pub(super) caldav_url: String,
    pub(super) carddav_url: String,
}

/// One use's state on screen: whether it is on, and the URL typed for it when detection found
/// none.
#[derive(Clone, Copy, Debug)]
struct Picked<'a> {
    on: bool,
    typed: &'a str,
}

/// The account a card's choices describe.
///
/// A calendar stores the endpoint detection found, or the one typed in its place. Contacts store
/// an address book found apart from the calendar, or the one typed. Contacts found only through
/// the calendar's server keep that server even with the calendar off, because that is where they
/// are looked for, and the stored uses keep the calendar closed.
fn chosen(
    offer: &UseOffer,
    mail: Option<bool>,
    calendar: Option<Picked<'_>>,
    contacts: Option<Picked<'_>>,
) -> ChosenUses {
    let calendar_on = calendar.is_some_and(|picked| picked.on);
    let contacts_on = contacts.is_some_and(|picked| picked.on);
    let found_or_typed = |found: &str, picked: Option<Picked<'_>>| {
        if found.is_empty() {
            picked.map_or_else(String::new, |picked| picked.typed.trim().to_owned())
        } else {
            found.to_owned()
        }
    };
    let carddav_url =
        if contacts_on && (!offer.carddav_url.is_empty() || offer.caldav_url.is_empty()) {
            found_or_typed(&offer.carddav_url, contacts)
        } else {
            String::new()
        };
    let through_calendar = contacts_on && carddav_url.is_empty() && !offer.caldav_url.is_empty();
    let caldav_url = if calendar_on || through_calendar {
        found_or_typed(&offer.caldav_url, calendar)
    } else {
        String::new()
    };
    let uses = (!offer.choices.is_empty()).then(|| {
        [
            (mail == Some(true)).then_some(AccountCapability::Mail),
            calendar_on.then_some(AccountCapability::Calendar),
            contacts_on.then_some(AccountCapability::Contacts),
        ]
        .into_iter()
        .flatten()
        .collect()
    });
    ChosenUses {
        uses,
        caldav_url,
        carddav_url,
    }
}

/// A calendar or contacts choice on screen: the toggle, and beneath it the server detection
/// found, or a field for one when it found none.
struct DavToggle {
    enabled: gtk::CheckButton,
    typed: gtk::Entry,
}

/// The toggles a card drew, read back when the person connects or signs in.
pub(super) struct UseToggles {
    offer: UseOffer,
    mail: Option<gtk::CheckButton>,
    calendar: Option<DavToggle>,
    contacts: Option<DavToggle>,
}

impl UseToggles {
    /// What is chosen now.
    pub(super) fn chosen(&self) -> ChosenUses {
        fn picked(state: Option<&(bool, String)>) -> Option<Picked<'_>> {
            state.map(|(on, typed)| Picked { on: *on, typed })
        }
        let state = |toggle: &Option<DavToggle>| {
            toggle
                .as_ref()
                .map(|toggle| (toggle.enabled.is_active(), toggle.typed.text().to_string()))
        };
        let (calendar, contacts) = (state(&self.calendar), state(&self.contacts));
        chosen(
            &self.offer,
            self.mail.as_ref().map(gtk::CheckButton::is_active),
            picked(calendar.as_ref()),
            picked(contacts.as_ref()),
        )
    }
}

/// Draws one section per choice the core offered, in its order. `mail_rows` is what the mail
/// choice shows while it is on: the servers detection found.
pub(super) fn append(
    content: &gtk::Box,
    offer: &UseOffer,
    mail_rows: &[gtk::Widget],
) -> UseToggles {
    let mut toggles = UseToggles {
        offer: offer.clone(),
        mail: None,
        calendar: None,
        contacts: None,
    };
    for choice in &offer.choices {
        match choice.capability {
            AccountCapability::Mail => {
                content.append(&section(l10n::setup_detect_section_email()));
                let enabled = gtk::CheckButton::with_label(l10n::setup_detect_mail_enable());
                enabled.set_active(choice.on);
                content.append(&enabled);
                for row in mail_rows {
                    content.append(row);
                    follow(&enabled, row);
                }
                toggles.mail = Some(enabled);
            }
            AccountCapability::Calendar => {
                content.append(&section(l10n::setup_detect_section_calendar()));
                toggles.calendar = Some(dav_toggle(
                    content,
                    choice,
                    &offer.caldav_url,
                    l10n::setup_detect_calendar_enable(),
                    l10n::setup_detect_calendar_add(),
                    l10n::setup_hint_caldav(),
                ));
            }
            AccountCapability::Contacts => {
                content.append(&section(l10n::setup_detect_section_contacts()));
                let found = if offer.carddav_url.is_empty() {
                    &offer.caldav_url
                } else {
                    &offer.carddav_url
                };
                toggles.contacts = Some(dav_toggle(
                    content,
                    choice,
                    found,
                    l10n::setup_detect_contacts_enable(),
                    l10n::setup_detect_contacts_add(),
                    l10n::setup_hint_carddav(),
                ));
            }
            AccountCapability::Colleagues => {}
        }
    }
    keep_one_on(&toggles);
    toggles
}

fn dav_toggle(
    content: &gtk::Box,
    choice: &SetupChoice,
    found: &str,
    enable: &str,
    add: &str,
    hint: &str,
) -> DavToggle {
    let enabled = gtk::CheckButton::with_label(if choice.server_found { enable } else { add });
    enabled.set_active(choice.on);
    content.append(&enabled);
    let detail = caption(&url_host(found));
    content.append(&detail);
    let typed = entry(hint, "", false);
    content.append(&typed);
    // Exactly one of the two belongs to this choice, the endpoint to recognise or a box to type
    // one into, and it follows the toggle, so a use switched off leaves nothing behind claiming
    // otherwise.
    let shown: gtk::Widget = if choice.server_found {
        typed.set_visible(false);
        detail.upcast()
    } else {
        detail.set_visible(false);
        typed.clone().upcast()
    };
    follow(&enabled, &shown);
    DavToggle { enabled, typed }
}

/// Shows `widget` only while `toggle` is on.
fn follow(toggle: &gtk::CheckButton, widget: &impl IsA<gtk::Widget>) {
    let widget = widget.as_ref().clone();
    widget.set_visible(toggle.is_active());
    toggle.connect_toggled(move |toggle| widget.set_visible(toggle.is_active()));
}

/// The last use switched on cannot be switched off: an account used for nothing is not one.
fn keep_one_on(toggles: &UseToggles) {
    let all: Vec<gtk::CheckButton> = toggles
        .mail
        .iter()
        .cloned()
        .chain(toggles.calendar.iter().map(|toggle| toggle.enabled.clone()))
        .chain(toggles.contacts.iter().map(|toggle| toggle.enabled.clone()))
        .collect();
    let settle = {
        let all = all.clone();
        move || {
            let on = all.iter().filter(|toggle| toggle.is_active()).count();
            for toggle in &all {
                toggle.set_sensitive(on > 1 || !toggle.is_active());
            }
        }
    };
    settle();
    let settle = std::rc::Rc::new(settle);
    for toggle in &all {
        let settle = std::rc::Rc::clone(&settle);
        toggle.connect_toggled(move |_| settle());
    }
}

/// The host of a discovered URL, for a line the person can recognise; the whole URL when it
/// does not parse.
pub(super) fn url_host(url: &str) -> String {
    url::Url::parse(url)
        .ok()
        .and_then(|parsed| parsed.host_str().map(str::to_owned))
        .unwrap_or_else(|| url.to_owned())
}

#[cfg(test)]
#[path = "setup_uses_tests.rs"]
mod tests;
