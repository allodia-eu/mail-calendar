//! The French seed of the showcase (screenshot) dataset — the twin of `super::en`, so the
//! French store listing gets screenshots whose mail reads in the same language as the chrome.
//! Message keys, folder ids, flags, and timings match English exactly; only the language
//! differs. Folder names are the French ones a French mail server would serve.

use engine_core::{
    calendar::Event,
    mail::{Mailbox, MailboxRole, Message},
};
use time::OffsetDateTime;

use super::{
    AccountSeed, Cal, CalendarNames, FRI, InviteText, MON, SAT, SUN, ShowcaseLocale, THU, TUE, WED,
    ago, all_day_event, event, invite_organizer, mailbox, msg, seed, zoned_wd,
};

pub(super) fn primary(now: OffsetDateTime) -> AccountSeed {
    let me = ("Eva Jansen", "eva.jansen@example.com");
    // The invitation's organiser, read from the roster rather than retyped, so the `From:` header
    // and the iTIP `ORGANIZER` can never name different people.
    let invite_from = invite_organizer();
    let mailboxes: Vec<Mailbox> = vec![
        mailbox("inbox", "Boîte de réception", MailboxRole::Inbox),
        mailbox("sent", "Envoyés", MailboxRole::Sent),
        mailbox("archive", "Archive", MailboxRole::Archive),
        mailbox("drafts", "Brouillons", MailboxRole::Drafts),
    ];
    let messages: Vec<Message> = vec![
        msg("p-welcome", "inbox").from("Allodia", "hello@allodia.eu").to(me.0, me.1)
            .subject("Bienvenue dans Allodia Mail & Calendar")
            .preview("Tout est prêt. Voici comment faire d’Allodia Mail & Calendar votre app : connectez un compte, choisissez votre profondeur de synchronisation, et gardez la main.")
            .at(ago(now, 35)).id("welcome-1@allodia.eu").done(),
        // A three-message conversation: Tom's request, Eva's Sent reply, Tom's follow-up.
        msg("p-launch-1", "inbox").from("Tom de Vries", "tom.devries@northwind.example").to(me.0, me.1)
            .subject("Lancement T3 — décision finale")
            .preview("Peux-tu jeter un dernier œil à la checklist avant qu’on fige le jeudi ? J’aimerais surtout ton feu vert sur le plan de retour arrière.")
            .at(ago(now, 150)).id("launch-1@northwind.example").done(),
        msg("p-launch-3", "inbox").from("Tom de Vries", "tom.devries@northwind.example").to(me.0, me.1)
            .subject("Re: Lancement T3 — décision finale")
            .preview("Parfait — merci pour ta réactivité. C’est parti pour jeudi. Je préviens l’équipe.")
            .at(ago(now, 95)).id("launch-3@northwind.example")
            .thread("launch-1@northwind.example", &["launch-1@northwind.example", "launch-2@example.com"]).done(),
        // The meeting invitation: an iTIP REQUEST whose calendar hold is already in the diary,
        // clashing with Thursday's board meeting (`super::invitation`).
        msg("p-invite", "inbox").from(invite_from.0, invite_from.1).to(me.0, me.1)
            .subject("Invitation : lancement du partenariat — jeudi 14h30")
            .preview("Vous êtes invitée au lancement du partenariat. Dites-moi si jeudi après-midi vous convient.")
            .at(ago(now, 55)).id("kickoff-1@northwind.example").done(),
        msg("p-report", "inbox").from("Example Cloud", "reports@example.org").to(me.0, me.1)
            .subject("Votre rapport d’utilisation de juin")
            .preview("Merci d’utiliser Example Cloud. Votre rapport d’utilisation mensuel est joint au format CSV.")
            .at(ago(now, 300)).seen().attachment().id("report-1@example.org").done(),
        msg("p-newsletter", "inbox").from("European Digital Weekly", "weekly@europeandigital.example").to(me.0, me.1)
            .subject("Cette semaine dans la tech européenne")
            .preview("Les règles de souveraineté avancent, et trois outils que nous suivons ce mois-ci.")
            .at(ago(now, 1445)).seen().id("news-1@europeandigital.example").done(),
        msg("p-contract", "inbox").from("Northwind Legal", "legal@northwind.example").to(me.0, me.1)
            .subject("À signer : contrat de partenariat")
            .preview("La version définitive est prête pour votre signature. Dites-nous si quelque chose doit encore être modifié.")
            .at(ago(now, 1600)).seen().flagged().id("contract-1@northwind.example").done(),
        msg("p-shipping", "inbox").from("De Fietswinkel", "orders@fietswinkel.example").to(me.0, me.1)
            .subject("Votre commande a été expédiée")
            .preview("Bonne nouvelle — votre commande est en route. Livraison prévue : demain entre 9h00 et 17h00.")
            .at(ago(now, 2880)).seen().id("ship-1@fietswinkel.example").done(),
        // Eva's own Sent reply on the thread (rides in the conversation with a "Sent" badge).
        msg("p-launch-2", "sent").from(me.0, me.1).to("Tom de Vries", "tom.devries@northwind.example")
            .subject("Re: Lancement T3 — décision finale")
            .preview("La checklist tient la route. Un ajustement sur le plan de retour arrière et, pour moi, c’est bon.")
            .at(ago(now, 120)).seen().id("launch-2@example.com")
            .thread("launch-1@northwind.example", &["launch-1@northwind.example"]).done(),
        msg("p-sent-lunch", "sent").from(me.0, me.1).to("Sofia Ruiz", "sofia.ruiz@northwind.example")
            .subject("Déjeuner jeudi ?")
            .preview("Ça te dit de déjeuner jeudi après la revue ? Il y a une nouvelle adresse juste à côté du bureau.")
            .at(ago(now, 180)).seen().id("lunch-1@example.com").done(),
        msg("p-archive-notes", "archive").from("Sofia Ruiz", "sofia.ruiz@northwind.example").to(me.0, me.1)
            .subject("Notes du séminaire")
            .preview("Deux très bonnes journées — voici le compte rendu et les actions sur lesquelles nous nous sommes mis d’accord.")
            .at(ago(now, 8640)).seen().id("offsite-1@northwind.example").done(),
        msg("p-draft-board", "drafts").from(me.0, me.1).to("Conseil d’administration", "board@northwind.example")
            .subject("Point T3 pour le conseil")
            .preview("Brouillon — les chiffres clés, l’état du lancement, et les deux décisions que j’attends du conseil.")
            .at(ago(now, 240)).draft().id("boarddraft-1@example.com").done(),
    ];
    seed(ShowcaseLocale::Fr, me.1, mailboxes, messages, now)
}

