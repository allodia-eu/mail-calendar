//! Conversions from the core's writing-style types into the FFI records in
//! [`records_writing_style`](crate::records_writing_style).

use mailcal_ai::{
    AiError,
    corpus::{CorpusReport as AiCorpusReport, LanguageReport as AiLanguageReport},
};
use mailcal_app::{DraftReply as AppDraftReply, LearnReport as AppLearnReport, WritingStyleError};
use mailcal_viewmodel::{
    AccountWritingStyleRow as AppAccountRow, AiRoute as AppAiRoute, HabitRow as AppHabitRow,
    LanguageStyleRow as AppLanguageStyleRow, LearningProgress as AppLearningProgress,
    LearningStage as AppLearningStage, WritingStyleDetail as AppDetail, WritingStyleRow as AppRow,
    WritingStyleSnapshot as AppSnapshot,
};

use crate::records_writing_style::{
    AccountWritingStyleRow, AiCharge, AiRoute, CorpusLanguage, CorpusReport, CreditBalance,
    DraftReply, GateRefusal, HabitRow, LanguageStyleRow, LearnReport, LearningProgress,
    LearningStage, WritingStyleDetail, WritingStyleFailure, WritingStyleRow, WritingStyleSnapshot,
};

impl From<WritingStyleError> for WritingStyleFailure {
    fn from(error: WritingStyleError) -> Self {
        match error {
            WritingStyleError::Unavailable => Self::Unavailable,
            WritingStyleError::Busy => Self::Busy,
            WritingStyleError::NoSentFolder => Self::NoSentFolder,
            WritingStyleError::NothingToLearn => Self::NothingToLearn,
            WritingStyleError::NoStyle => Self::NoStyle,
            WritingStyleError::NotFound => Self::NotFound,
            WritingStyleError::Ai(error) => match error {
                AiError::Refused(refused) => Self::Refused {
                    mode: refused.mode.into(),
                    class: refused.class.into(),
                },
                AiError::OutOfCredits => Self::OutOfCredits,
                AiError::NotEntitled => Self::NotEntitled,
                AiError::Unauthorized => Self::Unauthorized,
                AiError::RateLimited => Self::RateLimited,
                AiError::Unreachable => Self::Unreachable,
                AiError::Status(code) => Self::Status { code },
                AiError::Malformed => Self::Malformed,
                AiError::Cancelled => Self::Cancelled,
            },
        }
    }
}

fn charge(metering: Option<mailcal_ai::wire::Metering>) -> Option<AiCharge> {
    metering.map(|metering| AiCharge {
        credits_charged: metering.credits_charged,
        balance_credits: metering.balance_credits,
    })
}

impl From<AppLearnReport> for LearnReport {
    fn from(report: AppLearnReport) -> Self {
        Self {
            style_id: report.style_id,
            languages: report.languages,
            messages: report.messages,
            charge: charge(report.metering),
        }
    }
}

impl From<AppDraftReply> for DraftReply {
    fn from(draft: AppDraftReply) -> Self {
        Self {
            draft_id: draft.draft_id,
            text: draft.text,
            gaps: draft.gaps,
            language: draft.language,
            charge: charge(draft.metering),
        }
    }
}

impl From<AiLanguageReport> for CorpusLanguage {
    fn from(language: AiLanguageReport) -> Self {
        Self {
            language: language.language,
            usable: language.usable,
            sampled: language.sampled,
            tokens: language.tokens,
        }
    }
}

impl From<AiCorpusReport> for CorpusReport {
    fn from(report: AiCorpusReport) -> Self {
        Self {
            found: report.found,
            usable: report.usable,
            undetected: report.undetected,
            languages: report.languages.into_iter().map(Into::into).collect(),
            oldest: report.oldest,
            newest: report.newest,
            horizon: report.horizon,
        }
    }
}

impl From<AppRow> for WritingStyleRow {
    fn from(row: AppRow) -> Self {
        Self {
            id: row.id,
            name: row.name,
            source_account: row.source_account,
            languages: row.languages,
            messages: row.messages,
            oldest: row.oldest,
            newest: row.newest,
            learned_at: row.learned_at,
        }
    }
}

impl From<AppHabitRow> for HabitRow {
    fn from(habit: AppHabitRow) -> Self {
        Self {
            text: habit.text,
            share: habit.share,
        }
    }
}

impl From<AppLanguageStyleRow> for LanguageStyleRow {
    fn from(style: AppLanguageStyleRow) -> Self {
        Self {
            language: style.language,
            greetings: style.greetings.into_iter().map(Into::into).collect(),
            sign_offs: style.sign_offs.into_iter().map(Into::into).collect(),
            signs_as: style.signs_as,
            register: style.register,
            typical_words: style.typical_words,
            shape: style.shape,
            punctuation: style.punctuation,
            structure: style.structure,
            moves: style.moves,
            phrases: style.phrases,
            avoid: style.avoid,
        }
    }
}

impl From<AppDetail> for WritingStyleDetail {
    fn from(detail: AppDetail) -> Self {
        Self {
            row: detail.row.into(),
            notes: detail.notes,
            languages: detail.languages.into_iter().map(Into::into).collect(),
        }
    }
}

impl From<AppSnapshot> for WritingStyleSnapshot {
    fn from(snapshot: AppSnapshot) -> Self {
        Self {
            route: snapshot.route.map(|route| match route {
                AppAiRoute::Relay => AiRoute::Relay,
                AppAiRoute::OwnEndpoint => AiRoute::OwnEndpoint,
            }),
            refused: snapshot.refused.map(|refused| GateRefusal {
                mode: refused.mode.into(),
                class: refused.class.into(),
            }),
            styles: snapshot.styles.into_iter().map(Into::into).collect(),
            accounts: snapshot
                .accounts
                .into_iter()
                .map(|row: AppAccountRow| AccountWritingStyleRow {
                    account_id: row.account_id,
                    email: row.email,
                    style: row.style,
                })
                .collect(),
            learning: snapshot
                .learning
                .map(|learning: AppLearningProgress| LearningProgress {
                    account_id: learning.account_id,
                    stage: match learning.stage {
                        AppLearningStage::Reading => LearningStage::Reading,
                        AppLearningStage::Learning => LearningStage::Learning,
                    },
                    done: learning.done,
                    total: learning.total,
                }),
            balance: snapshot.balance.map(|balance| CreditBalance {
                credits: balance.credits,
                as_of: balance.as_of,
            }),
        }
    }
}
