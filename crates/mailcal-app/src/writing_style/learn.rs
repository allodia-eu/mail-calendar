//! Learning a writing style from an account's sent mail.
//!
//! Everything up to the request happens on the device: reading the Sent folder, cutting each
//! message to its author's own words, sampling. [`App::sent_corpus_report`] runs exactly that and
//! stops, so the consent screen can say what would be sent, and roughly how much, before the
//! person agrees; [`App::learn_writing_style`] runs it again and then sends.
//!
//! **Blocking.** The requests wait for their answers, so a host calls the learn off the main
//! thread, as it does the Allodia account sync. Progress is published on
//! [`Surface::WritingStyle`](crate::Surface::WritingStyle), and
//! [`App::cancel_writing_style_learning`] stops the run before its next request.

use std::sync::atomic::Ordering;

use engine_api::{AccountId, Provider};
use mailcal_account::StoredWritingStyle;
use mailcal_ai::{
    LearnOptions, LearnProgress,
    corpus::{self, CorpusOptions, CorpusReport},
    learn_style,
    wire::Metering,
};
use mailcal_viewmodel::{LearningProgress, LearningStage};

use super::WritingStyleError;
use crate::{App, Surface};

/// Which sent mail to learn from, in seconds since the Unix epoch; both ends inclusive and both
/// optional. The default is everything the device holds.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct LearnRange {
    /// The oldest instant to include.
    pub since: Option<i64>,
    /// The newest instant to include: "until the end of 2024", to leave out what was written
    /// with an assistant's help.
    pub until: Option<i64>,
}

impl LearnRange {
    /// Whether a message sent at `instant` is inside. An undated message is inside only when the
    /// range has no ends.
    pub(super) fn admits(self, instant: Option<i64>) -> bool {
        match instant {
            Some(instant) => {
                self.since.is_none_or(|since| instant >= since)
                    && self.until.is_none_or(|until| instant <= until)
            }
            None => self.since.is_none() && self.until.is_none(),
        }
    }
}

/// A style learned and stored.
#[derive(Debug, Clone, PartialEq)]
pub struct LearnReport {
    /// The new style's id.
    pub style_id: String,
    /// The languages it covers.
    pub languages: Vec<String>,
    /// How many messages it was learned from.
    pub messages: u32,
    /// What the relay charged and the balance after; `None` from an own endpoint.
    pub metering: Option<Metering>,
}

/// A run that stored nothing, and what it had cost by then.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
#[error("{error}")]
pub struct LearnFailure {
    /// Why.
    pub error: WritingStyleError,
    /// What the requests before it cost.
    pub metering: Option<Metering>,
}

impl From<WritingStyleError> for LearnFailure {
    fn from(error: WritingStyleError) -> Self {
        Self {
            error,
            metering: None,
        }
    }
}

impl<P: Provider> App<P> {
    /// What learning from `account`'s sent mail in `range` would read and send, without sending
    /// anything: the counts per language, the dates, and the device's horizon.
    ///
    /// # Errors
    ///
    /// Returns [`WritingStyleError::NoSentFolder`] when the account has no Sent folder.
    pub async fn sent_corpus_report(
        &self,
        account: &AccountId,
        range: LearnRange,
    ) -> Result<CorpusReport, WritingStyleError> {
        Ok(self.corpus(account, range).await?.report)
    }

