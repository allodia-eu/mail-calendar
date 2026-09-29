use super::{MESSAGE_CAP_TOKENS, estimate_tokens, within_budget};
use crate::corpus::CorpusMessage;

fn message(key: &str, recipient: &str, words: usize, sent_at: i64) -> CorpusMessage {
    CorpusMessage {
        key: key.to_owned(),
        text: "word ".repeat(words).trim_end().to_owned(),
        recipient: Some(recipient.to_owned()),
        sent_at: Some(sent_at),
    }
}

fn keys(sample: &[CorpusMessage]) -> Vec<&str> {
    sample.iter().map(|message| message.key.as_str()).collect()
}

#[test]
fn the_sample_never_exceeds_the_budget() {
    let candidates: Vec<_> = (0..200)
        .map(|index| message(&format!("m{index}"), &format!("r{}", index % 7), 60, index))
        .collect();
    let sample = within_budget(candidates, 2_000);
    let used: usize = sample
        .iter()
        .map(|message| estimate_tokens(&message.text))
        .sum();
    assert!(used <= 2_000, "{used}");
    assert!(!sample.is_empty());
}

/// One prolific correspondent must not crowd everyone else out of the sample.
#[test]
fn every_recipient_gets_a_turn_before_anyone_gets_a_second() {
    let mut candidates: Vec<_> = (0..50)
        .map(|index| message(&format!("boss{index}"), "boss", 80, 100 + index))
        .collect();
    candidates.push(message("sister", "sister", 50, 1));
    candidates.push(message("accountant", "accountant", 50, 2));

    // Room for four messages of about 100 tokens each.
    let sample = within_budget(candidates, 400);

    let sample = keys(&sample);
    assert!(sample.contains(&"sister"), "{sample:?}");
    assert!(sample.contains(&"accountant"), "{sample:?}");
    assert_eq!(
        sample.iter().filter(|key| key.starts_with("boss")).count(),
        2
    );
}

#[test]
fn within_a_recipient_the_longer_message_is_preferred() {
    let candidates = vec![
        message("short", "anna", 45, 3),
        message("long", "anna", 300, 1),
        message("medium", "anna", 120, 2),
    ];
    let one_message = estimate_tokens(&"word ".repeat(300));
    let sample = within_budget(candidates, one_message);
    assert_eq!(keys(&sample), ["long"]);
}

/// A message too big for the room left does not end the recipient's turn: a shorter one behind
/// it may still fit.
#[test]
fn a_message_that_does_not_fit_gives_way_to_a_shorter_one() {
    let candidates = vec![
        message("huge", "anna", 900, 1),
        message("small", "anna", 50, 2),
    ];
    let sample = within_budget(candidates, 100);
    assert_eq!(keys(&sample), ["small"]);
}

#[test]
fn the_sample_comes_back_oldest_first() {
    let candidates = vec![
        message("c", "x", 50, 30),
        message("a", "y", 50, 10),
        message("b", "z", 50, 20),
    ];
    assert_eq!(keys(&within_budget(candidates, 10_000)), ["a", "b", "c"]);
}

#[test]
fn a_very_long_message_is_cut_at_a_paragraph() {
    let paragraph = "word ".repeat(400);
    let text = format!("{paragraph}\n\n{paragraph}\n\n{paragraph}\n\n{paragraph}");
    let candidates = vec![CorpusMessage {
        key: "essay".to_owned(),
        text,
        recipient: None,
        sent_at: None,
    }];
    let sample = within_budget(candidates, 100_000);
    let kept = &sample[0].text;
    assert!(estimate_tokens(kept) <= MESSAGE_CAP_TOKENS);
    assert!(kept.ends_with("word"), "cut mid-paragraph");
}
