//! The Dutch seed of the showcase (screenshot) dataset — the twin of `super::en`, so the
//! Dutch store listing gets screenshots whose mail reads in the same language as the chrome.
//! Message keys, folder ids, flags, and timings match English exactly; only the language
//! differs. Folder names are the Dutch ones a Dutch mail server would serve.

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
        mailbox("inbox", "Postvak IN", MailboxRole::Inbox),
        mailbox("sent", "Verzonden", MailboxRole::Sent),
        mailbox("archive", "Archief", MailboxRole::Archive),
        mailbox("drafts", "Concepten", MailboxRole::Drafts),
    ];
    let messages: Vec<Message> = vec![
        msg("p-welcome", "inbox").from("Allodia", "hello@allodia.eu").to(me.0, me.1)
            .subject("Welkom bij Allodia Mail & Calendar")
            .preview("Je bent klaar om te beginnen. Zo maak je Allodia Mail & Calendar van jezelf — koppel een account, kies hoe ver terug je synchroniseert, en houd de regie.")
            .at(ago(now, 35)).id("welcome-1@allodia.eu").done(),
        // A three-message conversation: Tom's request, Eva's Sent reply, Tom's follow-up.
        msg("p-launch-1", "inbox").from("Tom de Vries", "tom.devries@northwind.example").to(me.0, me.1)
            .subject("Q3-lancering — definitieve go / no-go")
            .preview("Kun je nog één keer naar de checklist kijken voordat we donderdag vastleggen? Ik wil vooral je akkoord op het terugrolplan.")
            .at(ago(now, 150)).id("launch-1@northwind.example").done(),
        msg("p-launch-3", "inbox").from("Tom de Vries", "tom.devries@northwind.example").to(me.0, me.1)
            .subject("Re: Q3-lancering — definitieve go / no-go")
            .preview("Perfect — bedankt voor de snelle reactie. We gaan donderdag live. Ik laat het het team weten.")
            .at(ago(now, 95)).id("launch-3@northwind.example")
            .thread("launch-1@northwind.example", &["launch-1@northwind.example", "launch-2@example.com"]).done(),
        // The meeting invitation: an iTIP REQUEST whose calendar hold is already in the diary,
        // clashing with Thursday's board meeting (`super::invitation`).
        msg("p-invite", "inbox").from(invite_from.0, invite_from.1).to(me.0, me.1)
            .subject("Uitnodiging: kick-off samenwerking — donderdag 14:30")
            .preview("Je bent uitgenodigd voor de kick-off van de samenwerking. Laat je even weten of donderdagmiddag je uitkomt?")
            .at(ago(now, 55)).id("kickoff-1@northwind.example").done(),
        msg("p-report", "inbox").from("Example Cloud", "reports@example.org").to(me.0, me.1)
            .subject("Je verbruiksoverzicht van juni")
            .preview("Bedankt dat je Example Cloud gebruikt. Je maandelijkse verbruiksoverzicht zit als CSV in de bijlage.")
            .at(ago(now, 300)).seen().attachment().id("report-1@example.org").done(),
        msg("p-newsletter", "inbox").from("European Digital Weekly", "weekly@europeandigital.example").to(me.0, me.1)
            .subject("Deze week in de Europese tech")
            .preview("De soevereiniteitsregels gaan een stap verder, plus drie tools die we deze maand volgen.")
            .at(ago(now, 1445)).seen().id("news-1@europeandigital.example").done(),
        msg("p-contract", "inbox").from("Northwind Legal", "legal@northwind.example").to(me.0, me.1)
            .subject("Graag ondertekenen: samenwerkingsovereenkomst")
            .preview("De definitieve versie ligt klaar voor je handtekening. Laat het weten als er nog iets moet worden aangepast.")
            .at(ago(now, 1600)).seen().flagged().id("contract-1@northwind.example").done(),
        msg("p-shipping", "inbox").from("De Fietswinkel", "orders@fietswinkel.example").to(me.0, me.1)
            .subject("Je bestelling is verzonden")
            .preview("Goed nieuws — je bestelling is onderweg. Verwachte bezorging: morgen tussen 9:00 en 17:00.")
            .at(ago(now, 2880)).seen().id("ship-1@fietswinkel.example").done(),
        // Eva's own Sent reply on the thread (rides in the conversation with a "Sent" badge).
        msg("p-launch-2", "sent").from(me.0, me.1).to("Tom de Vries", "tom.devries@northwind.example")
            .subject("Re: Q3-lancering — definitieve go / no-go")
            .preview("De checklist ziet er goed uit. Eén aanpassing aan het terugrolplan en dan is het wat mij betreft een go.")
            .at(ago(now, 120)).seen().id("launch-2@example.com")
            .thread("launch-1@northwind.example", &["launch-1@northwind.example"]).done(),
        msg("p-sent-lunch", "sent").from(me.0, me.1).to("Sofia Ruiz", "sofia.ruiz@northwind.example")
            .subject("Donderdag lunchen?")
            .preview("Zin om donderdag na de review te lunchen? Er is een nieuwe zaak vlak bij kantoor.")
            .at(ago(now, 180)).seen().id("lunch-1@example.com").done(),
        msg("p-archive-notes", "archive").from("Sofia Ruiz", "sofia.ruiz@northwind.example").to(me.0, me.1)
            .subject("Aantekeningen van de heidag")
            .preview("Twee mooie dagen — hierbij de samenvatting en de actiepunten die we hebben afgesproken.")
            .at(ago(now, 8640)).seen().id("offsite-1@northwind.example").done(),
        msg("p-draft-board", "drafts").from(me.0, me.1).to("Bestuur", "board@northwind.example")
            .subject("Bestuursupdate Q3")
            .preview("Concept — de belangrijkste cijfers, de status van de lancering, en de twee besluiten die ik van het bestuur nodig heb.")
            .at(ago(now, 240)).draft().id("boarddraft-1@example.com").done(),
    ];
    seed(ShowcaseLocale::Nl, me.1, mailboxes, messages, now)
}

