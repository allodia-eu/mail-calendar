//! Nextcloud's remote wipe (`docs/nextcloud-sign-in.md`): when a server refuses the app password
//! a Nextcloud sign-in stored, it is asked once whether the person chose "Wipe device" for it, and
//! if so the account and everything the device holds of it go, as a removal would take them.
//!
//! A refusal is learned wherever it happens (at launch, on a redial, in a sync pass), so it
//! reaches this module through the app's observer rather than through any one of those paths.

use std::sync::{Arc, Weak, mpsc};

use mailcal_app::{AppObserver, Surface};

use crate::{MailcalApp, nextcloud::setup_client};

/// Forwards every signal to `inner`, and each account whose stored sign-in its server refused to
/// the thread [`answer_refusals`] runs as well.
pub(crate) struct RefusalRelay {
    pub(crate) inner: Arc<dyn AppObserver>,
    pub(crate) refused: mpsc::Sender<String>,
}

impl AppObserver for RefusalRelay {
    fn surface_changed(&self, surface: Surface) {
        self.inner.surface_changed(surface);
    }

    fn signin_refused(&self, account: &engine_api::AccountId) {
        // A closed receiver is an app shutting down, which has nothing left to wipe.
        let _ = self.refused.send(account.as_str().to_owned());
        self.inner.signin_refused(account);
    }
}

/// Answers each refusal `refused` carries, on a thread of its own: the answer is a request and
/// possibly a removal, neither of which may wait on the thread that noticed the refusal. Ends when
/// the app does.
pub(crate) fn answer_refusals(app: Weak<MailcalApp>, refused: mpsc::Receiver<String>) {
    let spawned = std::thread::Builder::new()
        .name("nextcloud-wipe".to_owned())
        .spawn(move || {
            for account in refused {
                let Some(app) = app.upgrade() else {
                    return;
                };
                app.answer_refusal(&account);
            }
        });
    if spawned.is_err() {
        log::warn!("nextcloud: could not start asking about remote wipes");
    }
}

impl MailcalApp {
    /// Asks the server that issued `id`'s app password whether the refusal was a wipe, and wipes
    /// the account when it was. Any other answer leaves the account waiting for a new sign-in.
    pub(crate) fn answer_refusal(&self, id: &str) {
        let Some((root, credentials)) = self.registry.nextcloud_credentials(id) else {
            return;
        };
        let Ok(client) = setup_client() else {
            return;
        };
        let handle = mailcal_account::account_log_handle(id);
        let password = &credentials.app_password;
        let asked = self.runtime.block_on(|| async {
            mailcal_nextcloud::wipe_requested(&client, &root, password).await
        });
        match asked {
            Ok(true) => {}
            Ok(false) => {
                log::info!("nextcloud: [{handle}] the password was refused, and no wipe was asked");
                return;
            }
            Err(err) => {
                log::info!("nextcloud: [{handle}] could not ask whether to wipe ({err})");
                return;
            }
        }
        log::warn!("nextcloud: [{handle}] the server asked this device to wipe the account");
        if let Err(err) = self.remove_account_here(id) {
            log::warn!(
                "nextcloud: [{handle}] the wipe could not erase the stored credential ({err})"
            );
            return;
        }
        let done = self.runtime.block_on(|| async {
            mailcal_nextcloud::wipe_finished(&client, &root, password).await
        });
        match done {
            Ok(()) => log::info!("nextcloud: [{handle}] wiped, and the server was told"),
            Err(err) => log::info!("nextcloud: [{handle}] wiped; the server was not told ({err})"),
        }
    }
}

#[cfg(test)]
#[path = "nextcloud_wipe_tests.rs"]
mod nextcloud_wipe_tests;
