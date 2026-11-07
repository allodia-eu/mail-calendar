//! What setup knows about a sign-in with Nextcloud: the server a manual form last asked about,
//! and a grant no account was made from yet (`docs/nextcloud-sign-in.md` rule 6). A child of the
//! setup state, so its phase changes are the state's own.

use super::{Phase, SetupState};
use crate::ui::{
    nextcloud_flight::Grant,
    setup_model::{AccountKind, ManualForm, SetupForm},
};

#[derive(Debug, Default)]
pub(crate) struct SetupNextcloud {
    /// The server the manual calendar and contacts pane last asked about, so leaving a field
    /// twice asks once.
    asked: Option<String>,
    /// The grant the last completion could not make an account of, and the server it is for.
    grant: Option<(String, Grant)>,
}

impl SetupNextcloud {
    pub(super) const fn new() -> Self {
        Self {
            asked: None,
            grant: None,
        }
    }

    /// The grant kept for `server`, when there is one.
    pub(in crate::ui) fn held_for(&self, server: &str) -> Option<Grant> {
        self.grant
            .as_ref()
            .filter(|(held, _)| held == server)
            .map(|(_, grant)| grant.clone())
    }

    pub(in crate::ui) fn hold(&mut self, server: String, grant: Grant) {
        self.grant = Some((server, grant));
    }

    /// Takes the kept grant: it became an account, or is to be withdrawn.
    pub(in crate::ui) fn take(&mut self) -> Option<Grant> {
        self.grant.take().map(|(_, grant)| grant)
    }
}

/// The server a manual calendar and contacts pane names: its calendar server, else its address
/// book's. `None` for any other pane, or one that names neither yet.
pub(in crate::ui) fn manual_server(form: &ManualForm) -> Option<&str> {
    if form.kind != AccountKind::Dav {
        return None;
    }
    [form.caldav_url.trim(), form.carddav_url.trim()]
        .into_iter()
        .find(|url| !url.is_empty())
}

impl SetupState {
    /// The browser holds a sign-in with Nextcloud.
    pub(in crate::ui) fn nextcloud_signing_in(&mut self) {
        self.phase = Phase::NextcloudSigningIn;
        self.clear_feedback();
        self.bump();
    }

    /// The sign-in ended without an account: back to the form, saying `said` when there is
    /// something to say.
    pub(in crate::ui) fn nextcloud_ended(&mut self, said: Option<String>) {
        self.phase = Phase::Form;
        self.clear_feedback();
        self.error = said;
        self.bump();
    }

    /// Records what the manual calendar and contacts pane holds now, and answers the server to
    /// ask about when it names one not asked about yet. Does not rebuild: nothing on screen
    /// changes while it is asked, and a rebuild would take the password being typed.
    pub(in crate::ui) fn adopt_manual_dav(&mut self, typed: ManualForm) -> Option<String> {
        if self.phase != Phase::Form || !matches!(self.form, Some(SetupForm::Manual(_))) {
            return None;
        }
        let server = manual_server(&typed).map(str::to_owned);
        self.form = Some(SetupForm::Manual(typed));
        let server = server.filter(|server| self.nextcloud.asked.as_ref() != Some(server))?;
        self.nextcloud.asked = Some(server.clone());
        Some(server)
    }

    /// The server answered. A yes for the server the pane still names draws the offer above its
    /// fields, which a rebuild keeps as they were typed; a no changes nothing on screen.
    pub(in crate::ui) fn nextcloud_answered(&mut self, server: &str, offered: bool) {
        if self.phase != Phase::Form {
            return;
        }
        let Some(SetupForm::Manual(form)) = self.form.as_mut() else {
            return;
        };
        if manual_server(form) != Some(server) {
            return;
        }
        let answer = offered.then(|| server.to_owned());
        if form.nextcloud != answer {
            form.nextcloud = answer;
            self.bump();
        }
    }
}
