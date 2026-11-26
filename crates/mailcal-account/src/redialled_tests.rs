//! The races [`Redialled`] settles: which drop counts, and which dial a waiting caller takes.

use std::{
    collections::VecDeque,
    sync::{
        Arc, Mutex,
        atomic::{AtomicUsize, Ordering},
    },
    time::Duration,
};

use engine_core::error::FailureClass;
use engine_provider::{ProviderError, ProviderResult};

use super::{Redial, Redialled};

/// A provider that is the number of the dial that made it (the first connect is 0), its dials
/// answering `answers` in turn after a second each, and how many dials were made.
fn dialling(answers: Vec<ProviderResult<usize>>) -> (Redialled<usize>, Arc<AtomicUsize>) {
    let dials = Arc::new(AtomicUsize::new(0));
    let counted = Arc::clone(&dials);
    let answers = Arc::new(Mutex::new(VecDeque::from(answers)));
    let redial: Redial<usize> = Box::new(move || {
        counted.fetch_add(1, Ordering::SeqCst);
        let answer = answers
            .lock()
            .unwrap()
            .pop_front()
            .expect("a dial too many");
        Box::pin(async move {
            tokio::time::sleep(Duration::from_secs(1)).await;
            answer
        })
    });
    (Redialled::adopt(0, redial), dials)
}

fn unreachable() -> ProviderResult<usize> {
    Err(ProviderError::retryable("the server could not be reached"))
}

fn generation(provider: &Redialled<usize>) -> u64 {
    provider.lock().generation
}

/// Two requests that failed on the same provider drop it once.
#[tokio::test]
async fn two_failures_on_one_provider_drop_it_once() {
    let (provider, _) = dialling(vec![]);
    let (on, _) = provider.get().await.unwrap();

    provider.drop_dead(on);
    provider.drop_dead(on);

    assert_eq!(generation(&provider), 1);
}

/// A request that failed on a provider already replaced keeps the new one.
#[tokio::test(start_paused = true)]
async fn a_drop_of_an_older_provider_keeps_the_new_one() {
    let (provider, dials) = dialling(vec![Ok(1)]);
    provider.drop_dead(0);
    let (on, connected) = provider.get().await.unwrap();

    provider.drop_dead(0);

    assert_eq!(provider.get().await.unwrap(), (on, connected));
    assert_eq!(*provider.last(), 1);
    assert_eq!(dials.load(Ordering::SeqCst), 1);
}

/// A dial that failed is not remembered past the next caller that dials again.
#[tokio::test(start_paused = true)]
async fn a_failed_dial_then_one_that_connects() {
    let (provider, dials) = dialling(vec![unreachable(), Ok(2)]);
    provider.drop_dead(0);

    let failed = provider.get().await.unwrap_err();
    let (_, connected) = provider.get().await.unwrap();

    assert_eq!(failed.class(), FailureClass::Retryable);
    assert_eq!(*connected, 2);
    assert_eq!(dials.load(Ordering::SeqCst), 2);
}

/// Callers that find no provider at once share one dial.
#[tokio::test(start_paused = true)]
async fn concurrent_callers_share_one_dial() {
    let (provider, dials) = dialling(vec![Ok(1)]);
    provider.drop_dead(0);

    let (first, second) = tokio::join!(provider.get(), provider.get());

    assert_eq!(*first.unwrap().1, 1);
    assert_eq!(*second.unwrap().1, 1);
    assert_eq!(dials.load(Ordering::SeqCst), 1);
}

/// Callers that waited on a dial that failed take its failure rather than dial again each, so an
/// unreachable server costs one deadline however many requests were waiting.
#[tokio::test(start_paused = true)]
async fn callers_waiting_on_a_failed_dial_share_its_failure() {
    let (provider, dials) = dialling(vec![unreachable()]);
    provider.drop_dead(0);

    let started = tokio::time::Instant::now();
    let (first, second, third) = tokio::join!(provider.get(), provider.get(), provider.get());

    for failed in [first, second, third] {
        assert_eq!(failed.unwrap_err().class(), FailureClass::Retryable);
    }
    assert_eq!(dials.load(Ordering::SeqCst), 1);
    assert_eq!(started.elapsed(), Duration::from_secs(1), "one dial's wait");
}
