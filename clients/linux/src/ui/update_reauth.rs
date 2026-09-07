//! Resolving a sign-in the server has stopped accepting: which route the banner's button takes,
//! and which account it takes it for. Split from `update` to keep each file under the 500-line
//! limit.

use super::{
    AppInput, AppModel,
    connectivity::ExpiredResolution,
    settings,
    setup_model::{DetectedForm, OAuthForm, SetupForm},
};

impl AppModel {
    pub(super) fn resolve_expired_signin(&mut self, sender: relm4::Sender<AppInput>) {
        match self.connectivity.expired_resolution() {
            Some(ExpiredResolution::Microsoft(email)) => {
                self.setup.open(false);
                self.setup
                    .show_form(SetupForm::Detected(DetectedForm::Microsoft(OAuthForm {
                        email: email.clone(),
                    })));
                self.start_microsoft_login(email, sender);
            }
            Some(ExpiredResolution::Google(email)) => {
                self.setup.open(false);
                self.setup
                    .show_form(SetupForm::Detected(DetectedForm::Google(OAuthForm {
                        email: email.clone(),
                    })));
                self.start_google_login(email, sender);
            }
            Some(ExpiredResolution::JmapOauth(account)) => {
                self.start_jmap_reauth(account, sender);
            }
            Some(ExpiredResolution::Settings) => {
                self.settings.open(Some(settings::Category::Accounts));
            }
            None => {}
        }
    }

    pub(super) fn resolve_microsoft_reauth(
        &mut self,
        calendar: bool,
        sender: relm4::Sender<AppInput>,
    ) {
        let email = if calendar {
            self.connectivity.calendar_reauth_emails.first()
        } else {
            self.connectivity.mail_reauth_emails.first()
        };
        let Some(email) = email.cloned() else {
            return;
        };
        self.setup.open(false);
        self.setup
            .show_form(SetupForm::Detected(DetectedForm::Microsoft(OAuthForm {
                email: email.clone(),
            })));
        self.start_microsoft_login(email, sender);
    }
}
