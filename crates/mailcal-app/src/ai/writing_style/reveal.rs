//! A style shown back (`docs/ai.md` rules 15 and 16): the reveal's six pages and the detail's
//! sections, composed from what is stored so a client only lays them out. The rules each page
//! follows are `mailcal_viewmodel::writing_style`'s.

use engine_api::Provider;
use mailcal_account::{StoredWritingStyle, WritingStyleId};
use mailcal_ai::{LanguageStyle, StyleGuide};
use mailcal_viewmodel::writing_style::{
    RevealCard, RevealHabits, RevealLanguage, RevealLetter, RevealPage, RevealPhrases,
    RevealSection, RevealTopic, card_text, habit_rows, letter_lines, reveal_pages, reveal_runs,
    reveal_sections,
};

use super::{StyleControl, WritingStyleChangeKind, guide_of};
use crate::App;

/// What a style was learned from: the reveal's first page, and the facts of a row and a detail.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StyleFacts {
    /// How many messages, in all.
    pub messages: u32,
    /// The address it was learned from.
    pub learned_from: String,
    /// The oldest message read, in seconds since the Unix epoch.
    pub since: Option<i64>,
    /// When it was learned, in seconds since the Unix epoch.
    pub learned_at: Option<i64>,
    /// Its languages as ISO 639-1 codes, most messages first.
    pub languages: Vec<String>,
}

/// The reveal of a new style.
#[derive(Debug, Clone, PartialEq)]
pub struct WritingStyleRevealView {
    /// The style.
    pub id: WritingStyleId,
    /// The first page.
    pub facts: StyleFacts,
    /// The pages, in order, a page no language has anything for left out.
    pub pages: Vec<RevealPage>,
    /// The per-language pages' content, most messages first. A client draws a language control
    /// when there is more than one.
    pub languages: Vec<RevealLanguage>,
    /// The name, for the last page.
    pub name: String,
    /// The notes, for the last page.
    pub notes: String,
    /// Save, on the last page.
    pub save: StyleControl,
    /// Leaving without saving, which counts the reveal as shown.
    pub close: StyleControl,
    /// Moving between the pages: held while a change to the library runs, never running.
    pub paging: StyleControl,
}

/// An existing style's detail.
#[derive(Debug, Clone, PartialEq)]
pub struct WritingStyleDetailView {
    /// The style.
    pub id: WritingStyleId,
    /// What it was learned from.
    pub facts: StyleFacts,
    /// The sections it has, in order, by the rule the reveal leaves a page out by.
    pub sections: Vec<RevealSection>,
    /// The reveal's sections, per language, as one page.
    pub languages: Vec<RevealLanguage>,
    /// The name.
    pub name: String,
    /// The notes.
    pub notes: String,
    /// The name's Save. A client holds it besides while `writing_style_name` refuses the name.
    pub rename: StyleControl,
    /// The notes field, sent on Return and on leaving it.
    pub notes_control: StyleControl,
    /// Forget, asked first.
    pub forget: StyleControl,
    /// The addresses of the accounts that draft in the style, which the question names.
    pub drafting_accounts: Vec<String>,
}

/// A style's facts.
pub(super) fn facts(style: &StoredWritingStyle, guide: &StyleGuide) -> StyleFacts {
    let learned = guide.learned.as_ref();
    StyleFacts {
        messages: learned.map_or(0, |learned| learned.messages_per_language.values().sum()),
        learned_from: style.learned_from.clone(),
        since: learned.and_then(|learned| learned.oldest),
        learned_at: learned
            .map(|learned| learned.learned_at)
            .filter(|at| *at > 0),
        languages: languages(guide)
            .into_iter()
            .map(|(code, _)| code.to_owned())
            .collect(),
    }
}

/// The guide's languages, most messages first.
fn languages(guide: &StyleGuide) -> Vec<(&str, &LanguageStyle)> {
    let count = |code: &str| {
        guide.learned.as_ref().map_or(0, |learned| {
            learned
                .messages_per_language
                .iter()
                .find(|(language, _)| language.as_str() == code)
                .map_or(0, |(_, count)| *count)
        })
    };
    let mut languages: Vec<(&str, &LanguageStyle)> = guide
        .languages
        .iter()
        .map(|(code, style)| (code.as_str(), style))
        .collect();
    languages.sort_by_key(|(code, _)| std::cmp::Reverse(count(code)));
    languages
}

