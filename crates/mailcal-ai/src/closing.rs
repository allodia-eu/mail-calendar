//! Keeping a drafted reply from closing twice. When the composer adds a signature, the reply stops
//! at its last sentence (the instructions say so); a model that closes anyway would put its own
//! sign-off above the signature's, so a last paragraph that repeats the signature's opening, is
//! made only of its lines, or is one of the person's own sign-offs on a line of its own, is taken
//! off here.

use crate::Habit;

/// The most words a sign-off on a line of its own is taken off at; a longer line is a sentence.
const SIGN_OFF_WORDS: usize = 6;

/// `reply` without a closing paragraph that the signature already carries or that is one of
/// `sign_offs`. A reply of one paragraph is left whole, and so is one when there is no signature.
pub(crate) fn without_closing(reply: &str, signature: Option<&str>, sign_offs: &[Habit]) -> String {
    let signature_lines: Vec<String> = signature
        .unwrap_or_default()
        .lines()
        .map(normalised)
        .filter(|line| !line.is_empty() && line != "--")
        .collect();
    let Some(opening) = signature_lines.first() else {
        return reply.to_owned();
    };
    let trimmed = reply.trim_end();
    let Some(split) = trimmed.rfind("\n\n") else {
        return reply.to_owned();
    };
    let (body, last) = (&trimmed[..split], &trimmed[split..]);
    let lines: Vec<String> = last
        .lines()
        .map(normalised)
        .filter(|line| !line.is_empty())
        .collect();
    let repeats_opening = lines.first() == Some(opening);
    let only_signature =
        !lines.is_empty() && lines.iter().all(|line| signature_lines.contains(line));
    let own_sign_off = match lines.as_slice() {
        [line] => {
            line.split_whitespace().count() <= SIGN_OFF_WORDS
                && sign_offs
                    .iter()
                    .any(|sign_off| normalised(&sign_off.text) == *line)
        }
        _ => false,
    };
    if repeats_opening || only_signature || own_sign_off {
        body.trim_end().to_owned()
    } else {
        reply.to_owned()
    }
}

/// A line as compared: trimmed, in lower case, runs of spaces as one, trailing punctuation off.
fn normalised(line: &str) -> String {
    line.split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase()
        .trim_end_matches([',', '.', '!', ';', ':'])
        .to_owned()
}

#[cfg(test)]
#[path = "closing_tests.rs"]
mod tests;
