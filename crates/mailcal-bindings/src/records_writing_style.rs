//! The Writing style surface's FFI records, and the outcomes of learning and drafting
//! (`docs/ai.md`). The conversions from the core's types are in `convert_writing_style`.

use mailcal_ai::{Class, Mode};

/// How far an external dispatch may reach.
#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum JurisdictionMode {
    /// Anywhere, including somewhere nobody has classified.
    All,
    /// Processed in the EU, whoever operates it.
    EuHosted,
    /// Processed in the EU and operated from the EU.
    EuNative,
}

impl From<Mode> for JurisdictionMode {
    fn from(mode: Mode) -> Self {
        match mode {
            Mode::All => Self::All,
            Mode::EuHosted => Self::EuHosted,
            Mode::EuNative => Self::EuNative,
        }
    }
}

/// Where a destination processes data, and from where it is operated.
#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum JurisdictionClass {
    /// In the EU, operated from the EU.
    EuNative,
    /// In the EU, operated from outside it.
    EuHosted,
    /// Outside the EU.
    NonEu,
    /// Nobody has said.
    Unknown,
}

impl From<Class> for JurisdictionClass {
    fn from(class: Class) -> Self {
        match class {
            Class::EuNative => Self::EuNative,
            Class::EuHosted => Self::EuHosted,
            Class::NonEu => Self::NonEu,
            Class::Unknown => Self::Unknown,
        }
    }
}

impl From<JurisdictionClass> for Class {
    fn from(class: JurisdictionClass) -> Self {
        match class {
            JurisdictionClass::EuNative => Self::EuNative,
            JurisdictionClass::EuHosted => Self::EuHosted,
            JurisdictionClass::NonEu => Self::NonEu,
            JurisdictionClass::Unknown => Self::Unknown,
        }
    }
}

/// Where AI requests go.
#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum AiRoute {
    /// Allodia's relay.
    Relay,
    /// The person's own endpoint.
    OwnEndpoint,
}

/// Why the gate would refuse a request now.
#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Record)]
pub struct GateRefusal {
    /// The mode in force.
    pub mode: JurisdictionMode,
    /// The destination's class.
    pub class: JurisdictionClass,
}

/// One learned style, as the Writing style screen lists it.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct WritingStyleRow {
    /// The style's opaque id.
    pub id: String,
    /// The person's name for it.
    pub name: String,
    /// The account it was learned from; empty when it came from another device.
    pub source_account: String,
    /// The languages it covers (ISO 639-1), most messages first.
    pub languages: Vec<String>,
    /// How many messages it was learned from.
    pub messages: u32,
    /// The oldest message it was learned from, in seconds since the Unix epoch.
    pub oldest: Option<i64>,
    /// The newest message it was learned from, in seconds since the Unix epoch.
    pub newest: Option<i64>,
    /// When it was learned, in seconds since the Unix epoch; zero when unknown.
    pub learned_at: i64,
}

/// One account's writing style, for the assignment picker.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct AccountWritingStyleRow {
    /// The account's id.
    pub account_id: String,
    /// The account's address.
    pub email: String,
    /// The style it drafts in, or `None`.
    pub style: Option<String>,
}

/// What a learning run is doing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum LearningStage {
    /// Reading sent mail on the device; nothing has left it.
    Reading,
    /// Sending the sample and waiting for the description.
    Learning,
}

/// A learning run in progress.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct LearningProgress {
    /// The account being learned from.
    pub account_id: String,
    /// What it is doing.
    pub stage: LearningStage,
    /// Requests answered.
    pub done: u32,
    /// Requests the run will make; zero while reading.
    pub total: u32,
}

/// AI credits, as Allodia's relay last reported them. For display: the relay decides.
#[derive(Debug, Clone, Copy, PartialEq, uniffi::Record)]
pub struct CreditBalance {
    /// Credits left.
    pub credits: f64,
    /// When the relay reported it, in seconds since the Unix epoch; shown as "as of".
    pub as_of: i64,
}

/// The Writing style surface (pulled after a `Surface::WritingStyle` signal).
#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct WritingStyleSnapshot {
    /// Where requests go; `None` when AI is not available or not set up. A client shows the
    /// Writing style category only when this is `Some` (`docs/settings.md`).
    pub route: Option<AiRoute>,
    /// What the gate would say to a request now; `None` when it would pass.
    pub refused: Option<GateRefusal>,
    /// The library, in the person's order.
    pub styles: Vec<WritingStyleRow>,
    /// One row per configured account.
    pub accounts: Vec<AccountWritingStyleRow>,
    /// A learning run in progress.
    pub learning: Option<LearningProgress>,
    /// The credits left, when requests go through Allodia's relay and it has reported any.
    pub balance: Option<CreditBalance>,
}

