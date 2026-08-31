//! Relm4 input dispatcher, split from the model construction and projection helpers.

use adw::prelude::*;
use mailcal_bindings::{Intent, SendStatus, Surface};
use relm4::ComponentSender;

use super::{
    AppInput, AppModel, PrimaryView,
    composer_model::ComposeKind,
    connectivity::{ConnectivityState, ExpiredResolution},
    model, settings,
    setup_model::{self, DetectedForm, OAuthForm, SetupForm},
    unfiled_copy::UnfiledCopyNotice,
};
use crate::l10n;

impl AppModel {
    fn pull(&mut self, surface: &Surface) {
        let Some(app) = self.app.clone() else {
            return;
        };
        match surface {
            Surface::MailboxList => self.pull_mailbox(&app),
            Surface::Reading => {
                self.reading.snapshot = app.reading_view();
                self.reading_generation = self.reading_generation.wrapping_add(1);
                #[cfg(any(debug_assertions, feature = "dev-harness"))]
                self.continue_showcase();
            }
            Surface::Sending => {
                self.notice = match app.send_status() {
                    SendStatus::Sending => Some(l10n::send_status_sending().to_owned()),
                    SendStatus::Sent => Some(l10n::send_status_sent().to_owned()),
                    SendStatus::Failed => Some(l10n::send_status_failed().to_owned()),
                    // Nothing to show, for two different reasons: nothing is in flight, and for
                    // `SentNotFiled` the standing UnfiledCopy question already says it: with a
                    // button, which a transient hint has nowhere to put.
                    SendStatus::Idle | SendStatus::SentNotFiled => None,
                };
            }
            // The question raised when a calendar server stored the answer and then reported it
            // could not pass it on. The core holds it and clears it; this only mirrors.
            Surface::InvitationReply => {
                self.reply_prompt = app.reply_prompt();
                self.reply_prompt_generation = self.reply_prompt_generation.wrapping_add(1);
            }
            Surface::UnfiledCopy => {
                self.unfiled_copy = app.unfiled_copy().map(|copy| UnfiledCopyNotice {
                    body: l10n::unfiled_copy_body(&copy.subject),
                    retrying: copy.retrying,
                });
            }
            // Both progress surfaces share one snapshot and one strip. The foreground bar wins
            // while active; the hint is the quiet caption for a pass nobody started.
            Surface::SyncProgress => {
                let progress = app.sync_progress();
                self.sync_bar = model::sync_bar(&progress);
                self.sync_hint = model::sync_hint(&progress, &self.snapshot.accounts);
            }
            Surface::Calendar => self.calendar.refresh(&app),
            Surface::Contacts => self.contacts.refresh(&app),
            Surface::CalendarStatus => self.calendar.refresh_write_status(&app),
            Surface::Settings => self.calendar.refresh_settings(&app),
            Surface::Connectivity => {
                self.connectivity = ConnectivityState::pull(&app, &self.snapshot.accounts);
            }
        }
    }

