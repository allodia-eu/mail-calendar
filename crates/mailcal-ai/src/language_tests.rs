use super::{detect, detectable};
use crate::catalog::CATALOG;

#[test]
fn every_catalog_locale_can_be_detected() {
    let detectable: Vec<&str> = detectable().collect();
    for shapes in CATALOG {
        assert!(
            detectable.contains(&shapes.locale),
            "the catalog has {}, and language.rs has no stopword list for it",
            shapes.locale
        );
    }
}

#[test]
fn a_short_reply_in_each_language_is_named() {
    let samples = [
        (
            "en",
            "Thanks for the update. I will have a look at the figures tomorrow and let you know \
             what we think about the plan for next week.",
        ),
        (
            "nl",
            "Dank je voor de update. Ik kijk morgen even naar de cijfers en laat je dan weten wat \
             wij van het plan voor volgende week vinden.",
        ),
        (
            "de",
            "Danke für die Nachricht. Ich schaue mir die Zahlen morgen an und sage Ihnen dann, \
             wie wir den Plan für die nächste Woche finden.",
        ),
        (
            "fr",
            "Merci pour la mise à jour. Je regarde les chiffres demain et je vous dis ce que nous \
             pensons du plan pour la semaine prochaine.",
        ),
        (
            "es",
            "Gracias por la actualización. Mañana miro las cifras y te digo qué pensamos del plan \
             para la semana que viene.",
        ),
        (
            "it",
            "Grazie per l'aggiornamento. Domani guardo i numeri e ti dico che cosa ne pensiamo \
             del piano per la settimana prossima.",
        ),
        (
            "pt",
            "Obrigado pela atualização. Amanhã vejo os números e digo-te o que achamos do plano \
             para a próxima semana, está bem? Não é urgente.",
        ),
    ];
    for (language, text) in samples {
        assert_eq!(detect(text), Some(language), "{text}");
    }
}

/// Danish shares enough function words with Dutch to win on count alone.
#[test]
fn a_neighbouring_language_is_not_taken_for_a_catalog_one() {
    let danish = "Tak for tallene. Jeg har kigget på dem i morges, og de er stort set som \
        forventet, selvom rejseudgifterne er højere end sidste kvartal. Kan du tjekke, om hotellet \
        i Lyon blev booket to gange? Hvis det er tilfældet, beder vi om at få pengene tilbage.";
    assert_eq!(detect(danish), None);
}

#[test]
fn a_text_with_too_little_to_go_on_is_not_named() {
    assert_eq!(detect("OK"), None);
    assert_eq!(detect("Tak for beskeden, vi ses i morgen."), None);
    assert_eq!(detect(""), None);
}
