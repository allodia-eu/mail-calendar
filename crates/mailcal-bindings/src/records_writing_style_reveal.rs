//! A style shown back (`docs/ai.md` rules 15 and 16): the reveal's pages and the detail's
//! sections, composed by the core. A client lays them out and animates them, drawing every value
//! final under reduced motion, and decides nothing about what is on which page.

use mailcal_app::ai as core;
use mailcal_viewmodel::writing_style as vm;

use super::records::control;
use crate::ControlState;

/// What a style was learned from.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct WritingStyleFacts {
    /// How many messages, in all.
    pub messages: u32,
    /// The address it was learned from.
    pub learned_from: String,
    /// The oldest message read, seconds since the Unix epoch.
    pub since: Option<i64>,
    /// When it was learned, seconds since the Unix epoch.
    pub learned_at: Option<i64>,
    /// ISO 639-1 codes, most messages first.
    pub languages: Vec<String>,
}

/// The reveal of a new style (`MailcalApp::writing_style_reveal`, pulled after
/// `Surface::WritingStyle`). It counts as shown once left, by `SaveReveal` or `CloseReveal`.
#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct WritingStyleReveal {
    pub id: String,
    /// The first page.
    pub facts: WritingStyleFacts,
    /// The pages in order; one no language has anything for is left out.
    pub pages: Vec<WritingStyleRevealPage>,
    /// The per-language pages' content, most messages first. A client draws a language control
    /// on a per-language page when there is more than one, and for a language with nothing for
    /// that page says so.
    pub languages: Vec<WritingStyleLanguage>,
    pub name: String,
    pub notes: String,
    /// Save, on the last page. A client holds it besides while `writing_style_name_refusal`
    /// refuses the name typed.
    pub save: ControlState,
    /// Leaving the reveal without saving.
    pub close: ControlState,
}

/// An existing style's detail (`MailcalApp::writing_style_detail`).
#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct WritingStyleDetail {
    pub id: String,
    pub facts: WritingStyleFacts,
    /// The reveal's sections, per language, as one page without paging.
    pub languages: Vec<WritingStyleLanguage>,
    pub name: String,
    pub notes: String,
    /// The name's Save (`WritingStyleIntent::Rename`), held besides while
    /// `writing_style_name_refusal` refuses the name typed.
    pub rename: ControlState,
    /// The notes field (`WritingStyleIntent::Notes`), sent on Return and on leaving it.
    pub notes_control: ControlState,
    /// Forget (`WritingStyleIntent::Forget`), asked first.
    pub forget: ControlState,
    /// The addresses of the accounts that draft in the style, which Forget's question names.
    pub drafting_accounts: Vec<String>,
}

/// A page of the reveal.
#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum WritingStyleRevealPage {
    /// What was read: `facts`.
    Read,
    /// A typical reply: each language's `letter`.
    Letter,
    /// Greetings and sign-offs: each language's `habits`.
    Habits,
    /// Tone and approach: each language's `cards`.
    Voice,
    /// Phrases used and avoided: each language's `phrases`.
    Phrases,
    /// The name and the notes, with Save.
    Name,
}

/// One language's sections; a section with nothing in it is `None`, or no cards.
#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct WritingStyleLanguage {
    /// ISO 639-1 code.
    pub language: String,
    pub letter: Option<WritingStyleLetter>,
    pub habits: Option<WritingStyleHabits>,
    pub cards: Vec<WritingStyleCard>,
    pub phrases: Option<WritingStylePhrases>,
}

/// The miniature letter: the person's usual length and shape, opening as they often do.
#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct WritingStyleLetter {
    /// The greeting most messages open with; empty when none is known.
    pub opening: Vec<WritingStyleSpan>,
    /// The grey lines, per paragraph.
    pub paragraphs: Vec<WritingStyleParagraph>,
    /// The sign-off most messages close with; empty when none is known.
    pub closing: Vec<WritingStyleSpan>,
    /// The name signed with; empty when they do not sign.
    pub signs_as: String,
    /// The typical length in words; zero when unknown.
    pub words: u32,
    /// How they paragraph and how long their sentences run; empty when nothing was said.
    pub shape: String,
}

/// One paragraph of the miniature letter.
#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct WritingStyleParagraph {
    /// Each line's width as a share of the letter's, from 0 to 1.
    pub lines: Vec<f64>,
}

/// A run of a greeting or sign-off: words, or a placeholder such as `[Name]` without its
/// brackets, drawn as a pill.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct WritingStyleSpan {
    pub text: String,
    pub placeholder: bool,
}

/// Greetings and sign-offs, most used first.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct WritingStyleHabits {
    pub greetings: Vec<WritingStyleHabit>,
    pub sign_offs: Vec<WritingStyleHabit>,
    /// The name signed with; empty when they do not sign.
    pub signs_as: String,
}