pub(super) fn secondary(now: OffsetDateTime) -> AccountSeed {
    let me = ("Eva Jansen", "eva@northwind.example");
    let mailboxes: Vec<Mailbox> = vec![
        mailbox("inbox", "Boîte de réception", MailboxRole::Inbox),
        mailbox("sent", "Envoyés", MailboxRole::Sent),
    ];
    let messages: Vec<Message> = vec![
        msg("w-welcome", "inbox").from("Northwind People", "hr@northwind.example").to(me.0, me.1)
            .subject("Bienvenue pour votre première semaine")
            .preview("Nous sommes ravis de vous accueillir. Tout ce qu’il faut pour bien démarrer se trouve dans l’espace d’intégration.")
            .at(ago(now, 240)).seen().id("hr-1@northwind.example").done(),
        msg("w-2fa", "inbox").from("Northwind IT", "it@northwind.example").to(me.0, me.1)
            .subject("Action requise : activez la connexion en deux étapes")
            .preview("Merci d’activer la validation en deux étapes sur votre compte avant vendredi. Cela prend environ deux minutes.")
            .at(ago(now, 1400)).flagged().id("it-1@northwind.example").done(),
        msg("w-review", "inbox").from("Sofia Ruiz", "sofia.ruiz@northwind.example").to(me.0, me.1)
            .subject("Revue de design décalée à vendredi")
            .preview("Petite info — nous avons décalé la revue de design à vendredi 11h00 pour que tout le monde puisse venir.")
            .at(ago(now, 2880)).seen().id("review-1@northwind.example").done(),
        msg("w-allhands", "inbox").from("Northwind", "allhands@northwind.example").to(me.0, me.1)
            .subject("Récap de la réunion générale & présentation")
            .preview("Vous avez manqué la réunion générale ? Voici l’enregistrement et la présentation.")
            .at(ago(now, 4320)).seen().id("ah-1@northwind.example").done(),
    ];
    seed(ShowcaseLocale::Fr, me.1, mailboxes, messages, now)
}

