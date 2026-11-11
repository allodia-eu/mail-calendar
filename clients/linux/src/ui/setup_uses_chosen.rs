//! What a card's choices come to: the uses an account is set up with and the servers it stores,
//! from what the person left on.

use mailcal_bindings::AccountCapability;

use super::UseOffer;

/// What the person chose, as the account is set up with it.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct ChosenUses {
    /// `None` when the card offered no choice, so the servers given decide.
    pub(in crate::ui) uses: Option<Vec<AccountCapability>>,
    pub(in crate::ui) caldav_url: String,
    pub(in crate::ui) carddav_url: String,
}

/// One use's state on screen: whether it is on, and the URL typed for it when detection found
/// none.
#[derive(Clone, Copy, Debug)]
pub(super) struct Picked<'a> {
    pub(super) on: bool,
    pub(super) typed: &'a str,
}

/// The account a card's choices describe.
///
/// A calendar stores the endpoint detection found, or the one typed in its place. Contacts store
/// an address book found apart from the calendar, or the one typed. Contacts found only through
/// the calendar's server keep that server even with the calendar off, because that is where they
/// are looked for, and the stored uses keep the calendar closed. Colleagues count only beside
/// contacts, and shared mailboxes and calendars beside mail or calendar. Video meetings count
/// whatever else is on.
pub(super) fn chosen(
    offer: &UseOffer,
    mail: Option<bool>,
    calendar: Option<Picked<'_>>,
    contacts: Option<Picked<'_>>,
    subs: Subchoices,
) -> ChosenUses {
    let Subchoices {
        shared_mailboxes,
        colleagues,
        meetings,
    } = subs;
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
            ((mail == Some(true) || calendar_on) && shared_mailboxes == Some(true))
                .then_some(AccountCapability::SharedMailboxes),
            calendar_on.then_some(AccountCapability::Calendar),
            contacts_on.then_some(AccountCapability::Contacts),
            (contacts_on && colleagues == Some(true)).then_some(AccountCapability::Colleagues),
            (meetings == Some(true)).then_some(AccountCapability::Meetings),
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

/// The choices beside a use: shared mailboxes and calendars beside mail and calendar, colleagues
/// within contacts, Teams meetings in the calendar's section; `None` for one the card did not
/// offer.
#[derive(Clone, Copy, Debug, Default)]
pub(super) struct Subchoices {
    pub(super) shared_mailboxes: Option<bool>,
    pub(super) colleagues: Option<bool>,
    pub(super) meetings: Option<bool>,
}
