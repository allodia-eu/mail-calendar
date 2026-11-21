//! Wiring the local MCP server to the running app.
//!
//! Two halves. [`AppBackend`] implements `mailcal-mcp`'s `MailBackend` port over the live app
//! plus the host's composer slot: this is the layer the port exists for, since `create_draft`
//! needs a UI action the core cannot provide and `mailcal-mcp` cannot depend on this crate
//! without a cycle. The rest is the FFI surface a Settings screen drives.
//!
//! # The endpoint is set, not derived
//!
//! A host calls [`MailcalApp::set_mcp_host`] after construction, exactly as it installs a
//! credential store. Two things follow, and both are deliberate:
//!
//! * **A platform with no endpoint has no server**, with no `#[cfg]` and no runtime platform check
//!   anywhere. iOS and Android simply never call it, which is right, because those OSes suspend the
//!   app and a server that is asleep when a client connects is worse than none.
//! * **The path is derived once, by the layer that knows the answer.** A sandboxed build's data
//!   directory differs from a Developer-ID one's; deriving it in the core *and* in the relay binary
//!   would give two answers that agree until the day they do not. The Settings screen puts the very
//!   same string into the config snippet it offers to copy.

use std::sync::Arc;

#[path = "app_advanced.rs"]
mod advanced;

use async_trait::async_trait;
use engine_api::{AccountId, Provider};
use mailcal_app::{App, MailActionError, MessageDetail, MessagePage, MessageRef, SendActionError};
use mailcal_mcp::{AgentDraft, ComposerError, ListenerStatus, MailBackend, McpConfig, McpServer};
use mailcal_viewmodel::{AccountRow, FolderRow};

use crate::{
    MailcalApp, McpAccountRow, McpHost, McpIntent, McpPanel, McpStatus, agent_ui::AgentUiSlot,
};

/// The app type every account shares (providers boxed behind the trait).
type SharedApp = Arc<App<Box<dyn Provider>>>;

/// The MCP port, over the running app and the host's composer.
pub(crate) struct AppBackend {
    app: SharedApp,
    agent_ui: AgentUiSlot,
}

impl core::fmt::Debug for AppBackend {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("AppBackend").finish_non_exhaustive()
    }
}

impl AppBackend {
    pub(crate) fn new(app: SharedApp, agent_ui: AgentUiSlot) -> Self {
        Self { app, agent_ui }
    }

    /// Resolves `(account, key)` into the typed reference the core's actions take.
    ///
    /// The pair travels together all the way from the MCP boundary, mirroring
    /// `MessageRef::from_parts`: a provider key is unique only *within* an account, so a bare key
    /// could route an action into the wrong mailbox. A malformed half is
    /// [`MailActionError::UnknownMessage`] rather than a panic.
    fn message_ref(account: &str, key: &str) -> Result<MessageRef, MailActionError> {
        MessageRef::from_parts(account, key.to_owned()).ok_or(MailActionError::UnknownMessage)
    }

    /// Parses an account id, or `None` if it is malformed.
    fn account_id(account: &str) -> Option<AccountId> {
        AccountId::try_from(account).ok()
    }
}

#[async_trait]
impl MailBackend for AppBackend {
    async fn accounts(&self) -> Vec<AccountRow> {
        self.app.query_accounts().await
    }

    async fn folders(&self, account: &str) -> Vec<FolderRow> {
        match Self::account_id(account) {
            Some(id) => self.app.query_folders(&id).await,
            None => Vec::new(),
        }
    }

    async fn folder_page(
        &self,
        account: &str,
        folder: Option<&str>,
        unread_only: bool,
        offset: usize,
        limit: usize,
    ) -> MessagePage {
        let Some(id) = Self::account_id(account) else {
            return MessagePage::default();
        };
        self.app
            .query_folder_page(&id, folder, unread_only, offset, limit)
            .await
    }

    async fn search(
        &self,
        query: &str,
        account: Option<&str>,
        folder: Option<&str>,
        offset: usize,
        limit: usize,
    ) -> MessagePage {
        let id = account.and_then(Self::account_id);
        self.app
            .query_search(query, id.as_ref(), folder, offset, limit)
            .await
    }

    async fn message(&self, account: &str, key: &str) -> Option<MessageDetail> {
        let reference = MessageRef::from_parts(account, key.to_owned())?;
        self.app.query_message(&reference).await
    }

    async fn mark_read(&self, account: &str, key: &str, read: bool) -> Result<(), MailActionError> {
        self.app
            .act_mark_read(&Self::message_ref(account, key)?, read)
            .await
    }

    async fn set_flagged(
        &self,
        account: &str,
        key: &str,
        flagged: bool,
    ) -> Result<(), MailActionError> {
        self.app
            .act_set_flagged(&Self::message_ref(account, key)?, flagged)
            .await
    }

    async fn archive(&self, account: &str, key: &str) -> Result<(), MailActionError> {
        self.app
            .act_archive(&Self::message_ref(account, key)?)
            .await
    }

    async fn trash(&self, account: &str, key: &str) -> Result<(), MailActionError> {
        self.app.act_trash(&Self::message_ref(account, key)?).await
    }

    async fn spam(&self, account: &str, key: &str) -> Result<(), MailActionError> {
        self.app.act_spam(&Self::message_ref(account, key)?).await
    }

    async fn send_plain(
        &self,
        account: Option<&str>,
        to: &[String],
        cc: &[String],
        bcc: &[String],
        subject: String,
        body: String,
    ) -> Result<(), SendActionError> {
        let id = match account {
            Some(account) => {
                Some(Self::account_id(account).ok_or(SendActionError::UnknownAccount)?)
            }
            None => None,
        };
        self.app
            .act_send_plain(id.as_ref(), to, cc, bcc, subject, body)
            .await
    }

