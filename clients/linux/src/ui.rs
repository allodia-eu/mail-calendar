//! Relm4 model and GTK/libadwaita three-pane mail shell.

use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
};

use mailcal_bindings::{
    Appearance, Intent, MailboxListSnapshot, MailcalApp, ReplyPrompt, ViewMode,
};
use relm4::{ComponentParts, ComponentSender, SimpleComponent};

use crate::{
    appearance, boot, crash, l10n, logger,
    observer::SurfaceObserver,
    preferences::{self, HostPreferences},
    secrets::SecretStore,
};

mod allodia;
mod allodia_sync;
mod avatar;
mod calendar;
mod calendar_actions;
mod composer;
mod composer_draft;
mod composer_header;
mod composer_model;
mod composer_signature;
mod connectivity;
mod contacts;
mod contacts_actions;
pub(crate) mod destinations;
mod dns;
mod folder_pane;
mod google;
mod host_tasks;
mod input;
mod invitation;
mod invitation_actions;
mod jmap;
mod jmap_actions;
mod mail_actions;
mod mailbox;
mod mailbox_display;
mod mailbox_progressive;
mod mailbox_reconcile;
mod mcp;
mod microsoft;
mod modal;
mod model;
mod notifications;
mod oauth_actions;
mod oauth_loopback;
mod operations;
mod reading;
mod recipients;
mod row_action;
mod runtime_timers;
mod search;
mod search_actions;
mod selection;
mod selection_bar;
mod selection_input;
mod settings;
mod setup;
mod setup_google;
mod setup_imap;
mod setup_jmap;
mod setup_manual;
#[cfg(test)]
mod setup_manual_tests;
mod setup_microsoft;
mod setup_model;
mod setup_onboarding;
#[cfg(test)]
mod setup_onboarding_tests;
#[cfg(test)]
mod setup_widget_tests;
mod setup_widgets;
mod shell;
#[cfg(any(debug_assertions, feature = "dev-harness"))]
mod showcase_hooks;
mod signature_image;
mod time_zone;
mod timestamps;
mod unfiled_copy;
mod update;
mod web_security;
mod webview;
mod welcome;

use calendar::CalendarModel;
use composer_draft::PendingNavigation;
use composer_model::{ComposeKind, ComposeRequest, initial_sender, quote_seed};
use connectivity::ConnectivityState;
use contacts::ContactsModel;
pub(crate) use destinations::PrimaryView;
use host_tasks::HostTasks;
pub(crate) use input::AppInput;
use mail_actions::DeleteTarget;
use mailbox::ThreadKey;
#[cfg(any(debug_assertions, feature = "dev-harness"))]
use model::OpenedMessage;
use model::ReadingState;
use search::SearchState;
use selection::Selection;
use setup::SetupState;
use shell::AppWidgets;
use unfiled_copy::UnfiledCopyNotice;

