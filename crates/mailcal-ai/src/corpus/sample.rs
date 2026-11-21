//! Choosing which messages go into a language's sample.
//!
//! The budget is the constraint, and within it three things are wanted at once: **longer
//! messages** (a two-line reply says little about how someone writes), **many recipients**
//! (people write to their accountant and to their sister differently, and the guide has to
//! describe both) and **recent mail** (style drifts). So each recipient's messages queue longest
//! first, the queues are served in turn, most recently written-to first, and a message that
//! does not fit the room left is skipped for a shorter one behind it.

use std::collections::{BTreeMap, VecDeque};

use super::CorpusMessage;

/// The most one message may take of a sample, in tokens; a longer one is cut at a paragraph.
const MESSAGE_CAP_TOKENS: usize = 1_500;

/// Roughly how many tokens `text` is: four characters to a token, which holds for the
/// Latin-script languages the catalog ships.
pub(crate) fn estimate_tokens(text: &str) -> usize {
    text.chars().count().div_ceil(4)
}

/// The messages to send for one language, oldest first, at most `budget` tokens in all.
pub(super) fn within_budget(candidates: Vec<CorpusMessage>, budget: usize) -> Vec<CorpusMessage> {
    let mut queues: BTreeMap<String, Vec<CorpusMessage>> = BTreeMap::new();
    for mut message in candidates {
        message.text = capped(&message.text);
        queues
            .entry(message.recipient.clone().unwrap_or_default())
            .or_default()
            .push(message);
    }
    let mut queues: Vec<(Option<i64>, VecDeque<CorpusMessage>)> = queues
        .into_values()
        .map(|mut messages| {
            messages.sort_by(|a, b| {
                estimate_tokens(&b.text)
                    .cmp(&estimate_tokens(&a.text))
                    .then(b.sent_at.cmp(&a.sent_at))
                    .then(a.key.cmp(&b.key))
            });
            let newest = messages.iter().filter_map(|message| message.sent_at).max();
            (newest, messages.into())
        })
        .collect();
    queues.sort_by_key(|(newest, _)| std::cmp::Reverse(*newest));

    let mut chosen = Vec::new();
    let mut room = budget;
    while queues.iter().any(|(_, queue)| !queue.is_empty()) {
        for (_, queue) in &mut queues {
            // The longest message of this recipient's that still fits.
            if let Some(index) = queue
                .iter()
                .position(|message| estimate_tokens(&message.text) <= room)
            {
                let message = queue.remove(index).expect("the index was just found");
                room -= estimate_tokens(&message.text);
                chosen.push(message);
            } else {
                queue.clear();
            }
        }
    }
    chosen.sort_by(|a, b| a.sent_at.cmp(&b.sent_at).then(a.key.cmp(&b.key)));
    chosen
}

/// `text` cut to [`MESSAGE_CAP_TOKENS`], at the last paragraph break that keeps it under, or at
/// the cap itself when there is none.
fn capped(text: &str) -> String {
    let cap_chars = MESSAGE_CAP_TOKENS * 4;
    if text.chars().count() <= cap_chars {
        return text.to_owned();
    }
    let head: String = text.chars().take(cap_chars).collect();
    match head.rfind("\n\n") {
        Some(at) if at > 0 => head[..at].trim_end().to_owned(),
        _ => head,
    }
}

#[cfg(test)]
#[path = "sample_tests.rs"]
mod tests;