/// One language's sections, each left out when it has nothing in it.
fn language(code: &str, style: &LanguageStyle) -> RevealLanguage {
    let first_run = |habits: &[mailcal_ai::Habit]| {
        habits
            .first()
            .map(|habit| reveal_runs(&habit.text))
            .unwrap_or_default()
    };
    let has_habits =
        !style.greetings.is_empty() || !style.sign_offs.is_empty() || !style.signs_as.is_empty();
    let known_shape =
        style.typical_words > 0 || style.typical_paragraphs > 0 || !style.shape.is_empty();
    let cards = [
        (
            RevealTopic::Register,
            &style.register_headline,
            &style.register,
        ),
        (
            RevealTopic::Structure,
            &style.structure_headline,
            &style.structure,
        ),
        (RevealTopic::Moves, &style.moves_headline, &style.moves),
        (RevealTopic::Punctuation, &String::new(), &style.punctuation),
    ]
    .into_iter()
    .filter_map(|(topic, headline, text)| {
        card_text(headline, text).map(|(heading, body)| RevealCard {
            topic,
            heading,
            body,
        })
    })
    .collect();
    let habit = |habits: &[mailcal_ai::Habit]| {
        habit_rows(habits.iter().map(|habit| (habit.text.clone(), habit.share)))
    };
    RevealLanguage {
        language: code.to_owned(),
        letter: (known_shape || has_habits).then(|| RevealLetter {
            opening: first_run(&style.greetings),
            paragraphs: letter_lines(style.typical_words, style.typical_paragraphs),
            closing: first_run(&style.sign_offs),
            signs_as: style.signs_as.clone(),
            words: style.typical_words,
            shape: style.shape.clone(),
        }),
        habits: has_habits.then(|| RevealHabits {
            greetings: habit(&style.greetings),
            sign_offs: habit(&style.sign_offs),
            signs_as: style.signs_as.clone(),
        }),
        cards,
        phrases: (!style.phrases.is_empty() || !style.avoid.is_empty()).then(|| RevealPhrases {
            used: style.phrases.clone(),
            avoided: style.avoid.clone(),
        }),
    }
}

/// Every language's sections, most messages first.
pub(super) fn sections(guide: &StyleGuide) -> Vec<RevealLanguage> {
    languages(guide)
        .into_iter()
        .map(|(code, style)| language(code, style))
        .collect()
}

impl<P: Provider> App<P> {
    /// The controls of a style's surfaces while the library's changes `work` run: all held, the
    /// one that started a change of `kind` on `id` running.
    fn style_controls(
        &self,
        id: &WritingStyleId,
    ) -> impl Fn(WritingStyleChangeKind) -> StyleControl {
        let (work, _) = self.style_work_view();
        let id = id.clone();
        move |kind| {
            StyleControl::of(
                !work.is_empty(),
                work.iter()
                    .any(|running| running.kind == kind && running.style.as_ref() == Some(&id)),
            )
        }
    }

    /// The reveal of the style `id`, or `None` when it names none or the assistant is not
    /// offered. Read from memory.
    #[must_use]
    pub fn writing_style_reveal(&self, id: &WritingStyleId) -> Option<WritingStyleRevealView> {
        if !self.ai_offer() {
            return None;
        }
        let style = self.writing_style(id)?;
        let guide = guide_of(&style.guide_json);
        let languages = sections(&guide);
        let control = self.style_controls(id);
        let paging = StyleControl::of(!self.style_work_view().0.is_empty(), false);
        Some(WritingStyleRevealView {
            id: id.clone(),
            facts: facts(&style, &guide),
            pages: reveal_pages(&languages),
            languages,
            name: style.name,
            notes: guide.notes,
            save: control(WritingStyleChangeKind::SaveReveal),
            close: control(WritingStyleChangeKind::CloseReveal),
            paging,
        })
    }

    /// The detail of the style `id`, or `None` when it names none or the assistant is not
    /// offered. Read from memory.
    #[must_use]
    pub fn writing_style_detail(&self, id: &WritingStyleId) -> Option<WritingStyleDetailView> {
        if !self.ai_offer() {
            return None;
        }
        let style = self.writing_style(id)?;
        let guide = guide_of(&style.guide_json);
        let languages = sections(&guide);
        let control = self.style_controls(id);
        let drafting_accounts = self
            .accounts
            .senders()
            .into_iter()
            .filter(|(account, _)| self.writing_style_slot(account.as_str()).as_ref() == Some(id))
            .map(|(_, address)| address)
            .collect();
        Some(WritingStyleDetailView {
            id: id.clone(),
            facts: facts(&style, &guide),
            sections: reveal_sections(&languages),
            languages,
            name: style.name,
            notes: guide.notes,
            rename: control(WritingStyleChangeKind::Rename),
            notes_control: control(WritingStyleChangeKind::Notes),
            forget: control(WritingStyleChangeKind::Forget),
            drafting_accounts,
        })
    }
}
