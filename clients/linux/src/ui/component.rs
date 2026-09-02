//! How the Relm4 component runs [`AppModel`]: boot, the input queue, and the render pass.
//!
//! Split from [`super`], which had reached the size limit, along the seam that was already
//! there: that module says what the model *is* and which modules exist, this one says how it
//! comes up and what it does with a message. A child module reaches its parent's private
//! items, so nothing here needed widening to move.

use std::collections::{HashMap, HashSet};

use mailcal_bindings::Appearance;
use relm4::{ComponentParts, ComponentSender, SimpleComponent};

use super::{
    AppInput, AppModel, PrimaryView, SetupState, allodia_sync, calendar::CalendarModel,
    calendar_actions, connectivity, contacts::ContactsModel, host_tasks::HostTasks, mcp, model,
    model::ReadingState, preferences, runtime_timers, search::SearchState, settings,
    setup_onboarding, shell::AppWidgets, time_zone, welcome,
};
use crate::{appearance, boot, crash, logger, observer::SurfaceObserver};

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