    /// Learns a style from `account`'s sent mail in `range`, stores it under `name`, and assigns
    /// it to the account when the account had none. `ui_language` is the language the person reads
    /// the app in, which the description is written in.
    ///
    /// # Errors
    ///
    /// Returns a [`LearnFailure`]: [`WritingStyleError::Unavailable`] with no backend,
    /// [`WritingStyleError::Busy`] while another run is going,
    /// [`WritingStyleError::NothingToLearn`] when no message says enough, or the backend's error,
    /// [`mailcal_ai::AiError::Cancelled`] included; with what the requests so far cost.
    pub async fn learn_writing_style(
        &self,
        account: &AccountId,
        range: LearnRange,
        name: String,
        ui_language: &str,
    ) -> Result<LearnReport, LearnFailure> {
        let backend = self
            .writing_style
            .backend()
            .ok_or(WritingStyleError::Unavailable)?;
        // Asked before the Sent folder is read, so a refused run costs nothing at all.
        backend
            .check()
            .map_err(|refused| WritingStyleError::Ai(mailcal_ai::AiError::Refused(refused)))?;
        self.begin_learning(account)?;
        let result = self
            .run_learning(account, range, name, ui_language, &backend)
            .await;
        *self
            .writing_style
            .learning
            .lock()
            .expect("learning state poisoned") = None;
        self.observer.surface_changed(Surface::WritingStyle);
        result
    }

    /// Stops a learning run before its next request.
    pub fn cancel_writing_style_learning(&self) {
        self.writing_style.cancel.store(true, Ordering::Relaxed);
    }

    fn begin_learning(&self, account: &AccountId) -> Result<(), WritingStyleError> {
        let mut learning = self
            .writing_style
            .learning
            .lock()
            .expect("learning state poisoned");
        if learning.is_some() {
            return Err(WritingStyleError::Busy);
        }
        self.writing_style.cancel.store(false, Ordering::Relaxed);
        *learning = Some(LearningProgress {
            account_id: account.as_str().to_owned(),
            stage: LearningStage::Reading,
            done: 0,
            total: 0,
        });
        drop(learning);
        self.observer.surface_changed(Surface::WritingStyle);
        Ok(())
    }

    async fn run_learning(
        &self,
        account: &AccountId,
        range: LearnRange,
        name: String,
        ui_language: &str,
        backend: &mailcal_ai::GatedBackend,
    ) -> Result<LearnReport, LearnFailure> {
        let corpus = self.corpus(account, range).await?;
        if corpus.languages.values().all(Vec::is_empty) {
            return Err(WritingStyleError::NothingToLearn.into());
        }
        let progress = |step: LearnProgress| {
            if let Some(learning) = self
                .writing_style
                .learning
                .lock()
                .expect("learning state poisoned")
                .as_mut()
            {
                learning.stage = LearningStage::Learning;
                learning.done = step.done;
                learning.total = step.total;
            }
            self.observer.surface_changed(Surface::WritingStyle);
        };
        let learned = learn_style(
            &corpus,
            backend,
            &LearnOptions {
                ui_language,
                now: time::OffsetDateTime::now_utc().unix_timestamp(),
                cancel: &self.writing_style.cancel,
                progress: &progress,
            },
        )
        .map_err(|failed| {
            self.note_metering(failed.metering);
            LearnFailure {
                error: failed.error.into(),
                metering: failed.metering,
            }
        })?;
        self.note_metering(learned.metering);

        let messages = corpus
            .languages
            .values()
            .map(|sample| u32::try_from(sample.len()).unwrap_or(u32::MAX))
            .sum();
        let languages = learned.guide.languages.keys().cloned().collect();
        let style_id = self.store_writing_style(
            account.as_str(),
            StoredWritingStyle {
                name,
                source: account.as_str().to_owned(),
                guide_json: serde_json::to_string(&learned.guide).unwrap_or_default(),
                exemplars_json: serde_json::to_string(&learned.exemplars).unwrap_or_default(),
            },
        );
        Ok(LearnReport {
            style_id: style_id.as_str().to_owned(),
            languages,
            messages,
            metering: learned.metering,
        })
    }

    /// The corpus for `account` in `range`, built on the device.
    async fn corpus(
        &self,
        account: &AccountId,
        range: LearnRange,
    ) -> Result<corpus::Corpus, WritingStyleError> {
        let (messages, horizon) = self.sent_mail(account, range).await?;
        let signatures = self
            .signatures
            .lock()
            .expect("signatures mutex poisoned")
            .plain_bodies();
        Ok(corpus::build(
            messages,
            &CorpusOptions {
                signatures,
                horizon,
            },
        ))
    }
}
