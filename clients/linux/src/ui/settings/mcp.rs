//! Settings → Advanced's AI assistant access, drawn from the core's `McpPanel` (`docs/mcp.md`).
//!
//! The section follows the core in place: a change made here, the server binding its endpoint or
//! refusing it, and an account added each move only the rows they concern, so a switch the person
//! just used keeps its focus.

use std::{
    cell::{Cell, RefCell},
    rc::{Rc, Weak},
};

use adw::prelude::*;
use mailcal_bindings::{Intent, MailcalApp, McpAccountRow, McpIntent, McpPanel, McpStatus};

use super::{PageContext, group};
use crate::l10n;

pub(super) fn section(ctx: &PageContext) -> Option<gtk::Box> {
    let panel = ctx.app.advanced_page().assistants?;
    let view = View::build(&ctx.app, &panel);
    let content = view.content.clone();
    let app = ctx.app.clone();
    ctx.follow.add(move || {
        if let Some(panel) = app.advanced_page().assistants {
            view.follow(panel);
        }
    });
    Some(content)
}

/// The words under the switch for what the server is doing.
fn status_text(status: McpStatus) -> &'static str {
    match status {
        McpStatus::Off => l10n::settings_mcp_status_off(),
        McpStatus::Starting => l10n::settings_mcp_status_starting(),
        McpStatus::Listening => l10n::settings_mcp_status_running(),
        McpStatus::AnotherInstance => l10n::settings_mcp_status_another_instance(),
        McpStatus::Unavailable => l10n::settings_mcp_status_unavailable(),
    }
}

/// Whether two panels list the same accounts, exposed or not.
fn same_accounts(left: &[McpAccountRow], right: &[McpAccountRow]) -> bool {
    left.len() == right.len()
        && left
            .iter()
            .zip(right)
            .all(|(left, right)| left.account_id == right.account_id && left.email == right.email)
}

/// The accounts group as drawn: a switch per account, and the row saying none is exposed.
struct Accounts {
    group: adw::PreferencesGroup,
    rows: Vec<adw::SwitchRow>,
    empty: adw::ActionRow,
}

/// The section's widgets, and the panel they show.
struct View {
    app: std::sync::Arc<MailcalApp>,
    drawn: RefCell<McpPanel>,
    /// Set while the section is brought to the core, so moving a switch to the core's value
    /// sends nothing back.
    following: Cell<bool>,
    content: gtk::Box,
    toggle: adw::SwitchRow,
    status: adw::ActionRow,
    disclosure: gtk::Revealer,
    details: gtk::Box,
    accounts: RefCell<Option<Accounts>>,
    direct: adw::SwitchRow,
    known: adw::SwitchRow,
    snippet: gtk::TextBuffer,
}

impl View {
    fn build(app: &std::sync::Arc<MailcalApp>, panel: &McpPanel) -> Rc<Self> {
        let content = gtk::Box::new(gtk::Orientation::Vertical, 18);
        let master = group(
            l10n::settings_mcp_heading(),
            l10n::settings_mcp_description(),
        );
        let toggle = switch(l10n::settings_mcp_toggle(), None, "mcp-enabled-toggle");
        master.add(&toggle);
        let status = adw::ActionRow::builder().use_markup(false).build();
        status.set_widget_name("mcp-status");
        master.add(&status);
        content.append(&master);

        let details = gtk::Box::new(gtk::Orientation::Vertical, 18);
        let send = group(
            l10n::settings_mcp_send_heading(),
            l10n::settings_mcp_send_note(),
        );
        let direct = switch(
            l10n::settings_mcp_send_toggle(),
            None,
            "mcp-direct-send-toggle",
        );
        let known = switch(
            l10n::settings_mcp_known_recipient_toggle(),
            Some(l10n::settings_mcp_known_recipient_note()),
            "mcp-known-recipient-toggle",
        );
        send.add(&direct);
        send.add(&known);
        let (configuration, snippet) = configuration();
        let disclosure = gtk::Revealer::builder()
            .transition_type(gtk::RevealerTransitionType::Crossfade)
            .child(&details)
            .build();
        content.append(&disclosure);

        let view = Rc::new(Self {
            app: app.clone(),
            drawn: RefCell::new(panel.clone()),
            following: Cell::new(false),
            content,
            toggle,
            status,
            disclosure,
            details: details.clone(),
            accounts: RefCell::new(None),
            direct,
            known,
            snippet,
        });
        let accounts = view.accounts_group(panel);
        details.append(&accounts.group);
        details.append(&send);
        details.append(&configuration);
        *view.accounts.borrow_mut() = Some(accounts);
        view.connect();
        view.show(panel);
        view
    }

    /// The accounts group for `panel`, each row telling the core when it is moved.
    fn accounts_group(self: &Rc<Self>, panel: &McpPanel) -> Accounts {
        let group = group(
            l10n::settings_mcp_accounts_heading(),
            l10n::settings_mcp_accounts_description(),
        );
        let rows = panel
            .accounts
            .iter()
            .map(|account| {
                let row = switch(&account.email, None, "");
                let view = Rc::downgrade(self);
                let id = account.account_id.clone();
                row.connect_active_notify(move |row| {
                    send(
                        &view,
                        McpIntent::SetAccountExposed {
                            account: id.clone(),
                            exposed: row.is_active(),
                        },
                    );
                });
                group.add(&row);
                row
            })
            .collect();
        if panel.accounts.is_empty() {
            group.add(
                &adw::ActionRow::builder()
                    .title(l10n::settings_accounts_empty())
                    .use_markup(false)
                    .build(),
            );
        }
        let empty = adw::ActionRow::builder()
            .title(l10n::settings_mcp_accounts_empty())
            .use_markup(false)
            .build();
        empty.set_widget_name("mcp-accounts-empty");
        group.add(&empty);
        Accounts { group, rows, empty }
    }