    async fn known_recipients(&self, query: &str) -> Vec<String> {
        // The same index the composer's autosuggest rides: the engine mines it from Sent mail,
        // so "someone you have written to" works on an account with no address book at all,
        // which is most accounts.
        self.app
            .recipient_suggestions(query)
            .await
            .into_iter()
            .map(|found| found.email)
            .collect()
    }

    fn open_composer(&self, draft: AgentDraft) -> Result<(), ComposerError> {
        crate::agent_ui::open_composer(&self.agent_ui, draft)
    }
}

/// The server over `app` and the host's composer slot, stopped until a host sets an endpoint.
/// Its status moving raises `Surface::Settings`, so a panel on screen follows a bind or a
/// refusal.
pub(crate) fn server(
    app: &SharedApp,
    agent_ui: &AgentUiSlot,
    handle: &tokio::runtime::Handle,
) -> Arc<McpServer> {
    let server = McpServer::new(
        Arc::new(AppBackend::new(Arc::clone(app), Arc::clone(agent_ui))),
        handle.clone(),
    );
    server.on_status_change({
        let app = Arc::clone(app);
        move || app.settings_changed()
    });
    Arc::new(server)
}

#[uniffi::export]
impl MailcalApp {
    /// Tells the core where the MCP server should listen (a Unix socket path, or a `\\.\pipe\…`
    /// name on Windows) and how an assistant starts the relay that reaches it. `None` means this
    /// platform has no endpoint and therefore no server.
    ///
    /// Call once after construction, before the Settings screen is shown. A host that never calls
    /// it can never listen, which is exactly how iOS and Android are excluded: by construction,
    /// not by a check.
    pub fn set_mcp_host(&self, host: Option<McpHost>) {
        *self.mcp_host.lock().expect("mcp-host mutex poisoned") = host;
        self.refresh_mcp();
    }

    /// Installs the host's composer port, so an assistant's `create_draft` can open a prefilled,
    /// **unsent** draft in the app's own composer. Optional: a client that has not wired one
    /// simply reports that it has no composer.
    pub fn set_agent_host_ui(&self, ui: Box<dyn crate::AgentHostUi>) {
        *self.agent_ui.lock().expect("agent-ui mutex poisoned") = Some(Arc::from(ui));
    }
}

impl MailcalApp {
    /// The endpoint and relay, if a host set them.
    pub(crate) fn mcp_host(&self) -> Option<McpHost> {
        self.mcp_host
            .lock()
            .expect("mcp-host mutex poisoned")
            .clone()
    }

    /// Settings → Advanced's assistant access, or `None` where the host set no endpoint.
    pub(crate) fn mcp_panel(&self) -> Option<McpPanel> {
        let host = self.mcp_host()?;
        let settings = self.app.mcp_settings();
        let status = if settings.enabled {
            match self.mcp.status() {
                ListenerStatus::Listening => McpStatus::Listening,
                ListenerStatus::AnotherInstance => McpStatus::AnotherInstance,
                // Asked to listen, and the listener not yet started: the moment between
                // turning it on and the endpoint being bound.
                ListenerStatus::Starting | ListenerStatus::Stopped => McpStatus::Starting,
                ListenerStatus::Unavailable => McpStatus::Unavailable,
            }
        } else {
            McpStatus::Off
        };
        Some(McpPanel {
            enabled: settings.enabled,
            status,
            nothing_exposed: !settings.accounts.iter().any(|account| account.exposed),
            accounts: settings
                .accounts
                .into_iter()
                .map(|row| McpAccountRow {
                    account_id: row.account_id,
                    email: row.email,
                    exposed: row.exposed,
                })
                .collect(),
            allow_direct_send: settings.allow_direct_send,
            require_known_recipient: settings.require_known_recipient,
            known_recipient_editable: settings.allow_direct_send,
            snippet: mailcal_mcp::configuration_snippet(
                &host.relay_command,
                &host.relay_args,
                &host.endpoint,
            ),
        })
    }

    /// Handles an `Intent::Mcp`: the decision is held and saved behind at once, and the server
    /// takes it up, on the host's thread, before `dispatch` returns. A connection already open
    /// sees it too, so unticking an account revokes access without a restart.
    pub(crate) fn dispatch_mcp(&self, intent: McpIntent) {
        let app = &self.app;
        match intent {
            McpIntent::SetEnabled { enabled } => {
                self.runtime.block_on(|| app.set_mcp_enabled(enabled));
            }
            McpIntent::SetAccountExposed { account, exposed } => {
                self.runtime
                    .block_on(|| app.set_mcp_account_exposed(&account, exposed));
            }
            McpIntent::SetDirectSend { allow } => {
                self.runtime
                    .block_on(|| app.set_mcp_allow_direct_send(allow));
            }
            McpIntent::SetKnownRecipient { require } => {
                self.runtime
                    .block_on(|| app.set_mcp_require_known_recipient(require));
            }
        }
        self.refresh_mcp();
    }

    /// Rebuilds the server's configuration from the persisted settings and (re)applies it.
    ///
    /// Called after every settings change and once at boot. The endpoint is handed over **only
    /// when the user has turned the feature on**, so "off" is not a flag the server checks; it
    /// is the absence of anywhere to listen.
    pub(crate) fn refresh_mcp(&self) {
        let enabled = self.app.mcp_enabled();
        let config = McpConfig {
            endpoint: if enabled {
                self.mcp_host().map(|host| host.endpoint)
            } else {
                None
            },
            accounts: self.app.mcp_exposed_accounts(),
            allow_direct_send: self.app.mcp_allow_direct_send(),
            require_known_recipient: self.app.mcp_require_known_recipient(),
        };
        self.mcp.apply(&config);
    }
}