pub(super) fn secondary(now: OffsetDateTime) -> AccountSeed {
    let me = ("Eva Jansen", "eva@northwind.example");
    let mailboxes: Vec<Mailbox> = vec![
        mailbox("inbox", "Postvak IN", MailboxRole::Inbox),
        mailbox("sent", "Verzonden", MailboxRole::Sent),
    ];
    let messages: Vec<Message> = vec![
        msg("w-welcome", "inbox").from("Northwind People", "hr@northwind.example").to(me.0, me.1)
            .subject("Welkom in je eerste week")
            .preview("Fijn dat je er bent. Alles voor een vliegende start staat klaar in de onboardingomgeving.")
            .at(ago(now, 240)).seen().id("hr-1@northwind.example").done(),
        msg("w-2fa", "inbox").from("Northwind IT", "it@northwind.example").to(me.0, me.1)
            .subject("Actie nodig: stel tweestapsaanmelding in")
            .preview("Zet vóór vrijdag verificatie in twee stappen aan op je account. Het kost ongeveer twee minuten.")
            .at(ago(now, 1400)).flagged().id("it-1@northwind.example").done(),
        msg("w-review", "inbox").from("Sofia Ruiz", "sofia.ruiz@northwind.example").to(me.0, me.1)
            .subject("Ontwerpreview verplaatst naar vrijdag")
            .preview("Even een seintje — we hebben de ontwerpreview naar vrijdag 11:00 verzet zodat iedereen erbij kan zijn.")
            .at(ago(now, 2880)).seen().id("review-1@northwind.example").done(),
        msg("w-allhands", "inbox").from("Northwind", "allhands@northwind.example").to(me.0, me.1)
            .subject("Samenvatting all-hands & slides")
            .preview("De all-hands gemist? Hier zijn de opname en de presentatie.")
            .at(ago(now, 4320)).seen().id("ah-1@northwind.example").done(),
    ];
    seed(ShowcaseLocale::Nl, me.1, mailboxes, messages, now)
}

