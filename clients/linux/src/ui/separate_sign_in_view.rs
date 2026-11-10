//! The widgets of a sign-in of its own ([`super::separate_sign_in`]): setup's step, the action
//! the account's page offers, and the form that action opens over Settings. Each is drawn from
//! the core's section and the browser sign-in's state; the form is a modal that a redraw of the
//! page leaves open and updates in place, so what was typed stands while a connect runs and after
//! it fails, and setup's step is updated in place the same way (`super::setup`).

use std::{cell::Cell, rc::Rc};

use adw::prelude::*;
use mailcal_bindings::{AccountCapability, AccountSeparateSignIn, ControlState};

use super::{
    AppInput,
    account_settings::AccountsInput,
    controls,
    separate_server_sign_in::{ProviderInput, SignInStage},
    separate_sign_in::{Password, SeparateForm, SeparateFrom, SeparateInput},
    setup_widgets::{body, entry, heading, page},
    unreached,
};
use crate::l10n;

fn input(input: SeparateInput) -> AppInput {
    AppInput::Accounts(AccountsInput::Separate(input))
}

/// What the step and the form both draw: the server, the sign-in at its own provider once it
/// says it has one, the login, the password, why the last attempt failed, and the other way out
/// beside Connect.
#[derive(Debug)]
pub(super) struct Fields {
    window: gtk::Window,
    from: SeparateFrom,
    sender: relm4::Sender<AppInput>,
    sign_in_area: gtk::Box,
    sign_in: gtk::Button,
    /// The line under the sign-in, which says whose it is.
    note: gtk::Label,
    /// The spinner while a sign-in is out, with the Cancel that retires it while the browser
    /// holds it.
    waiting: gtk::Box,
    spinner: gtk::Spinner,
    cancel: gtk::Button,
    username: gtk::Entry,
    password: gtk::Entry,
    error: gtk::Label,
    /// The other way out, then Connect, drawn from the core's controls each time they change.
    actions: gtk::Box,
    drawn: Cell<Option<(ControlState, ControlState, bool)>>,
}

impl Fields {
    fn append(
        content: &gtk::Box,
        window: &gtk::Window,
        form: &SeparateForm,
        from: SeparateFrom,
        sender: &relm4::Sender<AppInput>,
    ) -> Self {
        content.append(&body(&l10n::separate_server_body(&form.section.host)));
        // Drawn now and shown when the server answers, above the fields and without taking
        // the focus from one being typed in.
        let sign_in_area = gtk::Box::new(gtk::Orientation::Vertical, 8);
        let sign_in = gtk::Button::with_label(l10n::setup_imap_signin_button());
        sign_in.set_halign(gtk::Align::Start);
        sign_in_area.append(&sign_in);
        let note = body(l10n::setup_imap_signin_note());
        sign_in_area.append(&note);
        let waiting = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        let spinner = gtk::Spinner::new();
        waiting.append(&spinner);
        let cancel = gtk::Button::with_label(l10n::action_cancel());
        waiting.append(&cancel);
        sign_in_area.append(&waiting);
        content.append(&sign_in_area);
        let username = entry(l10n::setup_field_username(), &form.section.username, false);
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
        content.append(&actions);
        let fields = Self {
            window: window.clone(),
            from,
            sender: sender.clone(),
            sign_in_area,
            sign_in,
            note,
            waiting,
            spinner,
            cancel,
            username,
            password,
            error,
            actions,
            drawn: Cell::new(None),
        };
        fields.connect_sign_in();
        fields.show(form);
        fields
    }