pub(crate) struct AppModel {
    app: Option<Arc<MailcalApp>>,
    secrets: Option<Arc<SecretStore>>,
    preferences: Arc<HostPreferences>,
    connectivity: ConnectivityState,
    /// Retained for the lifetime of the component so GIO keeps delivering default-network changes.
    _network_monitor: gtk::gio::NetworkMonitor,
    /// Retained so GLib keeps refreshing calendars throughout the foreground session.
    _calendar_refresh_timer: gtk::glib::SourceId,
    /// Retained so GLib keeps checking the device zone throughout the foreground session.
    _device_timezone_timer: gtk::glib::SourceId,
    device_zone: time_zone::DeviceZoneMonitor,
    primary: PrimaryView,
    snapshot: MailboxListSnapshot,
    /// The conversations the user has opened inline. Host state, not the core's: it survives a
    /// snapshot refresh, so a background sync doesn't collapse a conversation being read.
    expanded_threads: HashSet<ThreadKey>,
    /// The rows the user has picked out to act on together. Host state, like the disclosure
    /// above it: transient, never persisted, and read by nothing outside this list
    /// (`docs/list-selection.md`, rule 1).
    selection: Selection,
    calendar: CalendarModel,
    contacts: ContactsModel,
    /// What the mail-search chrome is showing. Not the results: those arrive in `snapshot` like
    /// any other list, so this client keeps no second copy of them.
    search: SearchState,
    reading: ReadingState,
    /// Which reading snapshot the pane has drawn. Bumped on every `Surface::Reading` pull, so the
    /// invitation card is rebuilt when the core publishes a new one and left alone on every other
    /// render; a rebuild mid-render would take a half-typed note to the organiser away.
    reading_generation: u64,
    pending_mail_delete: Option<DeleteTarget>,
    composer: Option<ComposeRequest>,
    composer_generation: u64,
    composer_error: bool,
    /// The message or external draft waiting for the open composer to answer whether it is dirty.
    pending_navigation: Option<PendingNavigation>,
    /// A mail link received before an account exists. Account setup completing opens it.
    pending_mailto: Option<mailcal_bindings::MailtoPrefill>,
    /// The navigation the guard must answer, and the counter it is drawn from. Its own sequence,
    /// not the composer's: two navigations away from one draft: the second after a "Keep editing"
    /// ; must each get an answer, and reusing the composer's generation would make the pane treat
    /// the second as already asked.
    draft_check: Option<u64>,
    draft_check_seq: u64,
    /// Whether the "Discard draft?" question is on screen.
    discard_prompt: bool,
    notice: Option<String>,
    /// The mail list's bottom-bar caption while a background sync is downloading mail. `None`
    /// whenever nothing is arriving unasked, which is almost always.
    sync_hint: Option<String>,
    /// The separate foreground-download row. It wins the shared bottom strip while active.
    sync_bar: Option<model::SyncBar>,
    unfiled_copy: Option<UnfiledCopyNotice>,
    /// The standing "the organiser wasn't told" question, mirrored from the core. `None` is also
    /// how the core says *close the modal*; it clears the question the moment it is answered.
    reply_prompt: Option<ReplyPrompt>,
    /// Which question the modal is showing. The core carries no id, so the host counts, exactly as
    /// the calendar's dialogs are counted.
    reply_prompt_generation: u64,
    boot_error: Option<String>,
    webview_available: bool,
    calendar_refresh: calendar_actions::CalendarRefreshGate,
    calendar_manager_generation: u64,
    setup: SetupState,
    /// What this launch knows about the person's other devices, apart from the pass itself.
    allodia: allodia_sync::AllodiaLaunch,
    credential_repair_failed: Option<String>,
    /// What the next Settings render should show, and whether it opens the window or only redraws
    /// an open one ([`settings::SettingsState`]).
    settings: settings::SettingsState,
    host_tasks: HostTasks,
    /// The screenshot screen still waiting on something asynchronous. Only `Reply` ever waits: it
    /// cannot begin until the opened message's body has arrived, and that arrives on the observer.
    #[cfg(any(debug_assertions, feature = "dev-harness"))]
    showcase_pending: Option<crate::showcase::ShowcaseScreen>,
}

impl SimpleComponent for AppModel {
    type Init = ();
    type Input = AppInput;
    type Output = ();
    type Root = adw::ApplicationWindow;
    type Widgets = AppWidgets;

    fn init_root() -> Self::Root {
        adw::ApplicationWindow::new(&relm4::main_adw_application())
    }

