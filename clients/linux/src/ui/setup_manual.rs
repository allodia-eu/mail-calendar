//! The manual form: the user picks an account type, and that type's pane fills the rest.
//!
//! Reached from "Set up manually" on the email step, from the same button on a detected card
//! (prefilled with what detection found), and whenever detection comes back empty.

use std::rc::Rc;

use adw::prelude::*;

use super::{
    AppInput, setup_google, setup_imap, setup_jmap, setup_microsoft,
    setup_model::{AccountKind, ManualForm},
    setup_pane::ConnectPane,
    setup_widgets::body,
};
use crate::l10n;

/// A pane's current fields, read back on demand; so switching account type carries the address
/// (and that type's own servers) across instead of emptying the form.
pub(super) type FormSnapshot = Rc<dyn Fn() -> ManualForm>;

pub(super) fn fields(
    content: &gtk::Box,
    window: &gtk::Window,
    form: &ManualForm,
    error: Option<&str>,
    certificate: Option<&mailcal_bindings::RejectedCertificate>,
    required: bool,
    sender: &relm4::Sender<AppInput>,
) -> Option<ConnectPane> {
    if let Some(note) = &form.note {
        content.append(&body(note));
    }
    let picker = account_type_picker(content, form.kind);
    // The two credential routes connect from here, so they keep a pane a result can be drawn
    // into; the OAuth routes hand off to a browser and have nothing to report in place.
    let (snapshot, pane) = match form.kind {
        AccountKind::Imap => {
            let (snapshot, pane) = setup_imap::manual_fields(
                content,
                window,
                form,
                error,
                certificate,
                required,
                sender,
            );
            (snapshot, Some(pane))
        }
        AccountKind::Jmap => {
            let (snapshot, pane) =
                setup_jmap::manual_fields(content, window, form, error, required, sender);
            (snapshot, Some(pane))
        }
        AccountKind::Microsoft => (
            setup_microsoft::manual_fields(content, window, form, error, required, sender),
            None,
        ),
        AccountKind::Google => (
            setup_google::manual_fields(content, window, form, error, required, sender),
            None,
        ),
    };
    // Connected after the pane exists, so setting the initial selection above cannot fire it.
    let input = sender.clone();
    picker.connect_selected_notify(move |chosen| {
        let mut carried = snapshot();
        carried.kind = AccountKind::from_position(chosen.selected());
        input.emit(AppInput::SelectAccountKind(Box::new(carried)));
    });
    pane
}

fn account_type_picker(content: &gtk::Box, kind: AccountKind) -> gtk::DropDown {
    let row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    let label = body(l10n::setup_account_type());
    row.append(&label);
    let offered = AccountKind::offered();
    let labels: Vec<&str> = offered.iter().copied().map(AccountKind::label).collect();
    let picker = gtk::DropDown::from_strings(&labels);
    picker.set_selected(kind.position());
    picker.set_hexpand(true);
    picker.set_halign(gtk::Align::End);
    row.append(&picker);
    content.append(&row);
    picker
}
