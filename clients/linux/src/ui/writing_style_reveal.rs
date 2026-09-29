//! The reveal's steps and what its pictures are drawn from, decided without a widget (`docs/ai.md`,
//! "Learning" step 5).
//!
//! Which pages are about one language, where a stepped sheet stands, how many grey lines a
//! miniature letter draws, what a card says when the core gave it no heading, and a greeting cut
//! into words and placeholders. The pages that draw them are the Writing style settings detail.

use mailcal_bindings::{AccountWritingStyleRow, HabitFrequency, WritingStyleDetail};

use super::{timestamps, writing_style::languages_text};
use crate::l10n;

/// The reveal's pages, in the order the sheet walks them.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum RevealStep {
    Read,
    Letter,
    Habits,
    Voice,
    Phrases,
    Name,
}

impl RevealStep {
    pub(crate) const ALL: [Self; 6] = [
        Self::Read,
        Self::Letter,
        Self::Habits,
        Self::Voice,
        Self::Phrases,
        Self::Name,
    ];

    /// Whether the page is about one language, which is what puts the language control on screen.
    pub(crate) const fn is_per_language(self) -> bool {
        matches!(
            self,
            Self::Letter | Self::Habits | Self::Voice | Self::Phrases
        )
    }

    /// The page's heading.
    pub(crate) fn title(self) -> &'static str {
        match self {
            Self::Read => l10n::reveal_title(),
            Self::Letter => l10n::reveal_step_letter(),
            Self::Habits => l10n::reveal_step_habits(),
            Self::Voice => l10n::reveal_step_voice(),
            Self::Phrases => l10n::reveal_phrases(),
            Self::Name => l10n::reveal_step_name(),
        }
    }
}

/// Where a stepped sheet stands: which of `count` pages is on screen.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct WizardPager {
    count: usize,
    index: usize,
}

impl WizardPager {
    pub(crate) fn new(count: usize) -> Self {
        Self {
            count: count.max(1),
            index: 0,
        }
    }

    pub(crate) const fn index(self) -> usize {
        self.index
    }

    pub(crate) const fn is_first(self) -> bool {
        self.index == 0
    }

    pub(crate) const fn is_last(self) -> bool {
        self.index + 1 == self.count
    }

    /// Moves to `target`, held to the pages there are.
    pub(crate) fn go(&mut self, target: usize) {
        self.index = target.min(self.count - 1);
    }

    pub(crate) fn next(&mut self) {
        self.go(self.index + 1);
    }

    pub(crate) fn back(&mut self) {
        self.go(self.index.saturating_sub(1));
    }

    /// "Step 2 of 6", what the page dots say to a screen reader.
    pub(crate) fn step_label(self) -> String {
        l10n::a11y_wizard_step(&(self.index + 1).to_string(), &self.count.to_string())
    }
}

/// Whether the language control is on screen: on a page about one language, when there are two
/// or more to choose between.
pub(crate) const fn shows_languages(step: RevealStep, languages: usize) -> bool {
    step.is_per_language() && languages > 1
}

/// The miniature letter's grey lines: one list per paragraph, each line's width as a share of the
/// letter's. About fifteen words to a line, two paragraphs when the style does not say, and never
/// more lines than the letter has room for, so a long typical reply still reads as a letter.
pub(crate) fn letter_lines(words: u32, paragraphs: u32) -> Vec<Vec<f64>> {
    const FULL: [f64; 3] = [1.0, 0.96, 0.98];
    const LAST: [f64; 4] = [0.58, 0.74, 0.44, 0.66];
    let count = if paragraphs == 0 {
        2
    } else {
        usize::try_from(paragraphs).map_or(6, |paragraphs| paragraphs.min(6))
    };
    // Fifteen is odd, so a whole number of words never falls on a half: this is rounding.
    let wanted = if words == 0 {
        count * 2
    } else {
        usize::try_from(words.saturating_add(7) / 15).unwrap_or(usize::MAX)
    };
    let lines = wanted.clamp(count, 12);
    (0..count)
        .map(|paragraph| {
            let length = lines / count + usize::from(paragraph < lines % count);
            (0..length)
                .map(|line| {
                    if line + 1 == length {
                        LAST[paragraph % LAST.len()]
                    } else {
                        FULL[line % FULL.len()]
                    }
                })
                .collect()
        })
        .collect()
}