/// The three showcase calendars' names and a full week of events spread across them —
/// the same structure as `super::en` (identical keys, calendars, weekdays, times and
/// durations, including the Tuesday clash and the Sunday all-day overflow that exercise
/// column packing and the "+N more" banner); only the language differs.
pub(super) fn calendar(now: OffsetDateTime) -> (CalendarNames, Vec<Event>) {
    let events = vec![
        event(
            "ev-mon-standup",
            "Point d’équipe",
            Cal::Work,
            zoned_wd(now, MON, 9, 30),
            15,
        ),
        event(
            "ev-mon-sprint",
            "Planification du sprint",
            Cal::Work,
            zoned_wd(now, MON, 11, 0),
            60,
        ),
        event(
            "ev-mon-pickup",
            "Sortie d’école",
            Cal::Family,
            zoned_wd(now, MON, 15, 30),
            30,
        ),
        event(
            "ev-mon-designsync",
            "Point design",
            Cal::Work,
            zoned_wd(now, MON, 16, 0),
            45,
        ),
        event(
            "ev-mon-gym",
            "Salle de sport",
            Cal::Personal,
            zoned_wd(now, MON, 18, 30),
            60,
        ),
        event(
            "ev-tue-standup",
            "Point d’équipe",
            Cal::Work,
            zoned_wd(now, TUE, 9, 30),
            15,
        ),
        event(
            "ev-tue-triage",
            "Tri des bugs",
            Cal::Work,
            zoned_wd(now, TUE, 10, 0),
            60,
        ),
        event(
            "ev-tue-design",
            "Revue de design",
            Cal::Work,
            zoned_wd(now, TUE, 10, 15),
            45,
        ),
        event(
            "ev-tue-interview",
            "Entretien : backend",
            Cal::Work,
            zoned_wd(now, TUE, 10, 30),
            60,
        ),
        event(
            "ev-tue-lunch",
            "Déjeuner avec Sofia",
            Cal::Personal,
            zoned_wd(now, TUE, 12, 30),
            60,
        ),
        event(
            "ev-tue-1on1",
            "Point individuel avec Priya",
            Cal::Work,
            zoned_wd(now, TUE, 15, 0),
            30,
        ),
        event(
            "ev-tue-bookclub",
            "Club de lecture",
            Cal::Personal,
            zoned_wd(now, TUE, 19, 30),
            90,
        ),
        event(
            "ev-wed-dentist",
            "Dentiste",
            Cal::Personal,
            zoned_wd(now, WED, 8, 0),
            45,
        ),
        event(
            "ev-wed-standup",
            "Point d’équipe",
            Cal::Work,
            zoned_wd(now, WED, 9, 30),
            15,
        ),
        event(
            "ev-wed-review",
            "Bilan du lancement T3",
            Cal::Work,
            zoned_wd(now, WED, 13, 0),
            90,
        ),
        event(
            "ev-wed-football",
            "Entraînement de foot",
            Cal::Family,
            zoned_wd(now, WED, 16, 30),
            60,
        ),
        event(
            "ev-wed-dinner",
            "Dîner entre amis",
            Cal::Personal,
            zoned_wd(now, WED, 19, 0),
            120,
        ),
        event(
            "ev-thu-standup",
            "Point d’équipe",
            Cal::Work,
            zoned_wd(now, THU, 9, 30),
            15,
        ),
        event(
            "ev-thu-roadmap",
            "Atelier feuille de route",
            Cal::Work,
            zoned_wd(now, THU, 11, 0),
            90,
        ),
        event(
            "ev-thu-board",
            "Conseil d’administration",
            Cal::Work,
            zoned_wd(now, THU, 14, 0),
            120,
        ),
        event(
            "ev-thu-piano",
            "Cours de piano",
            Cal::Family,
            zoned_wd(now, THU, 17, 30),
            45,
        ),
        event(
            "ev-thu-parents",
            "Appeler mes parents",
            Cal::Family,
            zoned_wd(now, THU, 20, 0),
            45,
        ),
        event(
            "ev-fri-standup",
            "Point d’équipe",
            Cal::Work,
            zoned_wd(now, FRI, 9, 30),
            15,
        ),
        event(
            "ev-fri-lunchlearn",
            "Déjeuner-formation",
            Cal::Work,
            zoned_wd(now, FRI, 12, 30),
            60,
        ),
        event(
            "ev-fri-demo",
            "Démo du sprint",
            Cal::Work,
            zoned_wd(now, FRI, 15, 0),
            45,
        ),
        event(
            "ev-fri-haircut",
            "Coiffeur",
            Cal::Personal,
            zoned_wd(now, FRI, 17, 0),
            30,
        ),
        event(
            "ev-fri-datenight",
            "Soirée en amoureux",
            Cal::Personal,
            zoned_wd(now, FRI, 20, 0),
            120,
        ),
        event(
            "ev-sat-market",
            "Marché fermier",
            Cal::Personal,
            zoned_wd(now, SAT, 10, 0),
            90,
        ),
        event(
            "ev-sat-birthday",
            "Anniversaire de mamie",
            Cal::Family,
            zoned_wd(now, SAT, 14, 0),
            150,
        ),
        event(
            "ev-sat-cinema",
            "Cinéma",
            Cal::Personal,
            zoned_wd(now, SAT, 19, 30),
            150,
        ),
        event(
            "ev-sun-brunch",
            "Brunch avec Sofia",
            Cal::Personal,
            zoned_wd(now, SUN, 11, 0),
            90,
        ),
        event(
            "ev-sun-1on1",
            "Point individuel avec Tom",
            Cal::Work,
            zoned_wd(now, SUN, 15, 0),
            30,
        ),
        event(
            "ev-sun-walk",
            "Balade en famille",
            Cal::Family,
            zoned_wd(now, SUN, 16, 0),
            90,
        ),
        event(
            "ev-sun-mealprep",
            "Préparation des repas",
            Cal::Personal,
            zoned_wd(now, SUN, 18, 0),
            45,
        ),
        all_day_event(
            "ev-ad-offsite",
            "Séminaire d’équipe",
            Cal::Work,
            now,
            WED,
            2,
        ),
        all_day_event("ev-ad-leave", "Lisa en congé", Cal::Work, now, MON, 3),
        all_day_event(
            "ev-ad-release",
            "Mise en production",
            Cal::Work,
            now,
            FRI,
            1,
        ),
        all_day_event(
            "ev-ad-visiting",
            "Papa et maman en visite",
            Cal::Family,
            now,
            SAT,
            2,
        ),
        all_day_event(
            "ev-ad-birthday",
            "Anniversaire de Tom",
            Cal::Family,
            now,
            SUN,
            1,
        ),
        all_day_event("ev-ad-holiday", "Jour férié", Cal::Personal, now, SUN, 1),
        all_day_event(
            "ev-ad-marathon",
            "Semi-marathon",
            Cal::Personal,
            now,
            SUN,
            1,
        ),
    ];
    (
        CalendarNames {
            work: "Travail",
            personal: "Personnel",
            family: "Famille",
        },
        events,
    )
}

