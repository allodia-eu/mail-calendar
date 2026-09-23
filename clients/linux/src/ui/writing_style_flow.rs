//! Where the learning sheet stands, decided without a widget (`docs/ai.md`, "Learning").
//!
//! Which account, which sent mail, what that holds on this device, the consent, and the run the
//! consent started. The sheet in Settings draws each stage and runs the two blocking calls; the
//! answers come back here, tagged, so one that arrives after the person changed their mind lands
//! nowhere.

use mailcal_bindings::{
    AiRoute, CorpusReport, LearningProgress, LearningStage, WritingStyleFailure,
};

use super::{timestamps, writing_style::languages_text};
use crate::l10n;

/// Which sent mail a run reads, in seconds since the Unix epoch; either end open.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct Range {
    pub(crate) since: Option<i64>,
    pub(crate) until: Option<i64>,
}

impl Range {
    /// Everything up to and including the day `year`-`month`-`day`, which ends at its last second
    /// in `zone`: "until the end of 2024" has to keep the evening of the 31st.
    pub(crate) fn up_to(year: i32, month: i32, day: i32, zone: &str) -> Option<Self> {
        let date = jiff::civil::Date::new(
            i16::try_from(year).ok()?,
            i8::try_from(month).ok()?,
            i8::try_from(day).ok()?,
        )
        .ok()?;
        let end = date.in_tz(zone).ok()?.end_of_day().ok()?;
        Some(Self {
            since: None,
            until: Some(end.timestamp().as_second()),
        })
    }
}

/// Where the sheet is.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum LearnStage {
    /// Which account to learn from. Skipped when there is only one.
    Account,
    /// The chosen range is being read on the device; nothing has left it.
    Reading,
    /// What the range holds, and the consent that sends it.
    Report(CorpusReport),
    /// The run the person consented to, with the progress the surface last reported for it.
    Learning(Option<LearningProgress>),
    Failed(WritingStyleFailure),
    /// A style was learned and stored; the id opens it.
    Learned(String),
}

/// A report to read. The ticket travels with it, so only the answer to the latest one lands.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ReportRequest {
    pub(crate) ticket: u64,
    pub(crate) account: String,
    pub(crate) range: Range,
}

/// A run to start: what the person consented to, exactly.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct LearnRequest {
    pub(crate) account: String,
    pub(crate) range: Range,
}

#[derive(Clone, Debug)]
pub(crate) struct LearnFlow {
    account: Option<String>,
    range: Range,
    ticket: u64,
    stage: LearnStage,
}

impl LearnFlow {
    /// Starts on the account question, or past it when there is only one account to ask about;
    /// then with the report for everything on the device, which is the range the sheet opens on.
    pub(crate) fn new(accounts: &[String]) -> (Self, Option<ReportRequest>) {
        let mut flow = Self {
            account: None,
            range: Range::default(),
            ticket: 0,
            stage: LearnStage::Account,
        };
        let request = if let [only] = accounts {
            flow.choose_account(only.clone())
        } else {
            None
        };
        (flow, request)
    }

    pub(crate) const fn stage(&self) -> &LearnStage {
        &self.stage
    }

    pub(crate) fn choose_account(&mut self, account: String) -> Option<ReportRequest> {
        if self.stage != LearnStage::Account {
            return None;
        }
        self.account = Some(account);
        self.read()
    }

    /// A different range reads the device again. Not once the sample has been sent: the run
    /// already has the range it was consented to.
    pub(crate) fn choose_range(&mut self, range: Range) -> Option<ReportRequest> {
        if !self.range_open() {
            return None;
        }
        self.range = range;
        self.read()
    }

    fn read(&mut self) -> Option<ReportRequest> {
        let account = self.account.clone()?;
        self.ticket = self.ticket.wrapping_add(1);
        self.stage = LearnStage::Reading;
        Some(ReportRequest {
            ticket: self.ticket,
            account,
            range: self.range,
        })
    }

    pub(crate) fn report_arrived(
        &mut self,
        ticket: u64,
        report: Result<CorpusReport, WritingStyleFailure>,
    ) {
        if ticket != self.ticket || self.stage != LearnStage::Reading {
            return;
        }
        self.stage = match report {
            Ok(report) => LearnStage::Report(report),
            Err(failure) => LearnStage::Failed(failure),
        };
    }