    fn init(
        (): Self::Init,
        root: Self::Root,
        sender: ComponentSender<Self>,
    ) -> ComponentParts<Self> {
        let input = sender.input_sender().clone();
        let observer = SurfaceObserver::new(input.clone());
        let (app, secrets, snapshot, boot_error, allodia_syncable) =
            match boot::app(Box::new(observer)) {
                Ok(booted) => {
                    let snapshot = booted.app.mailbox_list();
                    (
                        Some(booted.app),
                        booted.secrets,
                        snapshot,
                        None,
                        booted.syncable,
                    )
                }
                Err(error) => (None, None, model::empty_mailbox(), Some(error), false),
            };
        // The core installed the log sink on the way through, so GTK's own warnings and criticals
        // now have somewhere to land (crate::crash).
        crash::capture_toolkit_diagnostics();
        // The same moment, for the same reason: a fault record needs the file to exist. Linux has
        // no tombstone and no Error Reporting, so the shared log is the only place a segfault in
        // the core or in GTK leaves a trace the user can hand over.
        mailcal_bindings::watch_for_native_faults(
            logger::diagnostic_log_path().to_string_lossy().into_owned(),
        );
        // Before the window is presented, so it is never painted in the desktop's scheme first.
        // The demo and the showcase have no store to have persisted a choice in, so they follow
        // the desktop unless a launch override says otherwise.
        appearance::apply(appearance::at_launch(
            app.as_deref()
                .map_or(Appearance::System, |app| app.display_settings().appearance),
        ));
        let requires_setup = secrets.is_some() && snapshot.accounts.is_empty();
        let flow = welcome::initial_flow(
            secrets.is_some() || welcome::force_in_fixture(),
            requires_setup,
            app.as_deref()
                .is_none_or(|app| app.analytics_consent().asked),
        );
        let welcome_pending = flow.welcome;
        mcp::install(app.as_deref(), input.clone());
        if !welcome_pending && let Some(app) = &app {
            app.report_app_opened();
        }
        // What the person's other devices have to say. Through the input queue rather than
        // inline: the model does not exist yet, and the pass blocks on the network.
        input.emit(AppInput::SyncAllodiaAccounts);
        input.emit(AppInput::ReadAccountsSynced);
        let mut setup = SetupState::closed();
        // Whether the card is on offer at all is a property of the build, not of the window, so it
        // is set before either is open. Setting it here rather than beside `open` below is what
        // covers the run where consent comes first: the first-account screen is then opened from
        // the welcome window's answer, several inputs later and nowhere near this file.
        // `signed_in` and the offers are absent by construction: an install with no accounts has
        // not run a pass.
        setup.set_onboarding(setup_onboarding::Onboarding {
            offered: mailcal_bindings::allodia_sign_in_available(),
            ..setup_onboarding::Onboarding::default()
        });
        if flow.setup {
            setup.open(true);
        }
        let calendar = CalendarModel::new(app.as_deref());
        // The core can begin an awaited download before the observer is subscribed. Pull the
        // current progress for the first frame so that opening an unsynced folder never depends on
        // a later progress edge to make its already-active wait visible.
        let (sync_bar, sync_hint) = app.as_deref().map_or((None, None), |app| {
            let progress = app.sync_progress();
            (
                model::sync_bar(&progress),
                model::sync_hint(&progress, &snapshot.accounts),
            )
        });
        // Pull once: a boot outage's signal fired before this model existed.
        let (connectivity, network_monitor) =
            connectivity::at_launch(app.as_deref(), &snapshot.accounts, &input);
        let calendar_input = input.clone();
        let mut calendar_refreshes_remaining = runtime_timers::calendar_refresh_limit();
        let calendar_refresh_timer =
            gtk::glib::timeout_add_local(runtime_timers::calendar_refresh_interval(), move || {
                calendar_input.emit(AppInput::PeriodicCalendarRefresh);
                if let Some(remaining) = &mut calendar_refreshes_remaining {
                    *remaining = remaining.saturating_sub(1);
                    if *remaining == 0 {
                        return gtk::glib::ControlFlow::Break;
                    }
                }
                gtk::glib::ControlFlow::Continue
            });
        let timezone_input = input.clone();
        let device_timezone_timer =
            gtk::glib::timeout_add_local(time_zone::poll_interval(), move || {
                timezone_input.emit(AppInput::CheckDeviceTimeZone);
                gtk::glib::ControlFlow::Continue
            });
        let model = Self {
            app,
            secrets,
            preferences: preferences::global(),
            connectivity,
            _network_monitor: network_monitor,
            _calendar_refresh_timer: calendar_refresh_timer,
            _device_timezone_timer: device_timezone_timer,
            device_zone: time_zone::DeviceZoneMonitor::new(mailcal_bindings::device_time_zone()),
            primary: PrimaryView::Mail,
            snapshot,
            expanded_threads: HashSet::new(),
            selection: Selection::default(),
            calendar,
            contacts: ContactsModel::default(),
            search: SearchState::default(),
            reading: ReadingState::new(model::empty_reading()),
            reading_generation: 0,
            pending_mail_delete: None,
            composer: None,
            composer_generation: 0,
            composer_error: false,
            pending_navigation: None,
            pending_mailto: None,
            draft_check: None,
            draft_check_seq: 0,
            discard_prompt: false,
            notice: None,
            sync_hint,
            sync_bar,
            unfiled_copy: None,
            reply_prompt: None,
            reply_prompt_generation: 0,
            boot_error,
            webview_available: true,
            calendar_refresh: calendar_actions::CalendarRefreshGate::default(),
            calendar_manager_generation: 0,
            setup,
            allodia: allodia_sync::AllodiaLaunch {
                syncable: allodia_syncable,
                accounts_synced: HashMap::new(),
            },
            credential_repair_failed: None,
            settings: settings::SettingsState::default(),
            host_tasks: HostTasks::new(welcome_pending, flow.setup_after_welcome),
            #[cfg(any(debug_assertions, feature = "dev-harness"))]
            showcase_pending: None,
        };
        #[cfg(any(debug_assertions, feature = "dev-harness"))]
        let model = {
            let mut model = model;
            model.apply_debug_open_hook();
            if std::env::var_os("MAILCAL_CALENDAR").is_some() {
                model.show_calendar();
            }
            model.begin_showcase(sender.input_sender());
            model
        };
        let mut widgets = AppWidgets::new(root, input);
        widgets.render(&model);
        ComponentParts { model, widgets }
    }

