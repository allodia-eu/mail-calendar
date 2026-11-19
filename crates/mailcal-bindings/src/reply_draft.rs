//! Draft a reply over the FFI (`docs/ai.md` rules 17 to 25): the record a composer draws, the
//! intents that change it, and the editor's report handed to the core.

mod app;
mod intent;
mod records;

pub use intent::ReplyDraftIntent;
pub use records::{
    ComposerReplyDraft, ReplyDraftHold, ReplyDraftItem, ReplyDraftScript, ReplyDraftSendQuestion,
    ReplyDraftState, ReplyDraftSuggestion, ReplyDraftTaskKind,
};

#[cfg(test)]
#[path = "reply_draft/tests.rs"]
mod tests;