    fn connect(self: &Rc<Self>) {
        let weak = Rc::downgrade(self);
        self.toggle.connect_active_notify(move |row| {
            send(
                &weak,
                McpIntent::SetEnabled {
                    enabled: row.is_active(),
                },
            );
        });
        let weak = Rc::downgrade(self);
        self.direct.connect_active_notify(move |row| {
            send(
                &weak,
                McpIntent::SetDirectSend {
                    allow: row.is_active(),
                },
            );
        });
        let weak = Rc::downgrade(self);
        self.known.connect_active_notify(move |row| {
            send(
                &weak,
                McpIntent::SetKnownRecipient {
                    require: row.is_active(),
                },
            );
        });
    }

    /// Brings the section to `panel`, rebuilding the accounts only when the accounts changed.
    fn follow(self: &Rc<Self>, panel: McpPanel) {
        if *self.drawn.borrow() == panel {
            return;
        }
        if !same_accounts(&self.drawn.borrow().accounts, &panel.accounts) {
            let fresh = self.accounts_group(&panel);
            if let Some(old) = self.accounts.replace(None) {
                self.details
                    .insert_child_after(&fresh.group, Some(&old.group));
                self.details.remove(&old.group);
            }
            *self.accounts.borrow_mut() = Some(fresh);
        }
        self.show(&panel);
        *self.drawn.borrow_mut() = panel;
    }

    /// Sets every control to `panel` without sending anything back.
    fn show(&self, panel: &McpPanel) {
        self.following.set(true);
        self.toggle.set_active(panel.enabled);
        self.status.set_title(status_text(panel.status));
        self.disclosure.set_reveal_child(panel.enabled);
        if let Some(accounts) = &*self.accounts.borrow() {
            for (row, account) in accounts.rows.iter().zip(&panel.accounts) {
                row.set_active(account.exposed);
            }
            accounts.empty.set_visible(panel.nothing_exposed);
        }
        self.direct.set_active(panel.allow_direct_send);
        self.known.set_active(panel.require_known_recipient);
        self.known.set_sensitive(panel.known_recipient_editable);
        if self
            .snippet
            .text(&self.snippet.start_iter(), &self.snippet.end_iter(), false)
            != panel.snippet
        {
            self.snippet.set_text(&panel.snippet);
        }
        self.following.set(false);
    }
}

/// Tells the core what the person changed, then shows what the core made of it.
fn send(view: &Weak<View>, intent: McpIntent) {
    let Some(view) = view.upgrade() else {
        return;
    };
    if view.following.get() {
        return;
    }
    view.app.dispatch(Intent::Mcp { intent });
    if let Some(panel) = view.app.advanced_page().assistants {
        view.follow(panel);
    }
}

fn switch(title: &str, subtitle: Option<&str>, name: &str) -> adw::SwitchRow {
    let row = adw::SwitchRow::builder()
        .title(title)
        .use_markup(false)
        .build();
    if let Some(subtitle) = subtitle {
        row.set_subtitle(subtitle);
    }
    if !name.is_empty() {
        row.set_widget_name(name);
    }
    row
}

/// The configuration to paste into an assistant, with a Copy button that copies what is shown.
fn configuration() -> (adw::PreferencesGroup, gtk::TextBuffer) {
    let section = group(
        l10n::settings_mcp_config_heading(),
        l10n::settings_mcp_config_description(),
    );
    let buffer = gtk::TextBuffer::new(None);
    let text = gtk::TextView::builder()
        .buffer(&buffer)
        .editable(false)
        .cursor_visible(false)
        .monospace(true)
        .wrap_mode(gtk::WrapMode::None)
        .build();
    text.set_widget_name("mcp-config-snippet");
    let scroll = gtk::ScrolledWindow::builder()
        .min_content_height(150)
        .max_content_height(220)
        .propagate_natural_height(true)
        .child(&text)
        .build();
    section.add(&scroll);
    let copy = gtk::Button::with_label(l10n::settings_mcp_copy());
    copy.set_widget_name("mcp-copy-config");
    let copy_row = adw::ActionRow::new();
    copy_row.add_suffix(&copy);
    let shown = buffer.clone();
    copy.connect_clicked(move |button| {
        if let Some(display) = gtk::gdk::Display::default() {
            let snippet = shown.text(&shown.start_iter(), &shown.end_iter(), false);
            display.clipboard().set_text(&snippet);
            button.set_label(l10n::settings_mcp_copied());
            let button = button.downgrade();
            gtk::glib::timeout_add_local_once(std::time::Duration::from_secs(2), move || {
                if let Some(button) = button.upgrade() {
                    button.set_label(l10n::settings_mcp_copy());
                }
            });
        }
    });
    section.add(&copy_row);
    (section, buffer)
}
