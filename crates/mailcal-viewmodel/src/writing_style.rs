//! The Writing style surface: the learned styles, which account drafts in which, whether AI is
//! available at all, and a learning run in progress (`docs/ai.md`).

use mailcal_jurisdiction::{Class, Mode};

/// One learned style, as the Writing style screen lists it.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct WritingStyleRow {
    /// The style's opaque id.
    pub id: String,
    /// The person's name for it.
    pub name: String,
    /// The account it was learned from; empty when it came from another device.
    pub source_account: String,
    /// The languages it covers (ISO 639-1), most messages first.
    pub languages: Vec<String>,
    /// How many messages it was learned from, in all.
    pub messages: u32,
    /// The oldest and newest message it was learned from, in seconds since the Unix epoch.
    pub oldest: Option<i64>,
    /// See [`oldest`](Self::oldest).
    pub newest: Option<i64>,
    /// When it was learned, in seconds since the Unix epoch; zero when unknown.
    pub learned_at: i64,
}

/// One account's writing style, for the assignment picker.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AccountWritingStyleRow {
    /// The account's id, passed back to the setter.
    pub account_id: String,
    /// The account's address, the row's label.
    pub email: String,
    /// The style it drafts in, or `None`.
    pub style: Option<String>,
}

/// What a learning run is doing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LearningStage {
    /// Reading the account's sent mail on the device. Nothing has left it.
    Reading,
    /// Sending the sample and waiting for the description.
    Learning,
}

/// A learning run in progress.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LearningProgress {
    /// The account being learned from.
    pub account_id: String,
    /// What it is doing.
    pub stage: LearningStage,
    /// Requests answered, while [`LearningStage::Learning`].
    pub done: u32,
    /// Requests the run will make; zero while reading.
    pub total: u32,
}

/// Where AI requests go, as far as a client needs to say.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AiRoute {
    /// Allodia's relay.
    Relay,
    /// The person's own endpoint.
    OwnEndpoint,
}

/// Why the gate would refuse a request right now: the mode in force and the class of the
/// destination. A client words the explanation from these.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GateRefusal {
    /// The mode in force.
    pub mode: Mode,
    /// The destination's class.
    pub class: Class,
}

/// The Writing style surface.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct WritingStyleSnapshot {
    /// Where requests go; `None` when AI is not available in this build or not set up.
    pub route: Option<AiRoute>,
    /// What the gate would say to a request now; `None` when it would pass.
    pub refused: Option<GateRefusal>,
    /// The library, in the person's order.
    pub styles: Vec<WritingStyleRow>,
    /// One row per configured account.
    pub accounts: Vec<AccountWritingStyleRow>,
    /// A learning run in progress.
    pub learning: Option<LearningProgress>,
}

/// One recurring form and roughly how often it is used, for the reveal screen.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct HabitRow {
    /// The exact wording.
    pub text: String,
    /// Roughly how often, as a percentage of messages.
    pub share: u8,
}

/// What was learned about one language, in the words the reveal screen shows.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct LanguageStyleRow {
    /// ISO 639-1 code.
    pub language: String,
    /// How the person opens a message.
    pub greetings: Vec<HabitRow>,
    /// How they close one.
    pub sign_offs: Vec<HabitRow>,
    /// The name they sign with; empty when they do not.
    pub signs_as: String,
    /// Register, and how it shifts.
    pub register: String,
    /// Typical length in words; zero when unknown.
    pub typical_words: u32,
    /// Paragraphing and sentence length.
    pub shape: String,
    /// Punctuation habits.
    pub punctuation: String,
    /// Structural habits.
    pub structure: String,
    /// How they decline, chase, apologise and confirm.
    pub moves: String,
    /// Characteristic phrases.
    pub phrases: Vec<String>,
    /// What they avoid.
    pub avoid: Vec<String>,
}

/// One style in full, for the reveal and edit screen.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct WritingStyleDetail {
    /// The list row.
    pub row: WritingStyleRow,
    /// The person's own notes.
    pub notes: String,
    /// Per language, most messages first.
    pub languages: Vec<LanguageStyleRow>,
}
