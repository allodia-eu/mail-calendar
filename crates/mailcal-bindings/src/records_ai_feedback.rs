//! The records feedback on a draft is given in, and the token counts a draft carries
//! (`docs/ai.md`, "Feedback"). The training mode rates with the same ones.

use mailcal_ai::{Rating, Reason, Verdict, wire::Usage};

/// Whether a draft was good.
#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum DraftVerdict {
    /// Thumbs up.
    Up,
    /// Thumbs down.
    Down,
}

/// What was wrong with a draft.
#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum DraftRatingReason {
    /// The tone was wrong.
    WrongTone,
    /// Too long or too short.
    WrongLength,
    /// It made things up.
    MadeThingsUp,
    /// It missed the point.
    MissedThePoint,
    /// It was in the wrong language.
    WrongLanguage,
    /// Something else, which the comment says.
    SomethingElse,
}

/// A verdict on a draft, what was wrong and anything the person added. The core keeps each reason
/// once and cuts the comment to 1,000 characters.
#[derive(Clone, PartialEq, Eq, uniffi::Record)]
pub struct DraftRating {
    /// Good or not.
    pub verdict: DraftVerdict,
    /// What was wrong; empty for a thumbs up.
    pub reasons: Vec<DraftRatingReason>,
    /// Anything the person added; empty when nothing.
    pub comment: String,
}

impl std::fmt::Debug for DraftRating {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DraftRating")
            .field("verdict", &self.verdict)
            .field("reasons", &self.reasons.len())
            .finish_non_exhaustive()
    }
}

/// The tokens a request read and wrote, as the server reported them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Record)]
pub struct TokenUsage {
    /// Tokens read.
    pub prompt_tokens: u64,
    /// Tokens written.
    pub completion_tokens: u64,
}

impl From<Usage> for TokenUsage {
    fn from(usage: Usage) -> Self {
        Self {
            prompt_tokens: usage.prompt_tokens,
            completion_tokens: usage.completion_tokens,
        }
    }
}

impl From<TokenUsage> for Usage {
    fn from(usage: TokenUsage) -> Self {
        Self {
            prompt_tokens: usage.prompt_tokens,
            completion_tokens: usage.completion_tokens,
        }
    }
}

impl From<DraftRating> for Rating {
    fn from(rating: DraftRating) -> Self {
        Rating::new(
            match rating.verdict {
                DraftVerdict::Up => Verdict::Up,
                DraftVerdict::Down => Verdict::Down,
            },
            rating.reasons.into_iter().map(|reason| match reason {
                DraftRatingReason::WrongTone => Reason::WrongTone,
                DraftRatingReason::WrongLength => Reason::WrongLength,
                DraftRatingReason::MadeThingsUp => Reason::MadeThingsUp,
                DraftRatingReason::MissedThePoint => Reason::MissedThePoint,
                DraftRatingReason::WrongLanguage => Reason::WrongLanguage,
                DraftRatingReason::SomethingElse => Reason::SomethingElse,
            }),
            &rating.comment,
        )
    }
}
