//! The last setup step, and a skippable one: an account that lacks a calendar or contacts can use
//! another account's, and a calendar without mail can name the account that sends its
//! invitations (`docs/onboarding.md`). The pickers are Settings' own, from the same snapshot, so
//! the step offers exactly what the account's page would.

use adw::prelude::*;
use mailcal_bindings::{AccountCapability, AccountsSnapshot, CapabilityState, LinkSlot};

use super::{
    AppInput,
    settings::{LinkPicker, link_pickers},
    setup_widgets::{body, caption, heading, page, primary, section},
};
use crate::l10n;

/// The calendar and address-book servers detection found beside a JMAP server, for the account
/// set up from it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Beside {
    pub(super) account: String,
    pub(super) email: String,
    pub(super) caldav_url: Option<String>,
    pub(super) carddav_url: Option<String>,
}

impl Beside {
    /// The host a person recognises the servers by.
    fn host(&self) -> String {
        self.caldav_url
            .as_deref()
            .or(self.carddav_url.as_deref())
            .map(super::setup_uses::url_host)
            .unwrap_or_default()
    }
}

/// The step for one account: its pickers, and what the person picked in each.
#[derive(Debug)]
pub(crate) struct LinkStep {
    pub(super) account: String,
    pub(super) pickers: Vec<LinkPicker>,
    /// The servers found beside the account's own, offered as an account of their own while its
    /// server offers no calendar or contacts.
    pub(super) beside: Option<Beside>,
    /// Per picker, the index into its options that is picked; a suggestion starts picked, and
    /// the person confirms it by continuing.
    pub(super) picked: Vec<Option<usize>>,
    /// Whether the person has changed a pick, after which a late suggestion leaves the step alone.
    pub(super) touched: bool,
}

impl LinkStep {
    /// The step for `account`, or `None` when no other account can fill anything it lacks and its
    /// server offers its calendar and contacts, or its kind is not offered linking at setup
    /// (`offers_setup_links`). A server without one gets the step anyway, for "Add another
    /// account" and for `beside`, which is offered only then.
    pub(super) fn for_account(
        snapshot: &AccountsSnapshot,
        account: &str,
        beside: Option<&Beside>,
    ) -> Option<Self> {
        let entry = snapshot
            .accounts
            .iter()
            .find(|entry| entry.id == account)
            .filter(|entry| mailcal_bindings::offers_setup_links(entry.kind))?;
        let pickers: Vec<LinkPicker> = link_pickers(entry)
            .into_iter()
            .filter(|picker| picker.selected.is_none())
            .collect();
        let lacks = entry.uses.iter().any(|use_| {
            matches!(
                use_.capability,
                AccountCapability::Calendar | AccountCapability::Contacts
            ) && use_.state == CapabilityState::NotOffered
        });
        let beside = beside.filter(|_| lacks).cloned();
        (!pickers.is_empty() || lacks).then(|| Self {
            account: account.to_owned(),
            picked: pickers.iter().map(|picker| picker.suggested).collect(),
            pickers,
            beside,
            touched: false,
        })
    }

    /// Picks `added`, an account just added from this step, in each slot it can fill and nothing
    /// is picked in: it was added to be linked.
    pub(super) fn adopt(&mut self, added: &str) {
        for (picker, picked) in self.pickers.iter().zip(self.picked.iter_mut()) {
            if picked.is_none()
                && let Some(index) = picker.options.iter().position(|option| option.id == added)
            {
                *picked = Some(index);
                self.touched = true;
            }
        }
    }

