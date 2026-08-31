//! Wiring the local MCP server to the running app.
//!
//! Two halves. [`AppBackend`] implements `mailcal-mcp`'s `MailBackend` port over the live app
//! plus the host's composer slot: this is the layer the port exists for, since `create_draft`
//! needs a UI action the core cannot provide and `mailcal-mcp` cannot depend on this crate
//! without a cycle. The rest is the FFI surface a Settings screen drives.
//!
//! # The endpoint is set, not derived
//!
//! A host calls [`MailcalApp::set_mcp_endpoint`] after construction, exactly as it installs a
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

use async_trait::async_trait;
use engine_api::{AccountId, Provider};
use mailcal_app::{App, MailActionError, MessageDetail, MessagePage, MessageRef, SendActionError};
use mailcal_mcp::{AgentDraft, ComposerError, MailBackend, McpConfig};
use mailcal_viewmodel::{AccountRow, FolderRow};

use crate::{MailcalApp, McpSettings, agent_ui::AgentUiSlot};

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

#[uniffi::export]
impl MailcalApp {
    /// The local MCP (AI assistant access) settings a Settings screen renders.
    ///
    /// Carries both the user's decisions **and** whether a server is actually listening, because
    /// a panel showing only the toggle would say "on" while nothing is reachable; another
    /// instance owning the endpoint, or a path that will not bind. `endpoint` is `None` on a
    /// platform whose host set none, which is how a Settings screen knows not to offer the panel
    /// at all.
    pub fn mcp_settings(&self) -> McpSettings {
        let mut settings: McpSettings = self.runtime.block_on(self.app.mcp_settings()).into();
        settings.running = self.mcp.is_running();
        settings.endpoint.clone_from(&self.mcp_endpoint());
        settings
    }

    /// Turns the local MCP server on or off, persists it, and starts or stops it immediately.
    ///
    /// Turning it off does **not** clear the exposed-account list: a user switching the feature
    /// off for an afternoon should not have to re-tick every mailbox, and the list is inert while
    /// nothing is listening.
    pub fn set_mcp_enabled(&self, enabled: bool) {
        self.runtime.block_on(self.app.set_mcp_enabled(enabled));
        self.refresh_mcp();
    }

    /// Exposes or hides one account to assistants, persists it, and re-applies it to a running
    /// server at once: so unticking an account revokes access without a restart.
    pub fn set_mcp_account_exposed(&self, account: String, exposed: bool) {
        self.runtime
            .block_on(self.app.set_mcp_account_exposed(&account, exposed));
        self.refresh_mcp();
    }

    /// Sets whether an assistant may send mail directly (no human review), persists it, and
    /// re-applies it. With it off the send tool is **absent** from the server's listing entirely.
    pub fn set_mcp_allow_direct_send(&self, allow: bool) {
        self.runtime
            .block_on(self.app.set_mcp_allow_direct_send(allow));
        self.refresh_mcp();
    }

    /// Sets whether a direct send is restricted to people the user already emails, persists it,
    /// and re-applies it.
    pub fn set_mcp_require_known_recipient(&self, require: bool) {
        self.runtime
            .block_on(self.app.set_mcp_require_known_recipient(require));
        self.refresh_mcp();
    }

    /// Tells the core where the MCP server should listen: a Unix socket path, or a
    /// `\\.\pipe\…` name on Windows. `None` means this platform has no endpoint and therefore no
    /// server.
    ///
    /// Call once after construction, before the Settings screen is shown. A host that never calls
    /// it can never listen, which is exactly how iOS and Android are excluded: by construction,
    /// not by a check.
    pub fn set_mcp_endpoint(&self, endpoint: Option<String>) {
        *self
            .mcp_endpoint
            .lock()
            .expect("mcp-endpoint mutex poisoned") = endpoint;
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
    /// The configured endpoint, if a host set one.
    pub(crate) fn mcp_endpoint(&self) -> Option<String> {
        self.mcp_endpoint
            .lock()
            .expect("mcp-endpoint mutex poisoned")
            .clone()
    }

    /// Rebuilds the server's configuration from the persisted settings and (re)applies it.
    ///
    /// Called after every settings change and once at boot. The endpoint is handed over **only
    /// when the user has turned the feature on**, so "off" is not a flag the server checks; it
    /// is the absence of anywhere to listen.
    pub(crate) fn refresh_mcp(&self) {
        let enabled = self.app.mcp_enabled();
        let config = McpConfig {
            endpoint: if enabled { self.mcp_endpoint() } else { None },
            accounts: self.app.mcp_exposed_accounts(),
            allow_direct_send: self.app.mcp_allow_direct_send(),
            require_known_recipient: self.app.mcp_require_known_recipient(),
        };
        self.mcp.apply(&config);
    }
}
