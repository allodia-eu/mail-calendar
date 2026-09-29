//! What the person changed in a draft before sending it.
//!
//! The one signal that says how a style falls short of the person, and the reason a draft is
//! never learned from as it was written: a style learned from text a model produced converges on
//! the model. What is kept is only the difference, word by word: the runs the person added, the
//! runs they took out, and the share of words that changed. A draft sent unedited yields none of
//! the first two and a share of zero.

use std::fmt;

/// The longest text compared, in words; a longer one is cut before the comparison.
const MAX_WORDS: usize = 2_000;

/// The difference between a draft and what was sent.
#[derive(Clone, Default, PartialEq)]
pub struct Correction {
    /// Runs of words the person added, in order.
    pub added: Vec<String>,
    /// Runs of words the person took out, in order.
    pub removed: Vec<String>,
    /// The share of words that changed: zero for a draft sent as it was, one for one replaced
    /// entirely.
    pub changed: f32,
}

impl fmt::Debug for Correction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Correction")
            .field("added", &self.added.len())
            .field("removed", &self.removed.len())
            .field("changed", &self.changed)
            .finish()
    }
}

/// The word-level difference between `draft` and `sent`.
#[must_use]
pub fn correction(draft: &str, sent: &str) -> Correction {
    let before: Vec<&str> = draft.split_whitespace().take(MAX_WORDS).collect();
    let after: Vec<&str> = sent.split_whitespace().take(MAX_WORDS).collect();
    let steps = diff(&before, &after);

    let mut out = Correction::default();
    let mut added: Vec<&str> = Vec::new();
    let mut removed: Vec<&str> = Vec::new();
    let mut changed = 0_usize;
    for step in steps {
        match step {
            Step::Keep => {
                flush(&mut out.added, &mut added);
                flush(&mut out.removed, &mut removed);
            }
            Step::Add(word) => {
                added.push(word);
                changed += 1;
            }
            Step::Remove(word) => {
                removed.push(word);
                changed += 1;
            }
        }
    }
    flush(&mut out.added, &mut added);
    flush(&mut out.removed, &mut removed);
    let total = before.len() + after.len();
    #[allow(
        clippy::cast_precision_loss,
        reason = "a share of at most four thousand words"
    )]
    if total > 0 {
        out.changed = changed as f32 / total as f32;
    }
    out
}

fn flush(into: &mut Vec<String>, run: &mut Vec<&str>) {
    if !run.is_empty() {
        into.push(run.join(" "));
        run.clear();
    }
}

enum Step<'a> {
    Keep,
    Add(&'a str),
    Remove(&'a str),
}

/// The shortest edit from `before` to `after`, through their longest common subsequence.
fn diff<'a>(before: &[&'a str], after: &[&'a str]) -> Vec<Step<'a>> {
    let (rows, cols) = (before.len(), after.len());
    // lengths[i][j]: the longest common subsequence of before[i..] and after[j..].
    let mut lengths = vec![vec![0_u16; cols + 1]; rows + 1];
    for i in (0..rows).rev() {
        for j in (0..cols).rev() {
            lengths[i][j] = if before[i] == after[j] {
                lengths[i + 1][j + 1] + 1
            } else {
                lengths[i + 1][j].max(lengths[i][j + 1])
            };
        }
    }
    let mut steps = Vec::with_capacity(rows + cols);
    let (mut i, mut j) = (0, 0);
    while i < rows || j < cols {
        if i < rows && j < cols && before[i] == after[j] {
            steps.push(Step::Keep);
            i += 1;
            j += 1;
        } else if j < cols && (i == rows || lengths[i][j + 1] >= lengths[i + 1][j]) {
            steps.push(Step::Add(after[j]));
            j += 1;
        } else {
            steps.push(Step::Remove(before[i]));
            i += 1;
        }
    }
    steps
}

#[cfg(test)]
mod tests {
    use super::correction;

    const DRAFT: &str = "Hi Anna,\n\nFriday works for me. See you at [time].\n\nBest,\nSam";

    #[test]
    fn a_draft_sent_as_it_was_carries_no_signal() {
        let unchanged = correction(DRAFT, DRAFT);
        assert!(unchanged.added.is_empty());
        assert!(unchanged.removed.is_empty());
        assert!(unchanged.changed.abs() < f32::EPSILON);
        // Line breaks and spacing are not edits.
        let reflowed = correction(DRAFT, &DRAFT.replace('\n', " "));
        assert!(reflowed.added.is_empty() && reflowed.removed.is_empty());
    }

    #[test]
    fn an_edited_draft_yields_the_edited_runs_only() {
        let sent =
            "Hoi Anna,\n\nFriday works for me. See you at ten, in the usual place.\n\nBest,\nSam";
        let edit = correction(DRAFT, sent);
        assert_eq!(edit.added, ["Hoi", "ten, in the usual place."]);
        assert_eq!(edit.removed, ["Hi", "[time]."]);
        assert!(edit.changed > 0.0 && edit.changed < 0.5);
    }

    #[test]
    fn a_draft_replaced_entirely_changed_everything() {
        let edit = correction("one two three", "four five");
        assert!((edit.changed - 1.0).abs() < f32::EPSILON);
        assert_eq!(edit.added, ["four five"]);
        assert_eq!(edit.removed, ["one two three"]);
    }

    #[test]
    fn nothing_against_nothing_is_no_change() {
        assert!(correction("", "").changed.abs() < f32::EPSILON);
    }

    #[test]
    fn a_correction_prints_no_words() {
        let printed = format!("{:?}", correction("secret one", "secret two"));
        assert!(!printed.contains("secret"));
    }
}