/// A card's heading and the text beneath it. The core's heading goes over the whole description;
/// without one, the description's first sentence becomes the heading and the rest stays beneath,
/// so no sentence is shown twice.
pub(crate) fn card_text(headline: &str, description: &str) -> (String, String) {
    let heading = headline.trim();
    let text = description.trim();
    if !heading.is_empty() {
        return (heading.to_owned(), text.to_owned());
    }
    let end = first_sentence_end(text);
    let sentence = text[..end].trim();
    let sentence = sentence.strip_suffix('.').unwrap_or(sentence);
    (sentence.to_owned(), text[end..].trim().to_owned())
}

/// Where the first sentence ends: after a full stop, question or exclamation mark followed by a
/// space or the end of the text, so the point in "3.5" does not end one.
fn first_sentence_end(text: &str) -> usize {
    let mut characters = text.char_indices().peekable();
    while let Some((index, character)) = characters.next() {
        if matches!(character, '.' | '!' | '?' | '…')
            && characters
                .peek()
                .is_none_or(|(_, next)| next.is_whitespace())
        {
            return index + character.len_utf8();
        }
    }
    text.len()
}

/// One run of a greeting or sign-off: words, or a bracketed placeholder such as `[Name]`, which the
/// reveal draws as a pill because there is no name to put in its place.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct RevealRun {
    pub(crate) text: String,
    pub(crate) placeholder: bool,
}

/// `text` cut into runs, each placeholder apart from the words around it and without its brackets.
pub(crate) fn reveal_runs(text: &str) -> Vec<RevealRun> {
    let mut runs = Vec::new();
    let mut rest = text;
    while let Some((before, inside, after)) = next_placeholder(rest) {
        if !before.is_empty() {
            runs.push(RevealRun {
                text: before.to_owned(),
                placeholder: false,
            });
        }
        runs.push(RevealRun {
            text: inside.to_owned(),
            placeholder: true,
        });
        rest = after;
    }
    if !rest.is_empty() {
        runs.push(RevealRun {
            text: rest.to_owned(),
            placeholder: false,
        });
    }
    runs
}

/// The first `[…]` in `text` with something inside and no bracket within it: what precedes it,
/// what is inside, and what follows.
fn next_placeholder(text: &str) -> Option<(&str, &str, &str)> {
    let mut from = 0;
    loop {
        let open = from + text[from..].find('[')?;
        let inside = open + 1;
        let close = inside + text[inside..].find(['[', ']'])?;
        if close > inside && text[close..].starts_with(']') {
            return Some((&text[..open], &text[inside..close], &text[close + 1..]));
        }
        from = close;
    }
}

/// The word beside a greeting's or sign-off's bar. The core decides which, so every client agrees.
pub(crate) fn frequency_text(frequency: HabitFrequency) -> &'static str {
    match frequency {
        HabitFrequency::Mostly => l10n::reveal_frequency_mostly(),
        HabitFrequency::Often => l10n::reveal_frequency_often(),
        HabitFrequency::Sometimes => l10n::reveal_frequency_sometimes(),
    }
}

/// The address of the account the style was learned from, while that account is still here. A
/// style from another device names none.
pub(crate) fn source_address<'a>(
    source: &str,
    accounts: &'a [AccountWritingStyleRow],
) -> Option<&'a str> {
    if source.is_empty() {
        return None;
    }
    accounts
        .iter()
        .find(|account| account.account_id == source)
        .map(|account| account.email.as_str())
}

/// The first page's figures read as one sentence, when there is a date to say it with.
pub(crate) fn stats_label(detail: &WritingStyleDetail, zone: &str, locale: &str) -> Option<String> {
    let date = timestamps::local_day(detail.row.oldest?, zone, locale)?;
    let codes = detail
        .languages
        .iter()
        .map(|language| language.language.clone())
        .collect::<Vec<_>>();
    Some(l10n::reveal_stats_a11y(
        i64::from(detail.row.messages),
        &date,
        &languages_text(&codes),
    ))
}

#[cfg(test)]
#[path = "writing_style_reveal_tests.rs"]
mod tests;
