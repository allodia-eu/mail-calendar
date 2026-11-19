//! The person's sent mail as learning and drafting both read it (`docs/ai.md` rules 14 and 25):
//! which messages are drafted replies, and a message's body as the text a model is handed.

use engine_api::{AccountId, Message, Provider};
use mailcal_account::drafted_reply_keyword;

use crate::{App, query::to_plain};

impl<P: Provider> App<P> {
    /// Whether `message`, as the store lists it, is a drafted reply: it carries the keyword, which
    /// reaches every device where the provider keeps one, or this device's log names it. Never
    /// learned from and never shown to a draft as the person's own words (rule 25).
    #[must_use]
    pub fn is_drafted_message(&self, message: &Message) -> bool {
        message.has_keyword(&drafted_reply_keyword())
            || message
                .envelope
                .message_id
                .iter()
                .any(|id| self.is_drafted_reply(id))
    }

    /// A message's body as text, or `None` when it cannot be read: its own text part, or the text
    /// of its sanitised HTML, never the HTML. Read through the engine's body read, which answers
    /// from the local cache for what the body warm has reached and fetches the rest once, as
    /// opening it would.
    pub(super) async fn message_text(
        &self,
        account: &AccountId,
        message: &Message,
    ) -> Option<String> {
        let handle = self.account_handle(account).await?;
        let provider = handle.providers.first()?;
        let body = self
            .engine
            .message_body(provider, account, message)
            .await
            .ok()?;
        match body.plain() {
            Some(plain) if !plain.trim().is_empty() => Some(plain.to_owned()),
            _ => body
                .html()
                .map(|html| to_plain(&engine_html::sanitize(html).into_html())),
        }
    }
}