/// One greeting or sign-off.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct WritingStyleHabit {
    pub runs: Vec<WritingStyleSpan>,
    /// Roughly how often, as a percentage of messages.
    pub share: u8,
    /// The bar's length, as a percentage of the list's shares together.
    pub relative: u8,
    /// The word beside the bar.
    pub frequency: WritingStyleFrequency,
}

/// How often a greeting or sign-off is used.
#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum WritingStyleFrequency {
    /// Half the messages or more.
    Mostly,
    /// A fifth or more.
    Often,
    /// Less.
    Sometimes,
}

/// What a card is about; the client names it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum WritingStyleTopic {
    Register,
    Structure,
    Moves,
    Punctuation,
}

/// One card of tone and approach: a heading over the text beneath it, never the same sentence
/// twice.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct WritingStyleCard {
    pub topic: WritingStyleTopic,
    pub heading: String,
    /// Empty when the heading says it all.
    pub body: String,
}

/// Phrases used and avoided.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct WritingStylePhrases {
    pub used: Vec<String>,
    pub avoided: Vec<String>,
}

impl From<core::StyleFacts> for WritingStyleFacts {
    fn from(facts: core::StyleFacts) -> Self {
        Self {
            messages: facts.messages,
            learned_from: facts.learned_from,
            since: facts.since,
            learned_at: facts.learned_at,
            languages: facts.languages,
        }
    }
}

fn runs(runs: Vec<vm::RevealRun>) -> Vec<WritingStyleSpan> {
    runs.into_iter()
        .map(|run| WritingStyleSpan {
            text: run.text,
            placeholder: run.placeholder,
        })
        .collect()
}

fn habits(habits: Vec<vm::RevealHabit>) -> Vec<WritingStyleHabit> {
    habits
        .into_iter()
        .map(|habit| WritingStyleHabit {
            runs: runs(habit.runs),
            share: habit.share,
            relative: habit.relative,
            frequency: match habit.frequency {
                vm::HabitFrequency::Mostly => WritingStyleFrequency::Mostly,
                vm::HabitFrequency::Often => WritingStyleFrequency::Often,
                vm::HabitFrequency::Sometimes => WritingStyleFrequency::Sometimes,
            },
        })
        .collect()
}

fn language(language: vm::RevealLanguage) -> WritingStyleLanguage {
    WritingStyleLanguage {
        language: language.language,
        letter: language.letter.map(|letter| WritingStyleLetter {
            opening: runs(letter.opening),
            paragraphs: letter
                .paragraphs
                .into_iter()
                .map(|lines| WritingStyleParagraph { lines })
                .collect(),
            closing: runs(letter.closing),
            signs_as: letter.signs_as,
            words: letter.words,
            shape: letter.shape,
        }),
        habits: language.habits.map(|found| WritingStyleHabits {
            greetings: habits(found.greetings),
            sign_offs: habits(found.sign_offs),
            signs_as: found.signs_as,
        }),
        cards: language
            .cards
            .into_iter()
            .map(|card| WritingStyleCard {
                topic: match card.topic {
                    vm::RevealTopic::Register => WritingStyleTopic::Register,
                    vm::RevealTopic::Structure => WritingStyleTopic::Structure,
                    vm::RevealTopic::Moves => WritingStyleTopic::Moves,
                    vm::RevealTopic::Punctuation => WritingStyleTopic::Punctuation,
                },
                heading: card.heading,
                body: card.body,
            })
            .collect(),
        phrases: language.phrases.map(|phrases| WritingStylePhrases {
            used: phrases.used,
            avoided: phrases.avoided,
        }),
    }
}

/// The reveal's record.
pub(crate) fn reveal(view: core::WritingStyleRevealView) -> WritingStyleReveal {
    WritingStyleReveal {
        id: view.id.as_str().to_owned(),
        facts: view.facts.into(),
        pages: view
            .pages
            .into_iter()
            .map(|page| match page {
                vm::RevealPage::Read => WritingStyleRevealPage::Read,
                vm::RevealPage::Letter => WritingStyleRevealPage::Letter,
                vm::RevealPage::Habits => WritingStyleRevealPage::Habits,
                vm::RevealPage::Voice => WritingStyleRevealPage::Voice,
                vm::RevealPage::Phrases => WritingStyleRevealPage::Phrases,
                vm::RevealPage::Name => WritingStyleRevealPage::Name,
            })
            .collect(),
        languages: view.languages.into_iter().map(language).collect(),
        name: view.name,
        notes: view.notes,
        save: control(view.save),
        close: control(view.close),
    }
}

/// The detail's record.
pub(crate) fn detail(view: core::WritingStyleDetailView) -> WritingStyleDetail {
    WritingStyleDetail {
        id: view.id.as_str().to_owned(),
        facts: view.facts.into(),
        languages: view.languages.into_iter().map(language).collect(),
        name: view.name,
        notes: view.notes,
        rename: control(view.rename),
        notes_control: control(view.notes_control),
        forget: control(view.forget),
        drafting_accounts: view.drafting_accounts,
    }
}
