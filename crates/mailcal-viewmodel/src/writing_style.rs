//! A writing style shown back to the person (`docs/ai.md` rules 15 and 16): the reveal's pages and
//! the detail's sections, composed here so a client only lays them out and animates them.
//!
//! The functions are the rules the four clients used to restate apiece: which share of messages
//! is "mostly", where the first sentence of a description ends, how many grey lines the miniature
//! letter draws, and where a placeholder such as `[Name]` sits in a greeting. The core builds the
//! records below with them from a learned style; nothing here reads a style itself.

/// How often a greeting or sign-off is used, read from its share of messages.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HabitFrequency {
    /// Half the messages or more.
    Mostly,
    /// A fifth or more.
    Often,
    /// Less than that, or not known.
    Sometimes,
}

impl HabitFrequency {
    /// The word for `share` percent of messages.
    #[must_use]
    pub const fn of(share: u8) -> Self {
        match share {
            50.. => Self::Mostly,
            20..=49 => Self::Often,
            _ => Self::Sometimes,
        }
    }
}

/// One run of a greeting or a sign-off: words, or a placeholder such as `[Name]` without its
/// brackets, which a client draws as a pill because there is no name to put in its place.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RevealRun {
    /// The words, or the placeholder's inside.
    pub text: String,
    /// Whether this run is a placeholder.
    pub placeholder: bool,
}

/// One greeting or sign-off with how often it is used.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RevealHabit {
    /// The wording, cut into runs.
    pub runs: Vec<RevealRun>,
    /// Roughly how often, as a percentage of messages.
    pub share: u8,
    /// Its part of its list, as a percentage of the list's shares together: a bar's length. A
    /// model's shares are rough and need not add up, so a bar is drawn from this.
    pub relative: u8,
    /// The word beside the bar.
    pub frequency: HabitFrequency,
}

/// The miniature letter: the person's usual length and shape, opening as they often do.
#[derive(Debug, Clone, PartialEq)]
pub struct RevealLetter {
    /// The greeting most messages open with; empty when none is known.
    pub opening: Vec<RevealRun>,
    /// One entry per paragraph, each line's width as a share of the letter's.
    pub paragraphs: Vec<Vec<f64>>,
    /// The sign-off most messages close with; empty when none is known.
    pub closing: Vec<RevealRun>,
    /// The name they sign with; empty when they do not sign.
    pub signs_as: String,
    /// The typical length in words; zero when unknown.
    pub words: u32,
    /// How they paragraph and how long their sentences run; empty when nothing was said.
    pub shape: String,
}

/// Greetings and sign-offs, most used first.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RevealHabits {
    /// How they open a message.
    pub greetings: Vec<RevealHabit>,
    /// How they close one.
    pub sign_offs: Vec<RevealHabit>,
    /// The name they sign with; empty when they do not sign.
    pub signs_as: String,
}

/// What a card on the tone and approach page is about.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RevealTopic {
    /// Register, and how it shifts with the recipient.
    Register,
    /// How a reply is built.
    Structure,
    /// How they decline, chase, apologise and confirm.
    Moves,
    /// Punctuation, capitalisation and emoji.
    Punctuation,
}

/// One card: a heading over the text beneath it, never the same sentence twice.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RevealCard {
    /// What it is about, which the client names.
    pub topic: RevealTopic,
    /// The heading.
    pub heading: String,
    /// The text beneath the heading; empty when the heading says it all.
    pub body: String,
}

/// Phrases used and avoided.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RevealPhrases {
    /// Short words and constructions characteristic of the person.
    pub used: Vec<String>,
    /// What they avoid.
    pub avoided: Vec<String>,
}

/// One language's sections. A section with nothing in it is `None` (or empty, for the cards),
/// and a client draws none for it.
#[derive(Debug, Clone, PartialEq)]
pub struct RevealLanguage {
    /// ISO 639-1 code.
    pub language: String,
    /// A typical reply.
    pub letter: Option<RevealLetter>,
    /// Greetings and sign-offs.
    pub habits: Option<RevealHabits>,
    /// Tone and approach, in the order the page shows them.
    pub cards: Vec<RevealCard>,
    /// Phrases.
    pub phrases: Option<RevealPhrases>,
}

/// A page of the reveal.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RevealPage {
    /// What was read: how many messages, from which address, since when, in which languages.
    Read,
    /// A typical reply.
    Letter,
    /// Greetings and sign-offs.
    Habits,
    /// Tone and approach.
    Voice,
    /// Phrases used and avoided.
    Phrases,
    /// The name and the notes, with Save.
    Name,
}

impl RevealPage {
    /// Whether the page shows one language at a time, under a language control.
    #[must_use]
    pub const fn is_per_language(self) -> bool {
        matches!(
            self,
            Self::Letter | Self::Habits | Self::Voice | Self::Phrases
        )
    }
}

/// A section of a style about one language at a time: a page of the reveal, a part of the detail.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RevealSection {
    /// A typical reply.
    Letter,
    /// Greetings and sign-offs.
    Habits,
    /// Tone and approach.
    Voice,
    /// Phrases used and avoided.
    Phrases,
}

impl From<RevealSection> for RevealPage {
    fn from(section: RevealSection) -> Self {
        match section {
            RevealSection::Letter => Self::Letter,
            RevealSection::Habits => Self::Habits,
            RevealSection::Voice => Self::Voice,
            RevealSection::Phrases => Self::Phrases,
        }
    }
}

