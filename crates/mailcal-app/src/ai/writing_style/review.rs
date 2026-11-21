//! The review's read (`docs/ai.md` rule 11, `Review`): the range's sent mail read on the device,
//! its older part fetched from the account's server first when the device does not hold it. It
//! sends nothing to a model, so it runs on the host's runtime beside the queue.

use engine_api::{AccountId, Provider};
use mailcal_ai::corpus::{self, CorpusOptions};
use time::{Date, Time};

use super::{
    flow::{OlderSent, Review, Reviewed, Stage},
    range::{RangeFetch, today},
};
use crate::{App, Surface, ai::AiFailure};

/// Whether sent mail held back to `horizon` (seconds since the Unix epoch) reaches `first_day`.
fn reaches(horizon: Option<i64>, first_day: Option<Date>) -> bool {
    match (horizon, first_day) {
        (Some(horizon), Some(day)) => {
            horizon < day.with_time(Time::MIDNIGHT).assume_utc().unix_timestamp() + 86_400
        }
        _ => false,
    }
}

impl<P: Provider> App<P> {
    /// Reads what the review under `ticket` found, on the device, fetching first the older sent
    /// mail the range reaches when the device does not hold it. An answer for a review the person
    /// has since left is dropped, and older mail fetched for it is removed all the same. Sends
    /// nothing.
    pub async fn read_learning_review(&self, ticket: u64) {
        let asked = self.flows().current(ticket).and_then(|flow| {
            (flow.stage == Stage::Review(Review::Reading))
                .then(|| Some((flow.account.clone()?.0, flow.range?)))
                .flatten()
        });
        let Some((account, range)) = asked else {
            return;
        };
        // One read holding Sent further back at a time, so one's narrowing never runs under
        // another's read.
        let _one_at_a_time = self.ai.writing_style.fetching.lock().await;
        let today = today();
        let first_day = range.first_day(today);
        let fetch = range.fetch(self.sent_reach(&account), today) != RangeFetch::Nothing;
        let mut complete = true;
        if fetch {
            if !self.review_moves(ticket, &Review::Reading, Review::Fetching) {
                return;
            }
            complete = self.fetch_older_sent(&account, first_day).await;
            if !self.review_moves(ticket, &Review::Fetching, Review::Reading) {
                return self.release_older_sent(&account).await;
            }
        }
        let read = self
            .learnable_sent_mail(&account, range.bounds(today))
            .await;
        let horizon = read.as_ref().and_then(|(_, horizon)| *horizon);
        let older = match fetch {
            false => OlderSent::NotNeeded,
            true if complete || reaches(horizon, first_day) => OlderSent::Fetched,
            true => OlderSent::NotFetched,
        };
        self.settle_review(ticket, &account, read, older).await;
        if fetch {
            self.release_older_sent(&account).await;
        }
    }

    /// Moves the review under `ticket` from `from` to `to`, signalling the page; false when the
    /// person has left it.
    fn review_moves(&self, ticket: u64, from: &Review, to: Review) -> bool {
        {
            let mut flows = self.flows();
            let Some(flow) = flows
                .current(ticket)
                .filter(|flow| flow.stage == Stage::Review(from.clone()))
            else {
                return false;
            };
            flow.stage = Stage::Review(to);
        }
        self.observer.surface_changed(Surface::WritingStyle);
        true
    }

    /// Puts what the read found on the review under `ticket`, unless the person has left it.
    async fn settle_review(
        &self,
        ticket: u64,
        account: &AccountId,
        read: Option<(Vec<mailcal_ai::corpus::SentMessage>, Option<i64>)>,
        older: OlderSent,
    ) {
        let another_account = self.accounts.senders().len() > 1;
        let keyword_kept = self.keeps_drafted_keyword(account).await;
        let found = read.map(|(messages, horizon)| {
            let signatures = self.signature_plain_texts();
            let corpus = corpus::build(
                messages,
                &CorpusOptions {
                    signatures,
                    horizon,
                },
            );
            let report = &corpus.report;
            log::info!(
                "ai: the review found {} sent message(s), {} usable, {} not loaded",
                report.found,
                report.usable,
                report.not_loaded,
            );
            corpus
        });
        {
            let mut flows = self.flows();
            let Some(flow) = flows
                .current(ticket)
                .filter(|flow| flow.stage == Stage::Review(Review::Reading))
            else {
                return;
            };
            flow.older = older;
            match found {
                None => flow.stage = Stage::Failed(AiFailure::NoSentFolder { another_account }),
                Some(corpus) if corpus.languages.values().all(Vec::is_empty) => {
                    flow.stage = Stage::Review(Review::NothingUsable(corpus.report));
                }
                Some(corpus) => {
                    flow.stage = Stage::Review(Review::Ready(corpus.report.clone()));
                    flow.reviewed = Some(Reviewed {
                        corpus,
                        keyword_kept,
                    });
                }
            }
        }
        self.observer.surface_changed(Surface::WritingStyle);
    }

    /// The plain text of every signature in the library, which the corpus cuts from a message
    /// where its client wrote no delimiter above it.
    fn signature_plain_texts(&self) -> Vec<String> {
        self.signatures
            .lock()
            .expect("signatures mutex poisoned")
            .library
            .entries
            .values()
            .map(|signature| signature.body_plain.clone())
            .collect()
    }
}
