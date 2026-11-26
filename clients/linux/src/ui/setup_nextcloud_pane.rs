//! The manual form's Nextcloud pane: the server's address and its own sign-in, for a person whose
//! email address leads detection nowhere near their Nextcloud server
//! (`docs/nextcloud-sign-in.md` rule 2). The core asks the address whether it is Nextcloud before
//! a browser opens. When it cannot tell, the pane says so and offers the sign-in anyway, for as
//! long as it holds the address the core was asked about.

use std::rc::Rc;

use adw::prelude::*;

use super::{
    AppInput,
    setup_manual::FormSnapshot,
    setup_model::ManualForm,
    setup_nextcloud::SetupNextcloudInput,
    setup_widgets::{actions, body, entry, primary, show_error},
};
use crate::l10n;

pub(super) fn manual_fields(
    content: &gtk::Box,
    window: &gtk::Window,
    form: &ManualForm,
    error: Option<&str>,
    required: bool,
    sender: &relm4::Sender<AppInput>,
) -> FormSnapshot {
    content.append(&body(l10n::setup_nextcloud_note()));
    let address = entry(
        l10n::setup_field_nextcloud_server(),
        &form.nextcloud_server,
        false,
    );
    content.append(&address);
    show_error(content, error);

    let snapshot: FormSnapshot = {
        let (base, address) = (form.clone(), address.clone());
        Rc::new(move || ManualForm {
            nextcloud_server: address.text().trim().to_owned(),
            ..base.clone()
        })
    };
    let start = |anyway: bool| {
        let (input, typed) = (sender.clone(), Rc::clone(&snapshot));
        move |_: &gtk::Button| {
            input.emit(AppInput::SetupNextcloud(SetupNextcloudInput::StartAt {
                typed: Box::new(typed()),
                anyway,
            }));
        }
    };
    let unsure = form.nextcloud_unsure.as_ref().map(|(asked, host)| {
        let offer = gtk::Box::new(gtk::Orientation::Vertical, 8);
        offer.append(&body(&l10n::setup_nextcloud_unsure(host)));
        let anyway = gtk::Button::with_label(l10n::setup_nextcloud_try_anyway());
        anyway.set_halign(gtk::Align::Start);
        anyway.connect_clicked(start(true));
        offer.append(&anyway);
        offer.set_visible(address.text().trim() == asked);
        content.append(&offer);
        (offer, asked.clone())
    });

    let actions = actions(window, required, sender);
    let button = primary(l10n::nextcloud_sign_in_button(), window);
    button.set_sensitive(!address.text().trim().is_empty());
    let gated = button.clone();
    address.connect_changed(move |address| {
        let typed = address.text();
        let typed = typed.trim();
        gated.set_sensitive(!typed.is_empty());
        if let Some((offer, asked)) = &unsure {
            offer.set_visible(typed == asked);
        }
    });
    button.connect_clicked(start(false));
    actions.append(&button);
    content.append(&actions);
    snapshot
}
