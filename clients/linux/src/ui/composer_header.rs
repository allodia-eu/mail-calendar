//! The composer's header form: the From picker, the three recipient rows and the Subject entry.
//!
//! Split out of [`super::composer`] so the editor host stays the readable part of that file: the
//! same split the Android composer made into `ComposerHeaderFields.kt`.

use std::{rc::Rc, sync::Arc};

use adw::prelude::*;
use gtk::accessible::Property as AccessibleProperty;
use mailcal_bindings::MailcalApp;

use super::{composer_model::ComposeRequest, recipients::RecipientField};
use crate::l10n;

/// Whether the composer must open with Cc and Bcc revealed, given what the request pre-filled them
/// with.
///
/// The row is collapsed by default, so anything a caller puts in it would otherwise be a recipient
/// the sender cannot see: and cannot remove. A `mailto:` link may name `bcc`, which makes this a
/// security rule rather than a nicety (docs/composer-security.md, Gate 12); a reply-all reaches it
/// the same way. Whitespace is not an address.
pub(super) fn reveals_cc_bcc(cc: &str, bcc: &str) -> bool {
    !cc.trim().is_empty() || !bcc.trim().is_empty()
}

/// The composer's three recipient fields, once the header has been built.
pub(super) struct RecipientRows {
    pub(super) to: Rc<RecipientField>,
    pub(super) cc: Rc<RecipientField>,
    pub(super) bcc: Rc<RecipientField>,
}

/// One To/Cc/Bcc row: its caption and the box the field sits in, so the pair hides together. A
/// grid row whose every child is hidden takes no height and no spacing, which a row of its own
/// would still have taken.
#[derive(Clone)]
struct Row {
    caption: gtk::Label,
    body: gtk::Box,
    field: Rc<RecipientField>,
}

impl Row {
    fn set_visible(&self, visible: bool) {
        self.caption.set_visible(visible);
        self.body.set_visible(visible);
    }
}

/// Attaches To, Cc and Bcc to `form` at rows 1–3, with Cc and Bcc behind a chevron on the To row.
///
/// The header a message usually needs is From, To, Subject; the arrangement Gmail, Thunderbird
/// and the Android composer share.
pub(super) fn recipient_rows(
    form: &gtk::Grid,
    request: &ComposeRequest,
    app: Option<&Arc<MailcalApp>>,
) -> RecipientRows {
    let chevron = gtk::Image::from_icon_name("pan-down-symbolic");
    let toggle = gtk::ToggleButton::new();
    toggle.set_child(Some(&chevron));
    toggle.add_css_class("flat");
    // Beside the entry, not beside the pills the field grows above it.
    toggle.set_valign(gtk::Align::End);
    toggle.set_tooltip_text(Some(l10n::compose_show_cc_bcc()));
    toggle.update_property(&[AccessibleProperty::Label(l10n::compose_show_cc_bcc())]);

    // Seeded, not set: every address a caller supplies is finished, so each becomes a pill rather
    // than the last one landing in the input as half-typed text.
    let to = row(
        form,
        1,
        l10n::compose_to(),
        &request.initial_to,
        app,
        Some(toggle.upcast_ref()),
    );
    let cc = row(form, 2, l10n::compose_cc(), &request.initial_cc, app, None);
    let bcc = row(
        form,
        3,
        l10n::compose_bcc(),
        &request.initial_bcc,
        app,
        None,
    );

    let revealed = reveals_cc_bcc(&request.initial_cc, &request.initial_bcc);
    toggle.set_active(revealed);
    reveal(&cc, &bcc, &chevron, revealed);
    let (cc_row, bcc_row, icon) = (cc.clone(), bcc.clone(), chevron.clone());
    toggle.connect_toggled(move |toggle| {
        reveal(&cc_row, &bcc_row, &icon, toggle.is_active());
    });

    RecipientRows {
        to: to.field,
        cc: cc.field,
        bcc: bcc.field,
    }
}

/// Cc and Bcc follow the toggle, and the chevron points the way it would move them.
fn reveal(cc: &Row, bcc: &Row, chevron: &gtk::Image, revealed: bool) {
    cc.set_visible(revealed);
    bcc.set_visible(revealed);
    chevron.set_icon_name(Some(if revealed {
        "pan-up-symbolic"
    } else {
        "pan-down-symbolic"
    }));
}

/// A To/Cc/Bcc row: the caption, and the pills-plus-input field beside it.
fn row(
    form: &gtk::Grid,
    index: i32,
    label: &'static str,
    value: &str,
    app: Option<&Arc<MailcalApp>>,
    trailing: Option<&gtk::Widget>,
) -> Row {
    let caption = gtk::Label::new(Some(label));
    caption.set_xalign(1.0);
    caption.set_valign(gtk::Align::Start);
    caption.set_margin_top(6);
    form.attach(&caption, 0, index, 1, 1);
    let field = Rc::new(RecipientField::new(label, app.map(Arc::clone)));
    field.seed(value);
    // The field and its chevron share one grid cell, so the form stays two columns wide and Cc,
    // Bcc and Subject are drawn as wide as To.
    let body = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    body.set_hexpand(true);
    body.append(field.widget());
    if let Some(trailing) = trailing {
        body.append(trailing);
    }
    form.attach(&body, 1, index, 1, 1);
    Row {
        caption,
        body,
        field,
    }
}

pub(super) fn from_picker(
    accounts: &[(String, String)],
    initial_from: Option<&str>,
) -> gtk::DropDown {
    let labels = gtk::StringList::new(
        &accounts
            .iter()
            .map(|(_, email)| email.as_str())
            .collect::<Vec<_>>(),
    );
    let dropdown = gtk::DropDown::new(Some(labels.clone()), None::<gtk::Expression>);
    if let Some(index) =
        initial_from.and_then(|id| accounts.iter().position(|(account, _)| account == id))
    {
        dropdown.set_selected(u32::try_from(index).unwrap_or(0));
    }
    dropdown
}

pub(super) fn add_from_row(form: &gtk::Grid, row: i32, dropdown: &gtk::DropDown) {
    let label = gtk::Label::new(Some(l10n::compose_from()));
    label.set_xalign(1.0);
    form.attach(&label, 0, row, 1, 1);
    form.attach(dropdown, 1, row, 1, 1);
}

pub(super) fn entry_row(
    form: &gtk::Grid,
    row: i32,
    label: &str,
    value: &str,
    focus: bool,
) -> gtk::Entry {
    let caption = gtk::Label::new(Some(label));
    caption.set_xalign(1.0);
    form.attach(&caption, 0, row, 1, 1);
    let entry = gtk::Entry::new();
    entry.set_text(value);
    entry.set_hexpand(true);
    if focus {
        entry.set_activates_default(true);
    }
    form.attach(&entry, 1, row, 1, 1);
    entry
}

#[cfg(test)]
#[path = "composer_header_tests.rs"]
pub(super) mod tests;