    pub(super) fn update_message(&mut self, message: AppInput, sender: &ComponentSender<Self>) {
        match message {
            AppInput::RefreshRequested => self.dispatch(Intent::RefreshMail),
            AppInput::RetryUnfiledCopy => self.dispatch(Intent::RetryUnfiledCopy),
            AppInput::DismissUnfiledCopy => self.dispatch(Intent::DismissUnfiledCopy),
            AppInput::RefreshCalendar => self.dispatch(Intent::RefreshCalendar),
            AppInput::PeriodicCalendarRefresh => {
                if self.calendar_refresh.periodic_refresh()
                    && let Some(app) = &self.app
                {
                    app.refresh_calendar_in_background();
                }
            }
            AppInput::SurfaceChanged(surface) => {
                let collect_new_mail = matches!(surface, Surface::MailboxList);
                self.pull(&surface);
                if collect_new_mail {
                    sender.input_sender().emit(AppInput::CollectNewMail);
                }
            }
            AppInput::NetworkReachabilityChanged(reachable) => {
                self.dispatch(Intent::ReportNetworkReachable { reachable });
            }
            AppInput::CheckDeviceTimeZone => {
                if let Some(zone) = self.device_zone.changed(super::time_zone::device_zone()) {
                    self.dispatch(Intent::ReportDeviceTimeZone { id: zone });
                }
            }
            AppInput::AcceptDeviceTimeZone => {
                self.dispatch(Intent::AcceptTimeZoneChange);
            }
            AppInput::DismissDeviceTimeZone => {
                self.dispatch(Intent::DismissTimeZoneChange);
            }
            AppInput::ResolveExpiredSignIn => {
                self.resolve_expired_signin(sender.input_sender().clone());
            }
            AppInput::ResolveMailReauth => {
                self.resolve_microsoft_reauth(false, sender.input_sender().clone());
            }
            AppInput::ResolveCalendarReauth => {
                self.resolve_microsoft_reauth(true, sender.input_sender().clone());
            }
            AppInput::ShowMail => self.primary = PrimaryView::Mail,
            AppInput::ShowCalendar => self.show_calendar(),
            AppInput::ShowContacts => self.show_contacts(),
            AppInput::RefreshContacts => self.dispatch(Intent::RefreshContacts),
            AppInput::SearchContacts(query) => self.search_contacts(query),
            AppInput::OpenContact(id) => {
                self.open_contact(id, sender.input_sender().clone());
            }
            AppInput::ContactOpened(lookup, detail) => {
                self.contact_opened(lookup, detail.as_deref());
            }
            AppInput::SetCalendarMode(mode) => {
                if let Some(app) = &self.app {
                    self.calendar.set_mode(app, mode);
                }
            }
            AppInput::StepCalendar(direction) => {
                if let Some(app) = &self.app {
                    self.calendar.step(app, direction);
                }
            }
            AppInput::CalendarToday => {
                if let Some(app) = &self.app {
                    self.calendar.jump_today(app);
                }
            }
            AppInput::ManageCalendars => {
                self.calendar_manager_generation = self.calendar_manager_generation.wrapping_add(1);
            }
            AppInput::ShowCalendarDay(date) => {
                if let Some(app) = &self.app {
                    self.calendar.show_day(app, &date);
                }
            }
            AppInput::ToggleAllDay => self.calendar.toggle_all_day(),
            AppInput::OpenCalendarEvent(event) => {
                if let Some(app) = &self.app {
                    self.calendar.open_event(app, event);
                }
            }
            AppInput::BeginNewEvent => self.calendar.begin_create(&self.snapshot),
            AppInput::BeginNewEventAt(slot) => {
                self.calendar.begin_create_at(&self.snapshot, slot);
            }
            AppInput::BeginEditEvent => self.calendar.begin_edit(&self.snapshot),
            AppInput::SubmitEventForm(form, this_occurrence_only) => {
                self.submit_event(&form, this_occurrence_only);
            }
            AppInput::RequestDeleteEvent(event) => self.request_delete_event(event),
            AppInput::RequestDeleteCurrentEvent => self.calendar.request_delete_current(),
            AppInput::DeleteCalendarEvent(event) => {
                self.dispatch(Intent::DeleteEvent {
                    account: event.account,
                    key: event.key,
                    // Empty means the whole series: the surface drew no single occurrence, or
                    // the user answered *All events*, which clears it.
                    occurrence: Some(event.occurrence).filter(|at| !at.is_empty()),
                });
                self.calendar.dismiss_dialog();
            }
            AppInput::DismissCalendarDialog => self.calendar.dismiss_dialog(),
            AppInput::SearchMail(query) => self.search_mail(query),
            AppInput::SetSearchScope(scope) => self.set_search_scope(scope),
            AppInput::OpenSyncDepthSettings => self.open_sync_depth_settings(),
            AppInput::OpenThreadMessage(message) => self.open_message(*message),
            AppInput::SetThreadExpanded { thread, expanded } => {
                self.set_thread_expanded(&thread, expanded);
            }
            AppInput::RespondToInvitation(answer) => self.respond_to_invitation(*answer),
            AppInput::AnswerReplyPrompt {
                send,
                remember,
                reply_subject,
            } => {
                // The two flags only. The subject is not logged: it *contains* the meeting's title,
                // which is message content (`docs/invitations.md` → Logging and privacy).
                log::info!("invitation: reply question answered send={send} remember={remember}");
                self.dispatch(Intent::AnswerReplyPrompt {
                    send,
                    remember,
                    reply_subject: Some(reply_subject),
                });
            }
            AppInput::PerformMailAction(request) => self.perform_mail_action(*request),
            AppInput::PerformOpenedMailAction(action) => self.perform_opened_mail_action(action),
            AppInput::RequestPermanentDelete(target) => {
                self.pending_mail_delete = Some(target);
            }
            AppInput::DismissPermanentDelete => self.pending_mail_delete = None,
            AppInput::ArchiveThread { account, thread_id } => {
                self.archive_thread(&account, &thread_id);
            }
            AppInput::ActivateSidebar(target) => self.activate_sidebar(&target),
            AppInput::SetAccountExpanded { account, expanded } => {
                self.dispatch(Intent::SetAccountExpanded { account, expanded });
            }
            AppInput::LoadRemoteImages => self.reading.load_remote_images = true,
            AppInput::RetryOpen => self.retry_open(),
            AppInput::OpenMailto(prefill) => self.open_mailto(*prefill),
            AppInput::OpenAgentDraft(draft) => {
                self.open_agent_draft(*draft);
                if let Some(window) = relm4::main_adw_application().active_window() {
                    window.present();
                }
            }
            AppInput::BeginNew => self.begin_compose(ComposeKind::New),
            AppInput::BeginReply(reply_all) => self.begin_compose(if reply_all {
                ComposeKind::ReplyAll
            } else {
                ComposeKind::Reply
            }),
            AppInput::BeginForward => self.begin_compose(ComposeKind::Forward),
            AppInput::CancelComposer => self.composer = None,
            AppInput::ComposerDraftChecked(edited) => self.draft_checked(edited),
            AppInput::DiscardDraft => self.take_pending_navigation(),
            AppInput::KeepEditing => self.keep_editing(),
            AppInput::SubmitComposer(submission) => self.submit_composer(&submission),
            AppInput::SaveAttachment { id, destination } => {
                self.save_attachment(id, destination, sender.input_sender().clone());
            }
            AppInput::OpenAttachment { id, file_name } => {
                self.open_attachment(id, &file_name, sender.input_sender().clone());
            }
            AppInput::AttachmentSaved(saved) => {
                self.notice = Some(
                    if saved {
                        l10n::attachment_saved()
                    } else {
                        l10n::attachment_save_failed()
                    }
                    .to_owned(),
                );
            }
            AppInput::AttachmentDecoded(result) => {
                self.launch_attachment(result, sender.input_sender().clone());
            }
            AppInput::AttachmentOpenFailed => {
                self.notice = Some(l10n::attachment_open_failed().to_owned());
            }
            AppInput::WebViewReady => {}
            AppInput::WebViewUnavailable => {
                self.webview_available = false;
                self.composer_error = self.composer.is_some();
            }
            // The Settings button names no category, so it reopens on the last one asked for.
            AppInput::OpenSettings => self.settings.open(None),
            AppInput::OpenAccountSetup => {
                if self.can_open_account_setup() {
                    self.setup.open(false);
                }
            }
            AppInput::RestartAccountSetup => {
                self.setup.open(self.snapshot.accounts.is_empty());
                // Removing the last account reopens the first-account screen, and a sign-in may
                // have happened since boot; so the card is re-derived rather than left as it was.
                self.refresh_onboarding_card();
            }
            AppInput::CancelAccountSetup => self.setup.cancel(),
            AppInput::ManualAccountSetup(email) => {
                self.setup.show_form(setup_model::manual_form(email, None));
            }
            AppInput::EditDetectedManually => {
                self.edit_detected_manually(sender.input_sender().clone());
            }
            AppInput::SelectAccountKind(form) => {
                self.select_account_kind(*form, sender.input_sender().clone());
            }
            AppInput::ProbeManualJmapSignIn(form) => {
                self.probe_manual_jmap_sign_in(*form, sender.input_sender().clone());
            }
            AppInput::DetectAccount(email) => {
                self.detect_account(email, sender.input_sender().clone());
            }
            AppInput::AccountDetected(email, recommendation) => {
                self.account_detected(email, *recommendation, sender.input_sender().clone());
            }
            AppInput::JmapOAuthAvailable {
                email,
                server_url,
                available,
            } => {
                self.jmap_oauth_available(&email, &server_url, available);
            }
            AppInput::SubmitAccount(submission) => {
                self.submit_account(*submission, sender.input_sender().clone());
            }
            AppInput::AccountAdded(result) => {
                self.account_added(result);
                // The account list changed, so the person's other devices should hear about it now
                // rather than at the next launch. A no-op when nobody is signed in.
                self.sync_after_account_change(sender.input_sender().clone());
            }
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
            AppInput::StartAllodiaSignIn => {
                self.start_allodia_sign_in(sender.input_sender().clone());
            }
            AppInput::StartAllodiaRegistration => {
                self.start_allodia(sender.input_sender().clone(), true);
            }
            AppInput::ManageAllodiaAccount => self.manage_allodia_account(),
            AppInput::CancelAllodiaSignIn => self.cancel_allodia_sign_in(),
            AppInput::AllodiaSignInSlow(attempt) => self.allodia_sign_in_slow(attempt),
            AppInput::AllodiaSignInFinished(attempt, outcome) => {
                self.allodia_sign_in_finished(attempt, outcome, sender.input_sender().clone());
            }
            AppInput::SignOutOfAllodia => self.sign_out_of_allodia(),
            AppInput::SyncAllodiaAccounts => {
                self.sync_allodia_accounts(sender.input_sender().clone());
            }
            AppInput::AllodiaSyncFinished(outcome) => self.allodia_sync_finished(outcome),
            AppInput::ReadAccountsSynced => self.read_accounts_synced(),
            AppInput::SettingsCategoryShown(category) => {
                self.settings.record_category(category);
            }
            AppInput::SetAllodiaAccountSyncMode(account_id, mode) => {
                self.set_allodia_account_sync_mode(
                    &account_id,
                    mode,
                    sender.input_sender().clone(),
                );
            }
            AppInput::AllodiaSyncModeChanged(account_id, failure) => {
                self.allodia_sync_mode_changed(&account_id, failure);
            }
            AppInput::SetUpOfferedAccount(offer) => {
                self.set_up_offered_account(*offer, sender.input_sender().clone());
            }
            AppInput::ReplaceAccountSecret { account, secret } => {
                self.replace_account_secret(account, secret, sender.input_sender().clone());
            }
            AppInput::AccountSecretReplaced { account, success } => {
                self.account_secret_replaced(account, success);
            }
            AppInput::RemoveAccount(id) => {
                self.remove_account(id, sender.input_sender().clone());
            }
            AppInput::AccountRemoved(result) => match result {
                Ok(()) => {
                    if let Some(app) = &self.app {
                        self.snapshot = app.mailbox_list();
                    }
                }
                // The account is still connected when the keyring write fails, so say what
                // happened in the app's own voice rather than surfacing a bare D-Bus string.
                Err(error) => self.notice = Some(l10n::remove_account_failed(&error)),
            },
            AppInput::AnalyticsDecided(enabled) => {
                if let Some(app) = &self.app {
                    app.set_analytics_consent(enabled);
                    app.report_app_opened();
                }
                self.host_tasks.welcome_pending = false;
                if self.host_tasks.setup_after_welcome {
                    self.setup.open(true);
                    self.host_tasks.setup_after_welcome = false;
                }
            }
            AppInput::CollectNewMail => {
                self.collect_new_mail(sender.input_sender().clone());
            }
            AppInput::BackgroundFinished => {
                self.background_finished(sender.input_sender());
            }
        }
    }

    fn resolve_expired_signin(&mut self, sender: relm4::Sender<AppInput>) {
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

    fn resolve_microsoft_reauth(&mut self, calendar: bool, sender: relm4::Sender<AppInput>) {
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
