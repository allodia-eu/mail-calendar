//! The widgets of a sign-in of its own ([`super::separate_sign_in`]): setup's step, the action
//! the account's page offers, and the form that action opens over Settings. Each is drawn from
//! the model's state; the form is a modal that a redraw of the page leaves open and updates in
//! place, so what was typed stands while a connect runs and after it fails.

use std::{cell::Cell, rc::Rc};

use adw::prelude::*;
use mailcal_bindings::{AccountCapability, AccountEntry};

use super::{
    AppInput,
    account_settings::AccountsInput,
    account_sign_in::{Password, SignInInput},
    separate_sign_in::{SeparateForm, SeparateFrom, SeparateInput},
    setup_widgets::{body, entry, heading, page, primary},
    unreached,
};
use crate::l10n;

fn input(input: SeparateInput) -> AppInput {
    AppInput::Accounts(AccountsInput::Separate(input))
}

/// What the step and the form both draw: the server, the login, the password, why the last
/// connect failed, Connect, and the other way out.
#[derive(Debug)]
struct Fields {
    username: gtk::Entry,
    password: gtk::Entry,
    error: gtk::Label,
    connect: gtk::Button,
    other: gtk::Button,
}

impl Fields {
    fn append(content: &gtk::Box, window: &gtk::Window, form: &SeparateForm, other: &str) -> Self {
        content.append(&body(&l10n::separate_server_body(&form.server.host)));
        let username = entry(l10n::setup_field_username(), &form.username, false);
        content.append(&username);
        let password = entry(l10n::setup_field_password(), "", true);
        password.set_input_purpose(gtk::InputPurpose::Password);
        password.set_activates_default(true);
        content.append(&password);
        let error = body("");
        error.add_css_class("error");
        content.append(&error);
        let actions = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        actions.set_halign(gtk::Align::End);
        let other = gtk::Button::with_label(other);
        actions.append(&other);
        let connect = primary(l10n::action_connect(), window);
        actions.append(&connect);
        content.append(&actions);
        let fields = Self {
            username,
            password,
            error,
            connect,
            other,
        };
        fields.show(form);
        fields
    }

    /// Holds every control while a connect runs, and says why the last one failed.
    fn show(&self, form: &SeparateForm) {
        for control in [
            self.username.upcast_ref::<gtk::Widget>(),
            self.password.upcast_ref(),
            self.connect.upcast_ref(),
            self.other.upcast_ref(),
        ] {
            control.set_sensitive(!form.busy);
        }
        self.error
            .set_text(form.error.as_deref().unwrap_or_default());
        self.error.set_visible(form.error.is_some());
    }

    /// Connect sends what was typed, once there is a password to send.
    fn connect_sends(&self, from: SeparateFrom, sender: &relm4::Sender<AppInput>) {
        let (username, password, sender) =
            (self.username.clone(), self.password.clone(), sender.clone());
        self.connect.connect_clicked(move |_| {
            let typed = password.text().to_string();
            if typed.is_empty() {
                return;
            }
            sender.emit(input(SeparateInput::Connect {
                from,
                username: username.text().trim().to_owned(),
                password: Password(typed),
            }));
        });
    }
}

/// Setup's step, after the mailbox was added and before its link step.
pub(super) fn setup_step(
    window: &gtk::Window,
    step: &SeparateForm,
    sender: &relm4::Sender<AppInput>,
) -> gtk::Box {
    let content = page();
    content.append(&heading(unreached::separate_title(&step.server.uses)));
    let fields = Fields::append(&content, window, step, l10n::separate_server_skip());
    fields.connect_sends(SeparateFrom::Setup, sender);
    let skip = sender.clone();
    fields
        .other
        .connect_clicked(move |_| skip.emit(input(SeparateInput::Skip)));
    content
}

/// The use whose row the account page's action sits under: the last of those that need the
/// sign-in, so it follows every note that says why. `None` when nothing needs one.
pub(super) fn action_after(entry: &AccountEntry) -> Option<AccountCapability> {
    entry.separate_server.as_ref()?.uses.last().copied()
}

/// The account page's action, which opens the form.
pub(super) fn page_action(
    entry: &AccountEntry,
    sender: &relm4::Sender<AppInput>,
) -> Option<adw::ButtonRow> {
    let server = entry.separate_server.clone()?;
    let row = adw::ButtonRow::new();
    row.set_use_markup(false);
    row.set_title(l10n::settings_account_use_separate_sign_in());
    let (sender, account) = (sender.clone(), entry.id.clone());
    row.connect_activated(move |_| {
        sender.emit(input(SeparateInput::Open {
            account: account.clone(),
            server: server.clone(),
        }));
    });
    Some(row)
}

/// The form on screen, and whether the model is the one closing it.
#[derive(Debug)]
struct Drawn {
    id: u64,
    window: gtk::Window,
    fields: Fields,
    closing: Rc<Cell<bool>>,
}

/// The two things Settings raises from the model's state rather than from a press: the switch's
/// question, once per sign-in, and the account page's form.
#[derive(Debug, Default)]
pub(super) struct Prompts {
    asked: Option<u64>,
    form: Option<Drawn>,
}

impl Prompts {
    /// Brings both in line with the model: asks a question not asked yet, and opens, updates or
    /// closes the form. `parent` is the window they are raised over.
    pub(super) fn sync(
        &mut self,
        question: Option<(u64, &[AccountCapability])>,
        form: Option<&SeparateForm>,
        parent: &gtk::Window,
        sender: &relm4::Sender<AppInput>,
    ) {
        if let Some((attempt, unreached)) = question
            && self.asked != Some(attempt)
        {
            self.asked = Some(attempt);
            let sender = sender.clone();
            unreached::ask(parent, unreached, move |answer| {
                sender.emit(AppInput::Accounts(AccountsInput::SignIn(
                    SignInInput::Answered { attempt, answer },
                )));
            });
        }
        match (form, &self.form) {
            (Some(form), Some(drawn)) if drawn.id == form.id => drawn.fields.show(form),
            (Some(form), _) => {
                self.close();
                self.form = Some(open(form, parent, sender));
            }
            (None, Some(_)) => self.close(),
            (None, None) => {}
        }
    }

    fn close(&mut self) {
        if let Some(drawn) = self.form.take() {
            drawn.closing.set(true);
            if drawn.window.is_visible() {
                drawn.window.close();
            }
        }
    }

    #[cfg(test)]
    pub(super) fn form_window(&self) -> Option<gtk::Window> {
        self.form.as_ref().map(|drawn| drawn.window.clone())
    }
}

fn open(form: &SeparateForm, parent: &gtk::Window, sender: &relm4::Sender<AppInput>) -> Drawn {
    let title = unreached::separate_title(&form.server.uses);
    let (window, _) = crate::ui::modal::new(parent, title, 440, None);
    let content = page();
    let fields = Fields::append(&content, &window, form, l10n::action_cancel());
    fields.connect_sends(SeparateFrom::Page, sender);
    let dismiss = window.clone();
    fields.other.connect_clicked(move |_| dismiss.close());
    let closing = Rc::new(Cell::new(false));
    let (closed, sender) = (closing.clone(), sender.clone());
    window.connect_close_request(move |_| {
        if !closed.get() {
            sender.emit(input(SeparateInput::Close));
        }
        gtk::glib::Propagation::Proceed
    });
    window.set_child(Some(&content));
    window.present();
    Drawn {
        id: form.id,
        window,
        fields,
        closing,
    }
}

#[cfg(test)]
#[path = "separate_sign_in_view_tests.rs"]
pub(crate) mod tests;