/// One recurring form and roughly how often it is used.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct HabitRow {
    /// The exact wording.
    pub text: String,
    /// Roughly how often, as a percentage of messages.
    pub share: u8,
}

/// What was learned about one language, for the reveal screen.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
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

/// One style in full.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct WritingStyleDetail {
    /// The list row.
    pub row: WritingStyleRow,
    /// The person's own notes.
    pub notes: String,
    /// Per language, most messages first.
    pub languages: Vec<LanguageStyleRow>,
}

/// One language's share of a corpus.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct CorpusLanguage {
    /// ISO 639-1 code.
    pub language: String,
    /// Usable messages in it.
    pub usable: u32,
    /// How many the sample holds.
    pub sampled: u32,
    /// Roughly how many tokens the sample is.
    pub tokens: u32,
}

/// What learning would read and send: the consent screen's facts.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct CorpusReport {
    /// Sent messages read.
    pub found: u32,
    /// Messages with enough of the person's own words to learn from.
    pub usable: u32,
    /// Usable messages in a language no style is learned for.
    pub undetected: u32,
    /// Per language, most messages first.
    pub languages: Vec<CorpusLanguage>,
    /// The oldest usable message, in seconds since the Unix epoch.
    pub oldest: Option<i64>,
    /// The newest usable message.
    pub newest: Option<i64>,
    /// The oldest sent message on this device, whatever the range: a client offers to fetch
    /// older sent mail when the range reaches past it.
    pub horizon: Option<i64>,
}

/// What a request cost, when it went through the relay.
#[derive(Debug, Clone, Copy, PartialEq, uniffi::Record)]
pub struct AiCharge {
    /// Credits it took.
    pub credits_charged: f64,
    /// Credits left.
    pub balance_credits: f64,
}

/// A style learned and stored.
#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct LearnReport {
    /// The new style's id.
    pub style_id: String,
    /// The languages it covers.
    pub languages: Vec<String>,
    /// How many messages it was learned from.
    pub messages: u32,
    /// What it cost; `None` from an own endpoint.
    pub charge: Option<AiCharge>,
}

/// A drafted reply, for the open composer.
#[derive(Clone, PartialEq, uniffi::Record)]
pub struct DraftReply {
    /// The draft's id: the client hands it to `setComposerDraftText` with the text, and the
    /// composer carries it back on submit, so a reply sent from it is never learned from.
    pub draft_id: String,
    /// The body, to go above the quote.
    pub text: String,
    /// The bracketed gaps in it; a client says "check the parts in brackets" when there are any.
    pub gaps: Vec<String>,
    /// The language it was written in.
    pub language: String,
    /// What it cost; `None` from an own endpoint.
    pub charge: Option<AiCharge>,
}

impl std::fmt::Debug for DraftReply {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DraftReply")
            .field("text_len", &self.text.len())
            .field("gaps", &self.gaps.len())
            .finish_non_exhaustive()
    }
}

/// Why learning or drafting produced nothing. A client words each from the variant; none carries
/// a server's sentence.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Error, thiserror::Error)]
pub enum WritingStyleFailure {
    /// AI is not available in this build, or not set up.
    #[error("unavailable")]
    Unavailable,
    /// A learning run is already going.
    #[error("busy")]
    Busy,
    /// The account has no Sent folder on this device.
    #[error("no sent folder")]
    NoSentFolder,
    /// Nothing in the range says enough to learn from.
    #[error("nothing to learn from")]
    NothingToLearn,
    /// No style to write in.
    #[error("no style")]
    NoStyle,
    /// The message is not on this device.
    #[error("not found")]
    NotFound,
    /// The gate refused; nothing left the device.
    #[error("refused")]
    Refused {
        /// The mode in force.
        mode: JurisdictionMode,
        /// The destination's class.
        class: JurisdictionClass,
    },
    /// No credits left.
    #[error("out of credits")]
    OutOfCredits,
    /// The plan does not include AI.
    #[error("not entitled")]
    NotEntitled,
    /// The endpoint refused the key or the sign-in.
    #[error("unauthorized")]
    Unauthorized,
    /// The endpoint asked for fewer requests.
    #[error("rate limited")]
    RateLimited,
    /// The endpoint could not be reached or did not answer in time.
    #[error("unreachable")]
    Unreachable,
    /// The endpoint answered with another status.
    #[error("status {code}")]
    Status {
        /// The HTTP status.
        code: u16,
    },
    /// The answer could not be read.
    #[error("malformed")]
    Malformed,
    /// The person stopped it.
    #[error("cancelled")]
    Cancelled,
}
