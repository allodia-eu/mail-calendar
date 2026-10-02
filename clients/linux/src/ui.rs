//! Relm4 model and GTK/libadwaita three-pane mail shell.

use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
};

use mailcal_bindings::{Intent, MailboxListSnapshot, MailcalApp, ReplyPrompt, ViewMode};

use crate::{
    l10n,
    preferences::{self, HostPreferences},
    secrets::SecretStore,
};

mod allodia;
mod allodia_subscription;
mod allodia_subscription_facts;
mod allodia_sync;
mod avatar;
mod calendar;
mod calendar_actions;
mod component;
mod composer;
mod composer_attach;
mod composer_discard;
mod composer_draft;
mod composer_drafts;
mod composer_fields;
mod composer_header;
mod composer_host;
mod composer_model;
mod composer_notice;
mod composer_open;
mod composer_quote;
mod composer_share;
mod composer_signature;
mod connectivity;
mod contacts;
mod contacts_actions;
pub(crate) mod destinations;
mod detached;
mod dns;
mod editor_paste;
mod folder_actions;
mod folder_dialogs;
mod folder_drag;
mod folder_menu;
mod folder_names;
mod folder_pane;
mod folder_pane_edit;
mod folder_pane_rows;
mod folder_picker;
mod google;
mod host_tasks;
mod icons;
mod imap_actions;
mod imap_signin;
mod input;
mod invitation;
mod invitation_actions;
mod jmap;
mod jmap_actions;
mod linked_text;
mod mail_actions;
mod mail_actions_menu;
mod mail_toolbar;
mod mailbox;
mod mailbox_display;
mod mailbox_empty;
mod mailbox_progressive;
mod mailbox_reconcile;
mod mcp;
mod message_move;
mod microsoft;
mod modal;
mod model;
mod notifications;
mod oauth_actions;
mod oauth_loopback;
mod operations;
mod outbox;
mod reader;
mod reading;
mod reading_windows;
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
mod setup_pane;
mod setup_server_field;
#[cfg(test)]
mod setup_server_field_tests;
mod setup_server_row;
#[cfg(test)]
mod setup_signin_tests;
mod setup_state;
#[cfg(test)]
mod setup_widget_tests;
mod setup_widgets;
mod shell;
mod shell_sidebar;
#[cfg(any(debug_assertions, feature = "dev-harness"))]
mod showcase_hooks;
mod signature_image;
mod sync_line;
mod time_zone;
mod timestamps;
mod unfiled_copy;
mod update;
mod update_pull;
mod update_reauth;
mod update_signin;
mod web_security;
mod webview;
mod welcome;

use calendar::CalendarModel;
use composer_draft::PendingNavigation;
use composer_drafts::DraftStatuses;
use composer_model::ComposeContext;
#[cfg(any(debug_assertions, feature = "dev-harness"))]
use composer_model::ComposeKind;
use composer_notice::ComposerNotice;
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
use reader::ComposerHost;
use reading_windows::DetachedDraft;
use search::SearchState;
use selection::Selection;
use setup_state::SetupState;
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
    /// The messages open in windows of their own, by the id the core holds each body under
    /// (`docs/reading-window.md`). The header each was opened on travels with it, so a window
    /// draws a subject and a sender from its first frame exactly as the pane does.
    reading_windows: HashMap<String, ReadingState>,
    /// The window an open asked to be brought forward, and the counter that says the ask is a new
    /// one. A render sees every model change, and presenting on each would keep pulling a window
    /// in front of whatever the person moved to since.
    reading_window_focus: Option<String>,
    reading_window_seq: u64,
    /// Which reading snapshot the pane has drawn. Bumped on every `Surface::Reading` pull, so the
    /// invitation card is rebuilt when the core publishes a new one and left alone on every other
    /// render; a rebuild mid-render would take a half-typed note to the organiser away.
    reading_generation: u64,
    pending_mail_delete: Option<DeleteTarget>,
    composer: Option<ComposeContext>,
    composer_generation: u64,
    /// The drafts being written in windows of their own, and the counter each is named by. Host
    /// state: a message exists to the core only once it is sent.
    composer_windows: Vec<DetachedDraft>,
    composer_window_seq: u64,
    /// What the composer's error line is showing, or `None` when it shows nothing.
    composer_error: Option<ComposerNotice>,
    /// How each open composition's most recent save ended, by composition id.
    ///
    /// A `Surface::DraftStatus` signal says that *some* composition's save moved, not which, so
    /// every open composer reads its own back (`docs/drafts.md`). Entries are dropped as each
    /// composer closes, so the map holds what is on screen and nothing else.
    draft_status: DraftStatuses,
    /// The message or external draft waiting for the open composer to be left.
    pending_navigation: Option<PendingNavigation>,
    /// A mail link received before an account exists. Account setup completing opens it.
    pending_mailto: Option<mailcal_bindings::MailtoPrefill>,
    /// A share received before an account exists, held on the same terms as a mail link.
    pending_share: Option<mailcal_bindings::SharePrefill>,
    /// The navigation the open composer must be left for, and the counter it is drawn from. Its
    /// own sequence, not the composer's: two navigations away from one draft must each get an
    /// answer, and reusing the composer's generation would make the pane treat the second as
    /// already answered.
    draft_check: Option<u64>,
    draft_check_seq: u64,
    /// The composer the "Discard draft?" question is on screen for, if it is.
    discard_prompt: Option<ComposerHost>,
    notice: Option<String>,
    /// The mail list's bottom-bar caption: an account a server has asked to wait, or a
    /// background sync downloading mail. `None` whenever there is nothing to say, which is
    /// almost always.
    sync_status: Option<sync_line::SyncStatus>,
    /// The separate foreground-download row. It wins the shared bottom strip while active.
    sync_bar: Option<sync_line::SyncBar>,
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

    /// What the message list calls itself: the folder on screen, or; while a search is running
    /// ; the results, as macOS and Windows label the same list.
    fn list_title(&self) -> String {
        if self.search.is_active() {
            return l10n::search_results().to_owned();
        }
        folder_names::header_title(&self.snapshot)
    }

    fn subtitle(&self) -> String {
        if let Some(error) = &self.boot_error {
            return l10n::status_connect_failed(error);
        }
        // The Outbox builds no mail list, so `total` is 0 there: counting conversations would
        // say "0 conversations" over a list of messages the user can see. The same sentence the
        // pane badge speaks, so the two agree (`docs/folder-pane.md`, rule 13).
        if self.snapshot.showing_outbox {
            let waiting = i64::try_from(self.snapshot.outbox.len()).unwrap_or(i64::MAX);
            return l10n::a11y_outbox_count(waiting);
        }
        let total = i64::try_from(self.snapshot.total).unwrap_or(i64::MAX);
        match self.snapshot.mode {
            ViewMode::Flat => l10n::mailbox_count_messages(total),
            ViewMode::Threaded => l10n::mailbox_count_conversations(total),
        }
    }
}
