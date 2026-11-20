//! The learning flow as the core decides it (`docs/ai.md` rules 11 to 14). A client draws it in
//! its flow host and words it; every control is a `ControlState`, and one that is `None` is not
//! drawn.

use mailcal_app::ai as core;

use super::records::{control, failed};
use crate::{AiFailure, AiRemedyControl, ControlState};

/// The learning flow, drawn in the flow host (pulled after `Surface::WritingStyle`); `None`
/// while no host draws it.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct WritingStyleLearning {
    pub stage: StyleLearningStage,
    /// The stages before `stage`, first to last, each where Back from the one after it goes: the
    /// path a navigation stack holds under the stage on screen. `Learning` and `Learned` stand on
    /// the review they were started from.
    pub trail: Vec<StyleLearningTrailStage>,
    pub back: Option<ControlState>,
    /// Next, on `ChooseAccount` and `ChooseRange`.
    pub next: Option<ControlState>,
    /// Learn, on `Review`; pressing it is the consent.
    pub learn: Option<ControlState>,
    /// Stop, on `Learning`. The failure's remedy is on `Failed`.
    pub stop: Option<ControlState>,
    /// Closing the host (`StyleLearningIntent::Close`). On `Learned` the reveal is on screen and
    /// this is the reveal's `close` in the same state; `Close` then leaves the reveal as
    /// `WritingStyleIntent::CloseReveal` does, and the flow ends once it is written as shown.
    pub close: ControlState,
}

/// A stage the flow can be taken back to, named without its data: what a navigation path keys on.
#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum StyleLearningTrailStage {
    ChooseAccount,
    ChooseRange,
    Review,
}

/// One account the flow can learn from.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct StyleLearningAccount {
    pub account_id: String,
    pub email: String,
}

/// Which sent mail to learn from. Days are `YYYY-MM-DD`, whole days in UTC. A range may reach
/// back past what the device holds: the review fetches the older sent mail first.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Enum)]
pub enum StyleLearnRange {
    /// All of it, as far back as the server holds it.
    Everything,
    /// Everything up to the end of last year.
    UntilEndOfLastYear,
    /// A range of the person's own, both days included, ending by `ChooseRange`'s `latest`.
    Between { since: String, until: String },
}

/// How far back the device holds an account's mail.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Enum)]
pub enum StyleSentReach {
    Everything,
    /// From this day, `YYYY-MM-DD`.
    Since {
        day: String,
    },
}

/// Where Learn sends what the review found.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Enum)]
pub enum StyleLearningDestination {
    /// Allodia's relay.
    Relay,
    /// The own endpoint, named by its host.
    OwnEndpoint { host: String },
}

/// One language of the review.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct StyleLearningLanguage {
    /// ISO 639-1 code.
    pub language: String,
    /// Messages in it that say enough to learn from.
    pub usable: u32,
}

/// What the review read on the device.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct StyleLearningReport {
    /// Messages found in the range.
    pub found: u32,
    /// Those that say enough to learn from, in any language.
    pub usable: u32,
    /// Per language, most messages first.
    pub languages: Vec<StyleLearningLanguage>,
    /// Usable messages left out because their language is not one the assistant names.
    pub other_languages: u32,
    /// The oldest and newest message read, seconds since the Unix epoch.
    pub oldest: Option<i64>,
    pub newest: Option<i64>,
}

/// The review's state.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Enum)]
pub enum StyleLearningReview {
    /// Fetching from the account's server the sent mail the range reaches and the device does not
    /// hold, which can take a while; nothing has left the device.
    Fetching,
    /// Reading on the device; nothing has left it.
    Reading,
    Ready {
        report: StyleLearningReport,
    },
    /// Nothing says enough to learn from; Learn is held.
    NothingUsable {
        report: StyleLearningReport,
    },
}

/// Where the flow stands, with what each stage shows.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Enum)]
pub enum StyleLearningStage {
    ChooseAccount {
        accounts: Vec<StyleLearningAccount>,
        selected: Option<String>,
    },
    ChooseRange {
        email: String,
        /// The presets, in order. The last is always `Between`, carrying the days a range of the
        /// person's own starts from before they change them.
        presets: Vec<StyleLearnRange>,
        /// The last day a range of the person's own may end on; it may start on any day.
        latest: String,
        selected: Option<StyleLearnRange>,
        /// How far back the device holds the account's mail; the review fetches what a range
        /// reaches further back.
        reach: StyleSentReach,
    },
    Review {
        email: String,
        range: StyleLearnRange,
        /// How far back the device holds the account's mail; the report's dates say how far back
        /// what was read reaches, older mail fetched for the review included.
        reach: StyleSentReach,
        /// The range reaches back past what the device holds and the older sent mail could not be
        /// fetched, so the review read only what the device holds, back to `reach`.
        older_not_fetched: bool,
        /// What the review found.
        state: StyleLearningReview,
        /// Where Learn sends; `None` when it cannot, and `cannot_send` says why. Both are `None`
        /// while the review is `NothingUsable`: nothing will be sent, so nothing is said about
        /// where.
        sends_to: Option<StyleLearningDestination>,
        cannot_send: Option<AiFailure>,
        /// Whether the account's server keeps the drafted-reply keyword; without it only this
        /// device knows which replies were drafted, which the review says.
        keyword_kept: bool,
    },
    Learning {
        done: u32,
        /// Zero until the run knows.
        total: u32,
    },
    Failed {
        failure: AiFailure,
        /// Its remedy (`StyleLearningIntent::Remedy` where it is a step of the flow).
        remedy: Option<AiRemedyControl>,
    },
    /// The reveal of `style` follows in the same host.
    Learned { style: String },
}

