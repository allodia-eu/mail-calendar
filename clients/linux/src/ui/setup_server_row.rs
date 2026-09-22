//! The manual form's server row: the name, the port and the security picker side by side.
//!
//! The trap this file exists to avoid: writing a port into the entry raises the same `changed`
//! signal the user does, so a naive handler reads the app's own fill as the user taking the port
//! over and the picker stops working. The guard cell is what tells the two apart.

use std::{cell::Cell, rc::Rc};

use adw::prelude::*;
use mailcal_bindings::ConnectionSecurity;

use super::{setup_server_field::ServerField, setup_widgets::entry};
use crate::l10n;

/// The three controls for one server, plus the shared state the picker and the port entry both
/// touch. [`ServerRow::read`] gives back the field as the user has left it.
#[derive(Clone)]
pub(super) struct ServerRow {
    pub(super) host: gtk::Entry,
    port: gtk::Entry,
    security: gtk::DropDown,
    field: Rc<Cell<bool>>,
    kind: mailcal_bindings::MailServerKind,
}

/// Builds the row and appends it to `content`.
pub(super) fn server_row(
    content: &gtk::Box,
    label: &str,
    host_value: &str,
    field: &ServerField,
) -> ServerRow {
    let row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    let host = entry(label, host_value, false);
    host.set_hexpand(true);
    let port = entry(l10n::setup_field_port(), field.port(), false);
    port.set_width_chars(6);
    port.set_hexpand(false);
    let security = gtk::DropDown::from_strings(&[
        l10n::setup_security_implicit_tls(),
        l10n::setup_security_starttls(),
    ]);
    security.set_selected(u32::from(field.security() == ConnectionSecurity::StartTls));
    row.append(&host);
    row.append(&port);
    row.append(&security);
    content.append(&row);

    // True while the code is writing the port, so that write is not mistaken for the user's.
    let filling = Rc::new(Cell::new(false));
    let typed_by_hand = Rc::new(Cell::new(!field.follows_security()));

    {
        let typed = typed_by_hand.clone();
        let guard = filling.clone();
        port.connect_changed(move |_| {
            if !guard.get() {
                typed.set(true);
            }
        });
    }
    {
        let typed = typed_by_hand.clone();
        let guard = filling.clone();
        let port = port.clone();
        let kind = field.kind();
        security.connect_selected_notify(move |chosen| {
            if typed.get() {
                return;
            }
            guard.set(true);
            port.set_text(&mailcal_bindings::standard_port(kind, selected(chosen)).to_string());
            guard.set(false);
        });
    }

    ServerRow {
        host,
        port,
        security,
        field: typed_by_hand,
        kind: field.kind(),
    }
}

impl ServerRow {
    /// The host name alone, trimmed; the port lives in its own entry.
    pub(super) fn host_text(&self) -> String {
        self.host.text().trim().to_owned()
    }

    /// The field as the user has left it, ready to be carried across a re-render.
    pub(super) fn read(&self) -> ServerField {
        let mut field = ServerField::new(self.kind);
        field.choose_security(selected(&self.security));
        if self.field.get() {
            field.type_port(self.port.text().trim());
        }
        field
    }

    /// The `host:port` this row submits.
    pub(super) fn dial(&self) -> String {
        self.read().dial(&self.host_text())
    }
}

/// Which security a picker is showing. Position 1 is STARTTLS, matching the order the strings
/// were built in above.
fn selected(picker: &gtk::DropDown) -> ConnectionSecurity {
    if picker.selected() == 1 {
        ConnectionSecurity::StartTls
    } else {
        ConnectionSecurity::ImplicitTls
    }
}
