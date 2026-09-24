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

/// AI credits, as Allodia's relay last reported them. For display: the relay decides.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CreditBalance {
    /// Credits left.
    pub credits: f64,
    /// When the relay reported it, in seconds since the Unix epoch; a client shows it as "as of".
    pub as_of: i64,
}

/// The Writing style surface.
#[derive(Debug, Clone, Default, PartialEq)]
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
    /// The credits left, when requests go through Allodia's relay and it has reported any.
    pub balance: Option<CreditBalance>,
}

/// One recurring form and roughly how often it is used, for the reveal screen.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HabitRow {
    /// The exact wording.
    pub text: String,
    /// Roughly how often, as a percentage of messages.
    pub share: u8,
    /// Its part of its list, as a percentage of the list's shares together: a bar's length. The
    /// model's shares are rough and need not add up, so a bar is drawn from this.
    pub relative: u8,
    /// How often, in the word the reveal shows beside the bar.
    pub frequency: HabitFrequency,
}

/// How often a greeting or sign-off is used, read from its share of messages.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HabitFrequency {
    /// Half the messages or more.
    Mostly,
    /// A fifth or more.
    Often,
    /// Less than that, or not known.
    Sometimes,
}

impl HabitFrequency {
    /// The word for a share of messages, in percent.
    #[must_use]
    pub const fn of(share: u8) -> Self {
        match share {
            50.. => Self::Mostly,
            20..=49 => Self::Often,
            _ => Self::Sometimes,
        }
    }
}

/// A list of `(wording, share)` as the reveal draws it: each with its part of the list and its
/// frequency word. A list whose shares are all unknown is drawn in equal parts.
#[must_use]
pub fn habit_rows(habits: impl IntoIterator<Item = (String, u8)>) -> Vec<HabitRow> {
    let habits: Vec<(String, u8)> = habits.into_iter().collect();
    let total: u32 = habits.iter().map(|(_, share)| u32::from(*share)).sum();
    let count = u32::try_from(habits.len()).unwrap_or(u32::MAX).max(1);
    habits
        .into_iter()
        .map(|(text, share)| {
            let part = (u32::from(share) * 100 + total / 2)
                .checked_div(total)
                .unwrap_or(100 / count);
            HabitRow {
                text,
                share,
                relative: u8::try_from(part.min(100)).unwrap_or(100),
                frequency: HabitFrequency::of(share),
            }
        })
        .collect()
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
    /// The register in at most five words; empty when a model gave none.
    pub register_headline: String,
    /// Typical length in words; zero when unknown.
    pub typical_words: u32,
    /// Typical paragraphs between greeting and sign-off; zero when unknown.
    pub typical_paragraphs: u32,
    /// Paragraphing and sentence length.
    pub shape: String,
    /// Punctuation habits.
    pub punctuation: String,
    /// Structural habits.
    pub structure: String,
    /// The structural habits in at most five words; empty when a model gave none.
    pub structure_headline: String,
    /// How they decline, chase, apologise and confirm.
    pub moves: String,
    /// The same in at most five words; empty when a model gave none.
    pub moves_headline: String,
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

#[cfg(test)]
#[path = "writing_style_tests.rs"]
mod tests;
