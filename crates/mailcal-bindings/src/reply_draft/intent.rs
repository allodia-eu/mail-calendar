//! The changes a composer makes to Draft a reply, as `Intent::Ai { intent: AiIntent::ReplyDraft }`.
//! Each applies before `dispatch` returns; a draft itself is made off the host's thread.

/// A change to one composition's Draft a reply.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Enum)]
pub enum ReplyDraftIntent {
    /// Draft, or Return in the instruction's line. Dropped unless the record's `draft` control is
    /// enabled, and, while `ask_before_replacing` holds, unless `replace` says the person was
    /// asked and chose to replace their text.
    Draft {
        composition: String,
        /// The composer's From, as `submit_composition` takes it; `None` for the account the
        /// composer opened on.
        from: Option<String>,
        /// The line as typed; the core trims it, and an empty one asks for the likeliest reply.
        instruction: String,
        /// The catalog locale the app is shown in, which the summary and the checklist are
        /// written in.
        language: String,
        replace: bool,
    },
    /// Stop: abandons the draft being made, the request in flight included, or drops one that
    /// waits for the replace question.
    Stop { composition: String },
    /// The answer to `replace_arrived`: `replace` puts the draft in, otherwise the person's text
    /// stays and the draft is dropped.
    AnswerArrived { composition: String, replace: bool },
    /// The person's tick on checklist item `index`. A gap follows the reply and takes none.
    Tick {
        composition: String,
        index: u32,
        done: bool,
    },
    /// Send asked about open items and the person answered, Send anyway or Keep editing alike:
    /// it is not asked again in this composition. A client sends this before it submits.
    SendQuestionAnswered { composition: String },
}