    fn update(&mut self, message: Self::Input, sender: ComponentSender<Self>) {
        self.update_message(message, &sender);
    }

    fn update_view(&self, widgets: &mut Self::Widgets, _sender: ComponentSender<Self>) {
        widgets.render(self);
    }
}

impl AppModel {
    fn dispatch(&self, intent: Intent) {
        if let Some(app) = &self.app {
            app.dispatch(intent);
        }
    }

    /// Whether the account-setup window may open. It needs somewhere to put the credential it
    /// will collect; except in a showcase build, which has no credential store *by design*
    /// (nothing a screenshot run does may reach the developer's keyring) and whose whole
    /// purpose on this screen is to photograph the window. Without the exception the
    /// `add-account` capture silently photographed the message list instead.
    fn can_open_account_setup(&self) -> bool {
        #[cfg(any(debug_assertions, feature = "dev-harness"))]
        if crate::showcase::is_on() {
            return true;
        }
        self.secrets.is_some()
    }

    #[cfg(any(debug_assertions, feature = "dev-harness"))]
    fn open_row(&mut self, index: usize) {
        let Some(row) = self.snapshot.rows.get(index) else {
            return;
        };
        self.open_message(OpenedMessage::from_row(row));
    }

    fn begin_compose(&mut self, kind: ComposeKind) {
        let Some(app) = &self.app else {
            return;
        };
        let opened = self.reading.opened.as_ref();
        if kind != ComposeKind::New && opened.is_none() {
            return;
        }
        let (initial_to, initial_cc) = match (kind, opened) {
            (ComposeKind::Reply | ComposeKind::ReplyAll, Some(message)) => {
                let recipients = app.reply_recipients(
                    message.account.clone(),
                    message.key.clone(),
                    kind == ComposeKind::ReplyAll,
                );
                (recipients.to, recipients.cc)
            }
            _ => (String::new(), String::new()),
        };
        let quote = opened.and_then(|message| {
            let settings = app.quote_settings();
            // Only a showcase reply arrives pre-written, so the store screenshot shows a written
            // reply rather than an empty body. Every real reply passes `None`.
            #[cfg(any(debug_assertions, feature = "dev-harness"))]
            let initial_text = (kind == ComposeKind::Reply || kind == ComposeKind::ReplyAll)
                .then(|| crate::showcase::reply_text(&message.account, &message.key))
                .flatten();
            #[cfg(not(any(debug_assertions, feature = "dev-harness")))]
            let initial_text: Option<String> = None;
            quote_seed(
                message,
                &self.reading.snapshot,
                &settings.style,
                kind == ComposeKind::Forward,
                initial_text.as_deref(),
                self.calendar.display_zone(),
            )
        });
        let subject = match (kind, opened) {
            (ComposeKind::Reply | ComposeKind::ReplyAll, Some(message)) => {
                l10n::subject_reply(&message.subject)
            }
            (ComposeKind::Forward, Some(message)) => l10n::subject_forward(&message.subject),
            _ => String::new(),
        };
        self.composer_generation = self.composer_generation.wrapping_add(1);
        self.composer_error = false;
        self.composer = Some(ComposeRequest {
            kind,
            account: opened.map(|message| message.account.clone()),
            key: opened.map(|message| message.key.clone()),
            initial_to,
            initial_cc,
            initial_bcc: String::new(),
            subject,
            initial_body: None,
            quote,
            initial_from: initial_sender(
                opened,
                self.snapshot.selected_account.as_deref(),
                app.default_send_account(),
            ),
            seeds_signature: true,
        });
    }

    /// What the message list calls itself: the folder on screen, or; while a search is running
    /// ; the results, as macOS and Windows label the same list.
    fn list_title(&self) -> String {
        if self.search.is_active() {
            return l10n::search_results().to_owned();
        }
        folder_pane::header_title(&self.snapshot)
    }

    fn subtitle(&self) -> String {
        if let Some(error) = &self.boot_error {
            return l10n::status_connect_failed(error);
        }
        let total = i64::try_from(self.snapshot.total).unwrap_or(i64::MAX);
        match self.snapshot.mode {
            ViewMode::Flat => l10n::mailbox_count_messages(total),
            ViewMode::Threaded => l10n::mailbox_count_conversations(total),
        }
    }
}
