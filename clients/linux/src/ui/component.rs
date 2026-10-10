//! How the Relm4 component runs the window: the launch, the input queue and the render pass, and
//! how [`AppModel`] comes up once the core has opened.
//!
//! Split from [`super`], which had reached the size limit, along the seam that was already
//! there: that module says what the model *is* and which modules exist, this one says how it
//! comes up and what it does with a message. A child module reaches its parent's private
//! items, so nothing here needed widening to move.

use std::collections::{HashMap, HashSet};

use gtk::prelude::GtkWindowExt;
use relm4::{ComponentParts, ComponentSender, SimpleComponent};

use super::{
    AppInput, AppModel, AppWindow, PrimaryView, SetupState, allodia_sync, calendar::CalendarModel,
    calendar_actions, composer_drafts, connectivity, contacts::ContactsModel,
    host_tasks::HostTasks, launch, mcp, model, model::ReadingState, preferences, runtime_timers,
    search::SearchState, selection::Selection, settings, setup_onboarding, sync_line, time_zone,
    welcome,
};
use crate::{appearance, boot, boot::BootedApp, crash, l10n, logger};

impl SimpleComponent for AppWindow {
    type Init = ();
    type Input = AppInput;
    type Output = ();
    type Root = adw::ApplicationWindow;
    type Widgets = launch::WindowWidgets;

    fn init_root() -> Self::Root {
        let root = adw::ApplicationWindow::new(&relm4::main_adw_application());
        // Here rather than with the shell: the window is presented on the launch page, and a
        // default size set after that is not the size it opens at.
        root.set_title(Some(l10n::app_title()));
        root.set_default_width(1280);
        root.set_default_height(800);
        root
    }

    fn init(
        (): Self::Init,
        root: Self::Root,
        sender: ComponentSender<Self>,
    ) -> ComponentParts<Self> {
        // Before the window is presented, so it is never painted in the desktop's scheme first.
        // Read from the store's own file, because the core that owns the setting is not open yet.
        appearance::apply(appearance::at_launch(boot::stored_appearance()));
        let widgets = launch::WindowWidgets::new(root, sender.input_sender().clone());
        launch::open_core(sender.input_sender());
        ComponentParts {
            model: Self::Opening(launch::Opening::default()),
            widgets,
        }
    }

    fn update(&mut self, message: Self::Input, sender: ComponentSender<Self>) {
        match self {
            Self::Open(model) => model.update_message(message, &sender),
            Self::Opening(opening) => match message {
                AppInput::LaunchStatusDue => opening.status_due(),
                AppInput::Booted(booted) => {
                    let held = opening.take_held();
                    let mut model = Box::new(AppModel::open(booted, sender.input_sender()));
                    #[cfg(any(debug_assertions, feature = "dev-harness"))]
                    {
                        model.apply_debug_open_hook();
                        if std::env::var_os("MAILCAL_CALENDAR").is_some() {
                            model.show_calendar();
                        }
                        model.begin_showcase(sender.input_sender());
                    }
                    for input in held {
                        model.update_message(input, &sender);
                    }
                    *self = Self::Open(model);
                }
                other => opening.hold(other),
            },
        }
    }

    fn update_view(&self, widgets: &mut Self::Widgets, _sender: ComponentSender<Self>) {
        widgets.render(self);
    }
}

impl AppModel {
    /// The model over what the core's constructor returned: the core, or why it could not open.
    pub(super) fn open(booted: Result<BootedApp, String>, input: &relm4::Sender<AppInput>) -> Self {
        let (app, secrets, snapshot, boot_error, allodia_syncable) = match booted {
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
        let requires_setup =
            secrets.is_some() && super::account_settings::no_accounts(app.as_deref());
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
        let (sync_bar, sync_status) = app.as_deref().map_or((None, None), |app| {
            let progress = app.sync_progress();
            (
                sync_line::sync_bar(&progress),
                sync_line::sync_status(&progress, &snapshot.accounts),
            )
        });
        // Pull once: a boot outage's signal fired before this model existed.
        let (connectivity, network_monitor) =
            connectivity::at_launch(app.as_deref(), &snapshot.accounts, input);
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
        Self {
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
            reading_windows: HashMap::new(),
            reading_window_focus: None,
            reading_window_seq: 0,
            reading_generation: 0,
            pending_mail_delete: None,
            composer: None,
            composer_generation: 0,
            composer_windows: Vec::new(),
            composer_window_seq: 0,
            composer_error: None,
            draft_status: composer_drafts::DraftStatuses::new(),
            pending_navigation: None,
            pending_mailto: None,
            pending_share: None,
            draft_check: None,
            draft_check_seq: 0,
            discard_prompt: None,
            notice: None,
            sync_status,
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
        }
    }
}