    /// The same step read again, for a suggestion that arrived after it was drawn. A step the
    /// person has already changed is kept as it is.
    ///
    /// The fresh pickers stand, since they may offer an account added since; what the person
    /// picked is carried across by account, slot by slot.
    pub(super) fn refreshed(self, mut fresh: Self) -> Self {
        if !self.touched {
            return fresh;
        }
        for (picker, picked) in fresh.pickers.iter().zip(fresh.picked.iter_mut()) {
            let earlier = self
                .pickers
                .iter()
                .zip(&self.picked)
                .find(|(earlier, _)| earlier.slot == picker.slot);
            if let Some((earlier, chose)) = earlier {
                *picked = chose
                    .and_then(|index| earlier.options.get(index))
                    .and_then(|option| {
                        picker
                            .options
                            .iter()
                            .position(|offered| offered.id == option.id)
                    });
            }
        }
        fresh.touched = true;
        fresh
    }

    /// Records a pick.
    pub(super) fn pick(&mut self, picker: usize, option: Option<usize>) {
        if let Some(slot) = self.picked.get_mut(picker) {
            *slot = option;
            self.touched = true;
        }
    }

    /// The links to set: each slot picked, and the account it names.
    pub(super) fn links(&self) -> Vec<(LinkSlot, String)> {
        self.pickers
            .iter()
            .zip(&self.picked)
            .filter_map(|(picker, picked)| {
                picked
                    .and_then(|index| picker.options.get(index))
                    .map(|option| (picker.slot, option.id.clone()))
            })
            .collect()
    }
}

/// The step's page.
pub(super) fn step(
    window: &gtk::Window,
    step: &LinkStep,
    sender: &relm4::Sender<AppInput>,
) -> gtk::Box {
    let content = page();
    content.append(&heading(l10n::setup_links_title()));
    content.append(&body(l10n::settings_account_links_description()));
    for (index, picker) in step.pickers.iter().enumerate() {
        content.append(&section(picker.title));
        let labels: Vec<&str> = std::iter::once(l10n::settings_account_link_none())
            .chain(picker.options.iter().map(|option| option.address.as_str()))
            .collect();
        let dropdown = gtk::DropDown::from_strings(&labels);
        let picked = step.picked.get(index).copied().flatten();
        dropdown.set_selected(
            picked
                .and_then(|at| u32::try_from(at + 1).ok())
                .unwrap_or(0),
        );
        content.append(&dropdown);
        if let Some(suggested) = picker.suggested.and_then(|at| picker.options.get(at)) {
            content.append(&caption(&l10n::settings_account_link_suggested(
                &suggested.address,
            )));
        }
        let input = sender.clone();
        dropdown.connect_selected_notify(move |dropdown| {
            let option = (dropdown.selected() as usize).checked_sub(1);
            input.emit(AppInput::SetupLinkPicked(index, option));
        });
    }
    if let Some(beside) = &step.beside {
        let host = beside.host();
        content.append(&body(&l10n::setup_links_beside_note(&host)));
        let set_up = gtk::Button::with_label(&l10n::setup_links_beside(&host));
        set_up.set_halign(gtk::Align::Start);
        let input = sender.clone();
        set_up.connect_clicked(move |_| input.emit(AppInput::SetupBesideAccount));
        content.append(&set_up);
    }
    content.append(&caption(l10n::setup_links_note()));
    let add = gtk::Button::with_label(l10n::setup_links_add());
    add.set_halign(gtk::Align::Start);
    let input = sender.clone();
    add.connect_clicked(move |_| input.emit(AppInput::SetupAddLinkedAccount));
    content.append(&add);

    let actions = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    actions.set_halign(gtk::Align::End);
    let skip = gtk::Button::with_label(l10n::setup_sender_name_skip());
    let input = sender.clone();
    skip.connect_clicked(move |_| input.emit(AppInput::SetupLinksDone(false)));
    actions.append(&skip);
    let link = primary(l10n::setup_detect_action(), window);
    let input = sender.clone();
    link.connect_clicked(move |_| input.emit(AppInput::SetupLinksDone(true)));
    actions.append(&link);
    content.append(&actions);
    content
}

#[cfg(test)]
#[path = "setup_links_tests.rs"]
pub(super) mod tests;

#[cfg(test)]
#[path = "setup_links_beside_tests.rs"]
pub(super) mod beside_tests;