    /// Pressing Learn under the consent: the one act that sends anything. Nothing to learn from
    /// offers no Learn, so it consents to nothing either.
    pub(crate) fn consent(&mut self) -> Option<LearnRequest> {
        let LearnStage::Report(report) = &self.stage else {
            return None;
        };
        if report.usable == 0 {
            return None;
        }
        let account = self.account.clone()?;
        self.stage = LearnStage::Learning(None);
        Some(LearnRequest {
            account,
            range: self.range,
        })
    }

    /// Takes the progress a snapshot carried, when it is this run's.
    pub(crate) fn progress(&mut self, learning: Option<&LearningProgress>) {
        let Some(progress) = learning
            .filter(|progress| self.account.as_deref() == Some(progress.account_id.as_str()))
        else {
            return;
        };
        if let LearnStage::Learning(shown) = &mut self.stage {
            *shown = Some(progress.clone());
        }
    }

    /// The run's answer: the new style's id, or why there is none.
    pub(crate) fn finished(&mut self, result: Result<String, WritingStyleFailure>) {
        if !self.is_learning() {
            return;
        }
        self.stage = match result {
            Ok(style) => LearnStage::Learned(style),
            Err(failure) => LearnStage::Failed(failure),
        };
    }

    pub(crate) fn learned(&self) -> Option<&str> {
        if let LearnStage::Learned(style) = &self.stage {
            Some(style.as_str())
        } else {
            None
        }
    }

    pub(crate) const fn is_learning(&self) -> bool {
        matches!(self.stage, LearnStage::Learning(_))
    }

    /// Whether the range can still change: once an account is chosen, and until the run starts.
    pub(crate) const fn range_open(&self) -> bool {
        self.account.is_some()
            && !matches!(self.stage, LearnStage::Learning(_) | LearnStage::Learned(_))
    }
}

/// The report as the consent sheet states it, one fact a line. The undetected count and the
/// device's horizon appear only when there is something to say.
pub(crate) fn report_lines(report: &CorpusReport, zone: &str, locale: &str) -> Vec<String> {
    let mut lines = vec![
        l10n::learn_report_found(i64::from(report.found)),
        l10n::learn_report_usable(i64::from(report.usable)),
    ];
    let languages = report
        .languages
        .iter()
        .map(|language| language.language.clone())
        .collect::<Vec<_>>();
    if !languages.is_empty() {
        lines.push(l10n::learn_report_languages(&languages_text(&languages)));
    }
    if report.undetected > 0 {
        lines.push(l10n::learn_report_undetected(i64::from(report.undetected)));
    }
    if let Some(day) = report
        .horizon
        .and_then(|horizon| timestamps::local_day(horizon, zone, locale))
    {
        lines.push(l10n::learn_report_horizon(&day));
    }
    lines
}

/// What will be sent where, by the route requests take. `None` when they have nowhere to go, in
/// which case there is nothing to consent to.
pub(crate) fn consent_text(route: Option<AiRoute>, own_base_url: Option<&str>) -> Option<String> {
    match route? {
        AiRoute::Relay => Some(l10n::learn_consent_relay().to_owned()),
        AiRoute::OwnEndpoint => Some(l10n::learn_consent_own(&host_of(
            own_base_url.unwrap_or_default(),
        ))),
    }
}

/// The host of an endpoint's address, which is the name the person gave it; the address itself
/// when it does not parse.
fn host_of(base_url: &str) -> String {
    url::Url::parse(base_url)
        .ok()
        .and_then(|url| url.host_str().map(str::to_owned))
        .unwrap_or_else(|| base_url.to_owned())
}

/// What a running learn says, and how full its bar is. Reading has no bar to fill: it does not
/// know how many requests it will make until it has read everything.
pub(crate) fn progress_text(progress: Option<&LearningProgress>) -> (String, Option<f64>) {
    let Some(progress) = progress.filter(|progress| progress.stage == LearningStage::Learning)
    else {
        return (l10n::learn_reading().to_owned(), None);
    };
    let text = l10n::learn_progress(&progress.done.to_string(), &progress.total.to_string());
    if progress.total == 0 {
        return (text, None);
    }
    let fraction = f64::from(progress.done) / f64::from(progress.total);
    (text, Some(fraction.min(1.0)))
}

#[cfg(test)]
#[path = "writing_style_flow_tests.rs"]
mod tests;