/// The meeting invitation's title, room and notes. See [`super::invite_text`].
pub(super) fn invite_text() -> InviteText {
    InviteText {
        summary: "Lancement du partenariat",
        location: "Salle de réunion Amstel · Amsterdam",
        description: "Une heure pour convenir du périmètre, du calendrier et de qui fait quoi. \
                      Apportez le projet de plan — nous le parcourrons ensemble.",
    }
}

/// The sample text the showcase reply composer opens pre-filled with — Eva's answer to Northwind
/// Legal's signature request (`p-contract`). See [`super::showcase_reply`].
pub(super) fn reply_text() -> &'static str {
    "Merci — je l’ai lu en entier et tout me convient. \
     Je le signe cet après-midi et je vous renvoie la version contresignée dans la journée."
}

/// The two signatures the showcase library holds. See [`super::signatures`].
pub(super) fn signatures() -> super::ShowcaseSignatures {
    super::ShowcaseSignatures {
        primary: super::SignatureSeed {
            name: "Travail",
            body_html: "<div><b>Eva Jansen</b></div><div>Responsable produit · Northwind</div>\
                        <div>eva.jansen@example.com</div>",
            body_plain: "Eva Jansen\nResponsable produit · Northwind\neva.jansen@example.com",
        },
        secondary: super::SignatureSeed {
            name: "Courte",
            body_html: "<div>Eva Jansen</div><div>eva@northwind.example</div>",
            body_plain: "Eva Jansen\neva@northwind.example",
        },
    }
}
