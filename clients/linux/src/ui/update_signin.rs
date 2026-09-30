//! The browser sign-ins' half of the input match: Google, Microsoft, JMAP and IMAP.
//!
//! Split from [`super::update`], which had reached the size limit, along the seam the arms
//! already had. All four run the same shape, start, cancel, the captured redirect, the outcome,
//! and none is reached from any other arm, so moving them costs the reader nothing and gives the
//! remaining match back its shape.
//!
//! The caller names every variant it forwards rather than using a wildcard. That is what keeps
//! both halves exhaustive: a new input variant nobody handles is then a compile error, not a
//! message that falls quietly into this file and panics at runtime.

use relm4::ComponentSender;

use super::{AppInput, AppModel};

impl AppModel {
    /// Dispatches one sign-in message.
    ///
    /// Reached only from the arm in [`super::update`] that names every variant below, so the
    /// fallback is genuinely unreachable: a message that is not a sign-in cannot arrive here
    /// without the compiler having refused the caller first.
    pub(super) fn update_sign_in(&mut self, message: AppInput, sender: &ComponentSender<Self>) {
        match message {
            AppInput::StartGoogleLogin(email) => {
                self.start_google_login(email, sender.input_sender().clone());
            }
            AppInput::CancelGoogleLogin => self.cancel_google_login(),
            AppInput::GoogleCallbackReceived(attempt) => {
                self.google_callback_received(attempt);
            }
            AppInput::GoogleFinished(attempt, outcome) => {
                self.google_finished(attempt, outcome, sender.input_sender().clone());
            }
            AppInput::StartMicrosoftLogin(email) => {
                self.start_microsoft_login(email, sender.input_sender().clone());
            }
            AppInput::CancelMicrosoftLogin => self.cancel_microsoft_login(),
            AppInput::MicrosoftCallbackReceived(attempt) => {
                self.microsoft_callback_received(attempt);
            }
            AppInput::MicrosoftFinished(attempt, outcome) => {
                self.microsoft_finished(attempt, outcome, sender.input_sender().clone());
            }
            AppInput::StartJmapLogin(email, server_url) => {
                self.start_jmap_login(email, server_url, sender.input_sender().clone());
            }
            AppInput::ProbeManualImapSignIn(form) => {
                self.probe_manual_imap_sign_in(*form, sender.input_sender().clone());
            }
            AppInput::ImapAuthAnswered {
                email,
                imap_host,
                offer,
            } => {
                self.imap_auth_answered(&email, &imap_host, *offer);
            }
            AppInput::StartImapLogin(form) => {
                self.start_imap_login(&form, sender.input_sender().clone());
            }
            AppInput::CancelImapLogin => self.cancel_imap_login(),
            AppInput::ImapPrepared(attempt, prepared) => {
                self.imap_prepared(attempt, prepared, sender.input_sender().clone());
            }
            AppInput::ImapCallbackReceived(attempt) => {
                self.imap_callback_received(attempt);
            }
            AppInput::ImapFinished(attempt, outcome) => {
                self.imap_finished(attempt, outcome, sender.input_sender().clone());
            }
            AppInput::CancelJmapLogin => self.cancel_jmap_login(),
            AppInput::JmapPrepared(attempt, prepared) => {
                self.jmap_prepared(attempt, prepared, sender.input_sender().clone());
            }
            AppInput::JmapCallbackReceived(attempt) => {
                self.jmap_callback_received(attempt);
            }
            AppInput::JmapFinished(attempt, outcome) => {
                self.jmap_finished(attempt, outcome, sender.input_sender().clone());
            }
            AppInput::JmapReauthPrepared(attempt, prepared) => {
                self.jmap_reauth_prepared(attempt, prepared, sender.input_sender().clone());
            }
            AppInput::JmapReauthFinished(attempt, outcome) => {
                self.jmap_reauth_finished(attempt, outcome);
            }
            other => unreachable!("not a sign-in message: {other:?}"),
        }
    }
}