/// De namen van de drie showcase-agenda's en een volle week aan afspraken, verdeeld over die
/// agenda's — een echte werk-en-privéweek (Europe/Amsterdam). Elke afspraak is verankerd aan een
/// **weekdag van de huidige week** en aan een tijdstip, zodat de zichtbare week gevuld is,
/// ongeacht op welke dag of welk uur de screenshot wordt gemaakt.
pub(super) fn calendar(now: OffsetDateTime) -> (CalendarNames, Vec<Event>) {
    let events = vec![
        // ---- Maandag -------------------------------------------------------------------------
        event(
            "ev-mon-standup",
            "Teamstandup",
            Cal::Work,
            zoned_wd(now, MON, 9, 30),
            15,
        ),
        event(
            "ev-mon-sprint",
            "Sprintplanning",
            Cal::Work,
            zoned_wd(now, MON, 11, 0),
            60,
        ),
        event(
            "ev-mon-pickup",
            "Kinderen ophalen",
            Cal::Family,
            zoned_wd(now, MON, 15, 30),
            30,
        ),
        event(
            "ev-mon-designsync",
            "Ontwerpsync",
            Cal::Work,
            zoned_wd(now, MON, 16, 0),
            45,
        ),
        event(
            "ev-mon-gym",
            "Sporten",
            Cal::Personal,
            zoned_wd(now, MON, 18, 30),
            60,
        ),
        // ---- Dinsdag: DRIE hiervan overlappen, en dat is de bedoeling — de core pakt een botsing
        // in kolommen (`calendar/packing.rs`); een client die `column`/`columns` negeert tekent ze
        // op volle breedte over elkaar heen, en een afspraak achter een andere is er een die je
        // mist. Een showcase waarin nooit iets botst, toont die fout ook nooit.
        event(
            "ev-tue-standup",
            "Teamstandup",
            Cal::Work,
            zoned_wd(now, TUE, 9, 30),
            15,
        ),
        event(
            "ev-tue-triage",
            "Bugtriage",
            Cal::Work,
            zoned_wd(now, TUE, 10, 0),
            60,
        ),
        event(
            "ev-tue-design",
            "Ontwerpreview",
            Cal::Work,
            zoned_wd(now, TUE, 10, 15),
            45,
        ),
        event(
            "ev-tue-interview",
            "Sollicitatie: backend",
            Cal::Work,
            zoned_wd(now, TUE, 10, 30),
            60,
        ),
        event(
            "ev-tue-lunch",
            "Lunch met Sofia",
            Cal::Personal,
            zoned_wd(now, TUE, 12, 30),
            60,
        ),
        event(
            "ev-tue-1on1",
            "1-op-1 met Priya",
            Cal::Work,
            zoned_wd(now, TUE, 15, 0),
            30,
        ),
        event(
            "ev-tue-bookclub",
            "Leesclub",
            Cal::Personal,
            zoned_wd(now, TUE, 19, 30),
            90,
        ),
        // ---- Woensdag ------------------------------------------------------------------------
        event(
            "ev-wed-dentist",
            "Tandarts",
            Cal::Personal,
            zoned_wd(now, WED, 8, 0),
            45,
        ),
        event(
            "ev-wed-standup",
            "Teamstandup",
            Cal::Work,
            zoned_wd(now, WED, 9, 30),
            15,
        ),
        event(
            "ev-wed-review",
            "Review Q3-lancering",
            Cal::Work,
            zoned_wd(now, WED, 13, 0),
            90,
        ),
        event(
            "ev-wed-football",
            "Voetbaltraining",
            Cal::Family,
            zoned_wd(now, WED, 16, 30),
            60,
        ),
        event(
            "ev-wed-dinner",
            "Etentje met vrienden",
            Cal::Personal,
            zoned_wd(now, WED, 19, 0),
            120,
        ),
        // ---- Donderdag -----------------------------------------------------------------------
        event(
            "ev-thu-standup",
            "Teamstandup",
            Cal::Work,
            zoned_wd(now, THU, 9, 30),
            15,
        ),
        event(
            "ev-thu-roadmap",
            "Roadmap-workshop",
            Cal::Work,
            zoned_wd(now, THU, 11, 0),
            90,
        ),
        event(
            "ev-thu-board",
            "Bestuursvergadering",
            Cal::Work,
            zoned_wd(now, THU, 14, 0),
            120,
        ),
        event(
            "ev-thu-piano",
            "Pianoles",
            Cal::Family,
            zoned_wd(now, THU, 17, 30),
            45,
        ),
        event(
            "ev-thu-parents",
            "Bellen met ouders",
            Cal::Family,
            zoned_wd(now, THU, 20, 0),
            45,
        ),
        // ---- Vrijdag -------------------------------------------------------------------------
        event(
            "ev-fri-standup",
            "Teamstandup",
            Cal::Work,
            zoned_wd(now, FRI, 9, 30),
            15,
        ),
        event(
            "ev-fri-lunchlearn",
            "Kennislunch",
            Cal::Work,
            zoned_wd(now, FRI, 12, 30),
            60,
        ),
        event(
            "ev-fri-demo",
            "Sprintdemo",
            Cal::Work,
            zoned_wd(now, FRI, 15, 0),
            45,
        ),
        event(
            "ev-fri-haircut",
            "Kapper",
            Cal::Personal,
            zoned_wd(now, FRI, 17, 0),
            30,
        ),
        event(
            "ev-fri-datenight",
            "Avondje uit",
            Cal::Personal,
            zoned_wd(now, FRI, 20, 0),
            120,
        ),
        // ---- Zaterdag ------------------------------------------------------------------------
        event(
            "ev-sat-market",
            "Boerenmarkt",
            Cal::Personal,
            zoned_wd(now, SAT, 10, 0),
            90,
        ),
        event(
            "ev-sat-birthday",
            "Oma's verjaardag",
            Cal::Family,
            zoned_wd(now, SAT, 14, 0),
            150,
        ),
        event(
            "ev-sat-cinema",
            "Bioscoop",
            Cal::Personal,
            zoned_wd(now, SAT, 19, 30),
            150,
        ),
        // ---- Zondag --------------------------------------------------------------------------
        event(
            "ev-sun-brunch",
            "Brunch met Sofia",
            Cal::Personal,
            zoned_wd(now, SUN, 11, 0),
            90,
        ),
        event(
            "ev-sun-1on1",
            "1-op-1 met Tom",
            Cal::Work,
            zoned_wd(now, SUN, 15, 0),
            30,
        ),
        event(
            "ev-sun-walk",
            "Familiewandeling",
            Cal::Family,
            zoned_wd(now, SUN, 16, 0),
            90,
        ),
        event(
            "ev-sun-mealprep",
            "Maaltijden voorbereiden",
            Cal::Personal,
            zoned_wd(now, SUN, 18, 0),
            45,
        ),
        // ---- Hele dag en meerdaags, in de balk boven het raster. VIER overlappen op zondag — één
        // baan meer dan de ingeklapte balk toont, zodat "+N meer" ook echt wordt geraakt; de
        // weekbalken (uitje, afwezig) beslaan meerdere dagen.
        all_day_event("ev-ad-offsite", "Teamuitje", Cal::Work, now, WED, 2),
        all_day_event("ev-ad-leave", "Lisa afwezig", Cal::Work, now, MON, 3),
        all_day_event("ev-ad-release", "Releasedag", Cal::Work, now, FRI, 1),
        all_day_event(
            "ev-ad-visiting",
            "Ouders op bezoek",
            Cal::Family,
            now,
            SAT,
            2,
        ),
        all_day_event(
            "ev-ad-birthday",
            "Toms verjaardag",
            Cal::Family,
            now,
            SUN,
            1,
        ),
        all_day_event(
            "ev-ad-holiday",
            "Nationale feestdag",
            Cal::Personal,
            now,
            SUN,
            1,
        ),
        all_day_event(
            "ev-ad-marathon",
            "Halve marathon",
            Cal::Personal,
            now,
            SUN,
            1,
        ),
    ];
    (
        CalendarNames {
            work: "Werk",
            personal: "Persoonlijk",
            family: "Familie",
        },
        events,
    )
}

