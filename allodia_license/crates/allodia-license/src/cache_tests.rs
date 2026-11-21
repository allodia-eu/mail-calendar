//! A stored answer that cannot grant: one the service refused to give, and one dated after the
//! device's clock.

use crate::{
    Cache, Capability, GRACE_SECONDS, Outcome, Stored,
    tests::{DAY, NOW, paid},
};

#[test]
fn a_refused_read_grants_nothing_at_once_and_asks_again() {
    // The service was reached with a token minted for it and would not answer for this account:
    // that is its answer, not an outage, so the stored one goes rather than standing through grace.
    let mut cache = Cache::default();
    cache.apply(Outcome::Answered(paid()), NOW);
    cache.apply(Outcome::Refused, NOW + DAY);
    assert!(!cache.effective(NOW + DAY).grants(&Capability::Push));
    assert_eq!(cache.stored(), None, "nothing is kept to grant later");
    assert!(cache.should_refresh(NOW + DAY));
    assert!(!cache.lapses_soon(NOW + GRACE_SECONDS - 1));
}

#[test]
fn an_answer_dated_after_now_is_stale_and_due() {
    // Device clocks move, and a stored file can be edited. An answer taken "in the future" has no
    // age that can be trusted, so it grants nothing and is asked again at once; otherwise it would
    // grant for as long as the clock stays behind it, past any grace.
    let mut cache = Cache::default();
    cache.apply(Outcome::Answered(paid()), NOW);

    for earlier in [NOW - 1, NOW - 400 * DAY, i64::MIN] {
        assert!(!cache.effective(earlier).grants(&Capability::Push));
        assert!(cache.should_refresh(earlier));
        assert!(!cache.lapses_soon(earlier));
    }
    assert!(
        cache.effective(NOW).grants(&Capability::Push),
        "the moment it was taken"
    );

    let restored = Cache::restore(Some(Stored {
        answer: paid(),
        fetched_at: NOW + 400 * DAY,
    }));
    assert!(!restored.effective(NOW).grants(&Capability::Push));
    assert!(restored.should_refresh(NOW));
}
