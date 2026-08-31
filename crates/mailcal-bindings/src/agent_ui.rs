//! The host's agent-facing UI port: opening the client's own composer on an assistant's behalf.
//!
//! Shaped exactly like [`MicrosoftCredentialStore`](crate::MicrosoftCredentialStore); a
//! `callback_interface` the host implements and installs after construction, held in a shared
//! slot. Unset means the capability does not exist, which is precisely what a client that has
//! not wired a composer should report; **no `#[cfg]` is needed anywhere** for a platform to lack
//! it (Linux, today).
//!
//! # Why an MCP draft opens a composer instead of sending
//!
//! Because a human then sees the recipients and the body before anything leaves the machine.
//! That is a *visibility* property, not a safety guarantee: a user who asked for "reply to Bob"
//! will press Send without reading, and `docs/mcp.md` says so plainly rather than letting this
//! carry weight the known-recipient guard is carrying. What it does buy is real: the message an
//! assistant composed is shown in the user's own app, in their own composer, where it can be
//! edited or abandoned.

use std::sync::{Arc, Mutex};

use mailcal_mcp::ComposerError;

/// A draft for the host to open, unsent, in its composer.
///
/// The recipient fields are comma-joined rather than lists, matching
/// `Intent::SubmitRichMail`; one representation of "a recipient field" across the FFI, not two.
#[derive(Debug, Clone, uniffi::Record)]
pub struct AgentDraft {
    /// The account to send from, or `None` to let the composer choose as it always does.
    pub account: Option<String>,
    /// The `To` field, comma-joined.
    pub to: String,
    /// The `Cc` field, comma-joined.
    pub cc: String,
    /// The `Bcc` field, comma-joined.
    pub bcc: String,
    /// The subject.
    pub subject: String,
    /// The body, as plain text.
    pub body_text: String,
    /// The account of the message being replied to, when this is a reply.
    pub reply_to_account: Option<String>,
    /// The provider key of the message being replied to, when this is a reply.
    pub reply_to_key: Option<String>,
}

impl From<mailcal_mcp::AgentDraft> for AgentDraft {
    fn from(draft: mailcal_mcp::AgentDraft) -> Self {
        Self {
            account: draft.account,
            to: draft.to,
            cc: draft.cc,
            bcc: draft.bcc,
            subject: draft.subject,
            body_text: draft.body_text,
            reply_to_account: draft.reply_to_account,
            reply_to_key: draft.reply_to_key,
        }
    }
}

/// The host's agent-facing UI actions.
///
/// An MCP `create_draft` does **not** send: it opens the app's own composer, prefilled and
/// unsent, so a human sees the recipients and the body and presses Send.
///
/// Implementations **must not block**. This is called from the MCP server's connection task; a
/// host that waits for the window to appear would stall that connection and, on a single-threaded
/// UI framework, risk deadlocking against its own main thread. Hop to the UI thread and return.
#[uniffi::export(callback_interface)]
pub trait AgentHostUi: Send + Sync {
    /// Opens `draft` in the host's composer and brings the window forward.
    fn open_composer(&self, draft: AgentDraft);
}

/// A shared slot for the host [`AgentHostUi`], filled after construction via
/// [`MailcalApp::set_agent_host_ui`](crate::MailcalApp::set_agent_host_ui).
pub(crate) type AgentUiSlot = Arc<Mutex<Option<Arc<dyn AgentHostUi>>>>;

/// Opens `draft` through whatever host UI is installed.
///
/// # Errors
///
/// [`ComposerError::NoHostComposer`] when none is: a headless build, or a client that has not
/// wired the port. The MCP tool turns that into a sentence the assistant can relay.
pub(crate) fn open_composer(
    slot: &AgentUiSlot,
    draft: mailcal_mcp::AgentDraft,
) -> Result<(), ComposerError> {
    let host = slot
        .lock()
        .expect("agent-ui mutex poisoned")
        .as_ref()
        .map(Arc::clone);
    let Some(host) = host else {
        return Err(ComposerError::NoHostComposer);
    };
    // Nothing about the draft is logged; it carries recipients and a body, both content.
    log::info!("mcp: opening a prefilled draft in the host composer");
    host.open_composer(draft.into());
    Ok(())
}