/// The sections a style has, in order: one is left out when no language has anything for it. The
/// reveal's pages and the detail's parts both follow it.
#[must_use]
pub fn reveal_sections(languages: &[RevealLanguage]) -> Vec<RevealSection> {
    let any = |has: fn(&RevealLanguage) -> bool| languages.iter().any(has);
    [
        (
            RevealSection::Letter,
            any(|language| language.letter.is_some()),
        ),
        (
            RevealSection::Habits,
            any(|language| language.habits.is_some()),
        ),
        (
            RevealSection::Voice,
            any(|language| !language.cards.is_empty()),
        ),
        (
            RevealSection::Phrases,
            any(|language| language.phrases.is_some()),
        ),
    ]
    .into_iter()
    .filter_map(|(section, has)| has.then_some(section))
    .collect()
}

/// The reveal's pages, in order: what was read, the sections ([`reveal_sections`]), then the name.
#[must_use]
pub fn reveal_pages(languages: &[RevealLanguage]) -> Vec<RevealPage> {
    let mut pages = vec![RevealPage::Read];
    pages.extend(reveal_sections(languages).into_iter().map(RevealPage::from));
    pages.push(RevealPage::Name);
    pages
}

/// A list of `(wording, share)` as the reveal draws it: each with its part of the list and its
/// frequency word. A list whose shares are all unknown is drawn in equal parts.
#[must_use]
pub fn habit_rows(habits: impl IntoIterator<Item = (String, u8)>) -> Vec<RevealHabit> {
    let habits: Vec<(String, u8)> = habits.into_iter().collect();
    let total: u32 = habits.iter().map(|(_, share)| u32::from(*share)).sum();
    let count = u32::try_from(habits.len()).unwrap_or(u32::MAX).max(1);
    habits
        .into_iter()
        .map(|(text, share)| {
            let part = (u32::from(share) * 100 + total / 2)
                .checked_div(total)
                .unwrap_or(100 / count);
            RevealHabit {
                runs: reveal_runs(&text),
                share,
                relative: u8::try_from(part.min(100)).unwrap_or(100),
                frequency: HabitFrequency::of(share),
            }
        })
        .collect()
}

/// A card's heading and the text beneath it, or `None` when there is nothing to say. A heading
/// given goes over the whole description; without one, the description's first sentence is the
/// heading, without its full stop, and the rest stays beneath, so no sentence is shown twice.
#[must_use]
pub fn card_text(headline: &str, description: &str) -> Option<(String, String)> {
    let heading = headline.trim();
    let text = description.trim();
    if !heading.is_empty() {
        return Some((heading.to_owned(), text.to_owned()));
    }
    if text.is_empty() {
        return None;
    }
    let end = first_sentence_end(text);
    let sentence = without_full_stop(text[..end].trim());
    Some((sentence.to_owned(), text[end..].trim().to_owned()))
}

/// `sentence` without the full stop that ends it, a doubled one included. Three points or more
/// are an ellipsis and stay, as do a question or exclamation mark, and so does one point after a
/// word that holds a point of its own, since it closes an abbreviation such as "e.g.".
fn without_full_stop(sentence: &str) -> &str {
    let stem = sentence.trim_end_matches('.');
    let points = sentence.len() - stem.len();
    if points == 0 || points >= 3 {
        return sentence;
    }
    let last_word = stem.rsplit(char::is_whitespace).next().unwrap_or(stem);
    if last_word.contains('.') {
        &sentence[..=stem.len()]
    } else {
        stem.trim_end()
    }
}

/// Where the first sentence of `text` ends: after a full stop, question mark, exclamation mark or
/// ellipsis that ends the text or is followed by a space and then anything but a lower-case
/// letter. So the point in "3.5" ends nothing, and neither does the one in "e.g. a reply".
#[must_use]
pub fn first_sentence_end(text: &str) -> usize {
    for (index, character) in text.char_indices() {
        if !matches!(character, '.' | '!' | '?' | '…') {
            continue;
        }
        let end = index + character.len_utf8();
        let rest = &text[end..];
        if rest.is_empty() {
            return end;
        }
        if !rest.starts_with(char::is_whitespace) {
            continue;
        }
        if rest
            .trim_start()
            .chars()
            .next()
            .is_none_or(|next| !next.is_lowercase())
        {
            return end;
        }
    }
    text.len()
}

/// The miniature letter's grey lines: one list per paragraph, each line's width as a share of the
/// letter's. About fifteen words to a line, two paragraphs when the style does not say, at most
/// six paragraphs and twelve lines, so a long typical reply still reads as a letter. Each
/// paragraph ends on a short line.
#[must_use]
pub fn letter_lines(words: u32, paragraphs: u32) -> Vec<Vec<f64>> {
    const FULL: [f64; 3] = [1.0, 0.96, 0.98];
    const LAST: [f64; 4] = [0.58, 0.74, 0.44, 0.66];
    let count = if paragraphs == 0 {
        2
    } else {
        usize::try_from(paragraphs.min(6)).unwrap_or(6)
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

/// `text` cut into runs, each placeholder apart from the words around it and without its
/// brackets. Empty brackets, and a bracket inside another, are words.
#[must_use]
pub fn reveal_runs(text: &str) -> Vec<RevealRun> {
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

#[cfg(test)]
#[path = "writing_style_tests.rs"]
mod tests;