    /// Holds every control while the core or a blocking sign-in call holds the form, shows the
    /// sign-in once it is offered, and says why the last attempt failed. The browser's wait holds
    /// nothing: a fresh press supersedes it, and Cancel beside it retires it.
    pub(super) fn show(&self, form: &SeparateForm) {
        let held = form.held();
        for control in [
            self.sign_in.upcast_ref::<gtk::Widget>(),
            self.username.upcast_ref(),
            self.password.upcast_ref(),
        ] {
            control.set_sensitive(!held);
        }
        let offers = form.offers();
        self.sign_in_area.set_visible(offers.is_some());
        let waiting = matches!(form.stage, SignInStage::Waiting(_));
        let (button, note) = if offers == Some(AccountSeparateSignIn::Nextcloud) {
            let note = if waiting {
                l10n::nextcloud_sign_in_waiting()
            } else {
                l10n::nextcloud_sign_in_note()
            };
            (l10n::nextcloud_sign_in_button(), note)
        } else {
            (
                l10n::setup_imap_signin_button(),
                l10n::setup_imap_signin_note(),
            )
        };
        self.sign_in.set_label(button);
        self.note.set_text(note);
        let out = form.stage != SignInStage::Idle;
        self.waiting.set_visible(out);
        self.spinner.set_spinning(out);
        self.cancel.set_visible(waiting);
        let said = form.said();
        self.error.set_text(said.as_deref().unwrap_or_default());
        self.error.set_visible(said.is_some());
        // A browser sign-in's own core call holds the buttons the core does not know about.
        let browsing = held && form.section.connect.enabled;
        let free = |control: ControlState| ControlState {
            enabled: control.enabled && !browsing,
            ..control
        };
        let connect = free(form.section.connect);
        let other = match self.from {
            SeparateFrom::Setup => free(form.section.stop_using),
            // The form's own Cancel waits for a connect it would not stop.
            SeparateFrom::Page => ControlState {
                enabled: !form.section.connect.running,
                running: false,
            },
        };
        let key = (connect, other, offers.is_some());
        if self.drawn.replace(Some(key)) != Some(key) {
            self.draw_actions(connect, other, offers.is_some());
        }
        // The sign-in leads where it is offered; Enter in the password field still connects.
        if offers.is_some() {
            self.sign_in.add_css_class("suggested-action");
        } else {
            self.sign_in.remove_css_class("suggested-action");
        }
    }

    /// The other way out and Connect, from the core's controls.
    fn draw_actions(&self, connect: ControlState, other: ControlState, offers: bool) {
        while let Some(child) = self.actions.first_child() {
            self.actions.remove(&child);
        }
        let (label, sender, window) = match self.from {
            SeparateFrom::Setup => (l10n::separate_server_skip(), self.sender.clone(), None),
            SeparateFrom::Page => (
                l10n::action_cancel(),
                self.sender.clone(),
                Some(self.window.clone()),
            ),
        };
        self.actions.append(&controls::button(
            label,
            other,
            false,
            move || match &window {
                Some(window) => window.close(),
                None => sender.emit(input(SeparateInput::Skip)),
            },
        ));
        let (from, username, password, sender) = (
            self.from,
            self.username.clone(),
            self.password.clone(),
            self.sender.clone(),
        );
        let button = controls::button(l10n::action_connect(), connect, false, move || {
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
        if !offers {
            button.add_css_class("suggested-action");
        }
        self.actions.append(&button);
        self.window.set_default_widget(Some(&button));
    }

    /// The sign-in and its Cancel send themselves.
    fn connect_sign_in(&self) {
        for (button, starts) in [(&self.sign_in, true), (&self.cancel, false)] {
            let (sender, from) = (self.sender.clone(), self.from);
            button.connect_clicked(move |_| {
                let pressed = if starts {
                    ProviderInput::Start
                } else {
                    ProviderInput::Cancel
                };
                sender.emit(input(SeparateInput::Provider {
                    from,
                    input: pressed,
                }));
            });
        }
    }
}

/// Setup's step, after the mailbox was added and before its link step, and its fields, which
/// the window updates in place.
pub(super) fn setup_step(
    window: &gtk::Window,
    step: &SeparateForm,
    sender: &relm4::Sender<AppInput>,
) -> (gtk::Box, Fields) {
    let content = page();
    content.append(&heading(unreached::separate_title(&step.section.uses)));
    let fields = Fields::append(&content, window, step, SeparateFrom::Setup, sender);
    (content, fields)
}

/// The account page's action, which opens the form, held while a change runs on the page.
pub(super) fn page_action(
    account: &str,
    enabled: bool,
    sender: &relm4::Sender<AppInput>,
) -> adw::ButtonRow {
    let (sender, account) = (sender.clone(), account.to_owned());
    super::controls::button_row(
        l10n::settings_account_use_separate_sign_in(),
        enabled,
        move || {
            sender.emit(input(SeparateInput::Open {
                account: account.clone(),
            }));
        },
    )
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
        question: Option<(&str, u64, &[AccountCapability])>,
        form: Option<&SeparateForm>,
        parent: &gtk::Window,
        sender: &relm4::Sender<AppInput>,
    ) {
        if let Some((account, attempt, unreached)) = question
            && self.asked != Some(attempt)
        {
            self.asked = Some(attempt);
            let (sender, account) = (sender.clone(), account.to_owned());
            unreached::ask(parent, unreached, move |answer| {
                sender.emit(AppInput::Accounts(AccountsInput::Change(
                    mailcal_bindings::AccountPageIntent::AnswerUnreached {
                        account: account.clone(),
                        attempt,
                        answer: answer.choice(),
                    },
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
    let title = unreached::separate_title(&form.section.uses);
    let (window, _) = crate::ui::modal::new(parent, title, 440, None);
    let content = page();
    let fields = Fields::append(&content, &window, form, SeparateFrom::Page, sender);
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
