//! Resolving a sign-in the server has stopped accepting, or a grant missing a permission: which
//! route the banner's button takes, and which account it takes it for. Split from `update` to
//! keep each file under the 500-line limit.

use super::{
    AppInput, AppModel, account_consent::ConsentFrom, connectivity::ExpiredResolution, settings,
};

impl AppModel {
    pub(super) fn resolve_expired_signin(&mut self, sender: relm4::Sender<AppInput>) {
        match self.connectivity.expired_resolution() {
            Some(ExpiredResolution::SignInAgain(account)) => {
                self.start_account_consent(account, Vec::new(), ConsentFrom::MainWindow, sender);
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

    /// The mail or calendar permission banner's Reconnect: one sign-in asks again for everything
    /// the account is used for, so it answers both banners at once (`docs/provider-oauth.md`
    /// rule 11).
    pub(super) fn resolve_permission_reauth(
        &mut self,
        calendar: bool,
        sender: relm4::Sender<AppInput>,
    ) {
        let accounts = if calendar {
            &self.connectivity.calendar_reauth
        } else {
            &self.connectivity.mail_reauth
        };
        let Some(account) = accounts.first().map(|account| account.id.clone()) else {
            return;
        };
        self.start_account_consent(account, Vec::new(), ConsentFrom::MainWindow, sender);
    }
}