fn day(date: time::Date) -> String {
    format!(
        "{:04}-{:02}-{:02}",
        date.year(),
        u8::from(date.month()),
        date.day()
    )
}

/// A `YYYY-MM-DD` day, or `None` when it is not one.
fn parse_day(text: &str) -> Option<time::Date> {
    let mut parts = text.trim().splitn(3, '-');
    let year = parts.next()?.parse().ok()?;
    let month = time::Month::try_from(parts.next()?.parse::<u8>().ok()?).ok()?;
    let day = parts.next()?.parse().ok()?;
    time::Date::from_calendar_date(year, month, day).ok()
}

impl From<core::LearnRange> for StyleLearnRange {
    fn from(range: core::LearnRange) -> Self {
        match range {
            core::LearnRange::Everything => Self::Everything,
            core::LearnRange::UntilEndOfLastYear => Self::UntilEndOfLastYear,
            core::LearnRange::Between { since, until } => Self::Between {
                since: day(since),
                until: day(until),
            },
        }
    }
}

impl StyleLearnRange {
    /// The core's range, or `None` for a day that is not one.
    pub(crate) fn to_core(&self) -> Option<core::LearnRange> {
        Some(match self {
            Self::Everything => core::LearnRange::Everything,
            Self::UntilEndOfLastYear => core::LearnRange::UntilEndOfLastYear,
            Self::Between { since, until } => core::LearnRange::Between {
                since: parse_day(since)?,
                until: parse_day(until)?,
            },
        })
    }
}

impl From<core::SentReach> for StyleSentReach {
    fn from(reach: core::SentReach) -> Self {
        match reach {
            core::SentReach::Everything => Self::Everything,
            core::SentReach::Since(date) => Self::Since { day: day(date) },
        }
    }
}

fn report(report: mailcal_ai::corpus::CorpusReport) -> StyleLearningReport {
    StyleLearningReport {
        found: report.found,
        usable: report.usable,
        languages: report
            .languages
            .into_iter()
            .map(|language| StyleLearningLanguage {
                language: language.language.as_str().to_owned(),
                usable: language.usable,
            })
            .collect(),
        other_languages: report.undetected,
        oldest: report.oldest,
        newest: report.newest,
    }
}

/// The learning flow's record.
pub(crate) fn learning(view: core::LearningView) -> WritingStyleLearning {
    let stage = match view.stage {
        core::LearningStage::ChooseAccount { accounts, selected } => {
            StyleLearningStage::ChooseAccount {
                accounts: accounts
                    .into_iter()
                    .map(|account| StyleLearningAccount {
                        account_id: account.id,
                        email: account.address,
                    })
                    .collect(),
                selected,
            }
        }
        core::LearningStage::ChooseRange {
            address,
            ranges,
            selected,
            reach,
        } => StyleLearningStage::ChooseRange {
            email: address,
            presets: ranges.presets.into_iter().map(Into::into).collect(),
            latest: day(ranges.latest),
            selected: selected.map(Into::into),
            reach: reach.into(),
        },
        core::LearningStage::Review {
            address,
            range,
            reach,
            older_not_fetched,
            state,
            sends_to,
            keyword_kept,
        } => StyleLearningStage::Review {
            email: address,
            range: range.into(),
            reach: reach.into(),
            older_not_fetched,
            state: match state {
                core::Review::Fetching => StyleLearningReview::Fetching,
                core::Review::Reading => StyleLearningReview::Reading,
                core::Review::Ready(found) => StyleLearningReview::Ready {
                    report: report(found),
                },
                core::Review::NothingUsable(found) => StyleLearningReview::NothingUsable {
                    report: report(found),
                },
            },
            sends_to: match &sends_to {
                Some(Ok(core::AiRoute::Relay)) => Some(StyleLearningDestination::Relay),
                Some(Ok(core::AiRoute::OwnEndpoint { host, .. })) => {
                    Some(StyleLearningDestination::OwnEndpoint { host: host.clone() })
                }
                Some(Err(_)) | None => None,
            },
            cannot_send: sends_to.and_then(Result::err).map(Into::into),
            keyword_kept,
        },
        core::LearningStage::Learning { done, total } => {
            StyleLearningStage::Learning { done, total }
        }
        core::LearningStage::Failed { failure, remedy } => {
            let (failure, remedy) = failed(failure, remedy);
            StyleLearningStage::Failed { failure, remedy }
        }
        core::LearningStage::Learned { style } => StyleLearningStage::Learned {
            style: style.as_str().to_owned(),
        },
    };
    WritingStyleLearning {
        stage,
        trail: view
            .trail
            .into_iter()
            .map(|stage| match stage {
                core::LearningTrailStage::ChooseAccount => StyleLearningTrailStage::ChooseAccount,
                core::LearningTrailStage::ChooseRange => StyleLearningTrailStage::ChooseRange,
                core::LearningTrailStage::Review => StyleLearningTrailStage::Review,
            })
            .collect(),
        back: view.back.map(control),
        next: view.next.map(control),
        learn: view.learn.map(control),
        stop: view.stop.map(control),
        close: control(view.close),
    }
}