/// The meeting invitation's title, room and notes. See [`super::invite_text`].
pub(super) fn invite_text() -> InviteText {
    InviteText {
        summary: "Kick-off samenwerking",
        location: "Vergaderzaal Amstel · Amsterdam",
        description: "Een uur om de scope, de planning en de verdeling af te spreken. \
                      Neem het conceptplan mee — dat lopen we samen door.",
    }
}

/// The sample text the showcase reply composer opens pre-filled with — Eva's answer to Northwind
/// Legal's signature request (`p-contract`). See [`super::showcase_reply`].
pub(super) fn reply_text() -> &'static str {
    "Bedankt — ik heb hem doorgenomen en wat mij betreft is alles in orde. \
     Ik onderteken hem vanmiddag en stuur de getekende versie vandaag nog terug."
}

/// De twee handtekeningen in de showcasebibliotheek. Zie [`super::signatures`].
pub(super) fn signatures() -> super::ShowcaseSignatures {
    super::ShowcaseSignatures {
        primary: super::SignatureSeed {
            name: "Werk",
            body_html: "<div><b>Eva Jansen</b></div><div>Productlead · Northwind</div>\
                        <div>eva.jansen@example.com</div>",
            body_plain: "Eva Jansen\nProductlead · Northwind\neva.jansen@example.com",
        },
        secondary: super::SignatureSeed {
            name: "Kort",
            body_html: "<div>Eva Jansen</div><div>eva@northwind.example</div>",
            body_plain: "Eva Jansen\neva@northwind.example",
        },
    }
}
