//! Which of the catalog's languages a text is written in.
//!
//! A stopword count, not a model: sent mail is prose of forty words or more by the time it is
//! asked, and at that length the commonest function words separate these seven languages
//! reliably. A word several languages share counts for less, so `de` (Dutch and Spanish) or `que`
//! (Spanish, French, Portuguese) cannot outvote a word only one of them uses. And the winner's
//! function words must make up a fair share of the text: Danish shares `er`, `om` and `kan` with
//! Dutch, but in Danish they are a few words in ten, where in Dutch they are nearer half.
//!
//! Every catalog locale needs a list here; a test fails when the catalog gains one that has none,
//! and a language outside the lists is reported as not detected rather than guessed.

use std::collections::BTreeMap;

/// The commonest function words of each language, lower case.
const STOPWORDS: [(&str, &[&str]); 7] = [
    (
        "en",
        &[
            "the", "and", "to", "of", "i", "you", "is", "that", "it", "for", "we", "on", "with",
            "have", "be", "this", "are", "will", "not", "can", "would", "your", "my", "me", "if",
            "but", "just", "all", "thanks", "please", "was", "what", "from", "our", "about", "let",
            "know", "could", "they", "there",
        ],
    ),
    (
        "nl",
        &[
            "de", "het", "een", "en", "van", "ik", "je", "jij", "u", "dat", "niet", "is", "op",
            "te", "met", "voor", "zijn", "er", "maar", "ook", "als", "wat", "bij", "nog", "naar",
            "dan", "wel", "kan", "heb", "we", "wij", "ons", "mijn", "graag", "dank", "groet",
            "even", "zou", "deze", "dit", "hoe", "om",
        ],
    ),
    (
        "de",
        &[
            "der", "die", "das", "und", "ich", "sie", "nicht", "ist", "zu", "den", "mit", "von",
            "für", "auf", "es", "ein", "eine", "dem", "wir", "sich", "auch", "bei", "noch", "aber",
            "wie", "wenn", "bitte", "danke", "gerne", "habe", "haben", "kann", "werden", "uns",
            "mir", "dass", "ihnen", "grüße", "viele",
        ],
    ),
    (
        "fr",
        &[
            "le",
            "la",
            "les",
            "de",
            "des",
            "et",
            "je",
            "vous",
            "est",
            "que",
            "qui",
            "pas",
            "pour",
            "dans",
            "en",
            "un",
            "une",
            "du",
            "au",
            "nous",
            "sur",
            "avec",
            "ce",
            "cette",
            "mais",
            "ou",
            "bien",
            "merci",
            "bonjour",
            "cordialement",
            "avez",
            "suis",
            "sont",
            "votre",
            "vos",
            "mon",
            "ne",
            "très",
        ],
    ),
    (
        "es",
        &[
            "el", "la", "los", "las", "de", "y", "que", "en", "un", "una", "es", "no", "por",
            "para", "con", "se", "lo", "su", "al", "del", "pero", "como", "me", "te", "muy",
            "gracias", "hola", "saludos", "está", "estoy", "tengo", "usted", "nos", "esta", "este",
            "puedes", "también",
        ],
    ),
    (
        "it",
        &[
            "il", "lo", "la", "i", "gli", "le", "di", "e", "che", "è", "non", "per", "un", "una",
            "in", "con", "su", "mi", "ti", "ci", "ma", "come", "sono", "grazie", "ciao", "saluti",
            "del", "della", "anche", "questo", "questa", "ho", "hai", "abbiamo", "può", "molto",
        ],
    ),
    (
        "pt",
        &[
            "o",
            "a",
            "os",
            "as",
            "de",
            "e",
            "que",
            "não",
            "um",
            "uma",
            "é",
            "para",
            "com",
            "em",
            "do",
            "da",
            "no",
            "na",
            "por",
            "se",
            "mas",
            "como",
            "obrigado",
            "obrigada",
            "olá",
            "cumprimentos",
            "você",
            "estou",
            "tenho",
            "isso",
            "este",
            "esta",
            "muito",
            "nos",
            "ao",
            "também",
        ],
    ),
];

/// The fewest weighted hits a text needs before any language is named.
const MIN_SCORE: f64 = 3.0;
/// How far ahead of the runner-up the winner must be.
const MIN_LEAD: f64 = 1.3;
/// The smallest share of the text's words the winner's function words must make up.
const MIN_COVERAGE: f64 = 0.2;

/// The ISO 639-1 code of the language `text` is written in, or `None` when it is too short, in
/// none of the catalog's languages, or too close to call.
#[must_use]
pub fn detect(text: &str) -> Option<&'static str> {
    let mut sharing: BTreeMap<&str, f64> = BTreeMap::new();
    for (_, words) in STOPWORDS {
        for word in words {
            *sharing.entry(word).or_default() += 1.0;
        }
    }
    let mut scores = [0.0_f64; STOPWORDS.len()];
    let mut hits = [0_u32; STOPWORDS.len()];
    let mut total = 0_u32;
    for word in text
        .split(|ch: char| !ch.is_alphabetic())
        .filter(|word| !word.is_empty())
    {
        total += 1;
        let word = word.to_lowercase();
        let Some(shared) = sharing.get(word.as_str()) else {
            continue;
        };
        for (index, (_, words)) in STOPWORDS.iter().enumerate() {
            if words.contains(&word.as_str()) {
                scores[index] += 1.0 / shared;
                hits[index] += 1;
            }
        }
    }
    let mut ranked: Vec<(usize, f64)> = scores.into_iter().enumerate().collect();
    ranked.sort_by(|a, b| b.1.total_cmp(&a.1));
    let (best, score) = ranked[0];
    let runner_up = ranked[1].1;
    let coverage = f64::from(hits[best]) / f64::from(total.max(1));
    (score >= MIN_SCORE && score >= runner_up * MIN_LEAD && coverage >= MIN_COVERAGE)
        .then_some(STOPWORDS[best].0)
}

/// The languages [`detect`] can name.
pub fn detectable() -> impl Iterator<Item = &'static str> {
    STOPWORDS.iter().map(|(language, _)| *language)
}

#[cfg(test)]
#[path = "language_tests.rs"]
mod tests;
