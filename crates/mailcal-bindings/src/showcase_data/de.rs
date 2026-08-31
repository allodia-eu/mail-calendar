//! The German seed of the showcase (screenshot) dataset — the twin of `super::en`, so the
//! German store listing gets screenshots whose mail reads in the same language as the chrome.
//! Message keys, folder ids, flags, and timings match English exactly; only the language
//! differs. Folder names are the German ones a German mail server would serve.

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
        mailbox("inbox", "Posteingang", MailboxRole::Inbox),
        mailbox("sent", "Gesendet", MailboxRole::Sent),
        mailbox("archive", "Archiv", MailboxRole::Archive),
        mailbox("drafts", "Entwürfe", MailboxRole::Drafts),
    ];
    let messages: Vec<Message> = vec![
        msg("p-welcome", "inbox").from("Allodia", "hello@allodia.eu").to(me.0, me.1)
            .subject("Willkommen bei Allodia Mail & Calendar")
            .preview("Alles ist bereit. So machen Sie Allodia Mail & Calendar zu Ihrem — verbinden Sie ein Konto, wählen Sie die Synchronisationstiefe und behalten Sie die Kontrolle.")
            .at(ago(now, 35)).id("welcome-1@allodia.eu").done(),
        // A three-message conversation: Tom's request, Eva's Sent reply, Tom's follow-up.
        msg("p-launch-1", "inbox").from("Tom de Vries", "tom.devries@northwind.example").to(me.0, me.1)
            .subject("Q3-Launch — endgültiges Go / No-Go")
            .preview("Kannst du dir die Checkliste noch einmal ansehen, bevor wir den Donnerstag festzurren? Vor allem hätte ich gern dein Okay zum Rollback-Plan.")
            .at(ago(now, 150)).id("launch-1@northwind.example").done(),
        msg("p-launch-3", "inbox").from("Tom de Vries", "tom.devries@northwind.example").to(me.0, me.1)
            .subject("Re: Q3-Launch — endgültiges Go / No-Go")
            .preview("Perfekt — danke für die schnelle Rückmeldung. Wir gehen am Donnerstag live. Ich sage dem Team Bescheid.")
            .at(ago(now, 95)).id("launch-3@northwind.example")
            .thread("launch-1@northwind.example", &["launch-1@northwind.example", "launch-2@example.com"]).done(),
        // The meeting invitation: an iTIP REQUEST whose calendar hold is already in the diary,
        // clashing with Thursday's board meeting (`super::invitation`).
        msg("p-invite", "inbox").from(invite_from.0, invite_from.1).to(me.0, me.1)
            .subject("Einladung: Kick-off der Partnerschaft — Donnerstag 14:30")
            .preview("Sie sind zum Kick-off der Partnerschaft eingeladen. Sagen Sie mir bitte, ob Donnerstagnachmittag passt.")
            .at(ago(now, 55)).id("kickoff-1@northwind.example").done(),
        msg("p-report", "inbox").from("Example Cloud", "reports@example.org").to(me.0, me.1)
            .subject("Ihre Nutzungsübersicht für Juni")
            .preview("Danke, dass Sie Example Cloud nutzen. Ihre monatliche Nutzungsübersicht liegt als CSV bei.")
            .at(ago(now, 300)).seen().attachment().id("report-1@example.org").done(),
        msg("p-newsletter", "inbox").from("European Digital Weekly", "weekly@europeandigital.example").to(me.0, me.1)
            .subject("Diese Woche in der europäischen Tech-Welt")
            .preview("Die Souveränitätsregeln gehen einen Schritt weiter, dazu drei Tools, die wir diesen Monat beobachten.")
            .at(ago(now, 1445)).seen().id("news-1@europeandigital.example").done(),
        msg("p-contract", "inbox").from("Northwind Legal", "legal@northwind.example").to(me.0, me.1)
            .subject("Bitte unterschreiben: Partnerschaftsvertrag")
            .preview("Die endgültige Fassung liegt zur Unterschrift bereit. Sagen Sie Bescheid, falls noch etwas geändert werden muss.")
            .at(ago(now, 1600)).seen().flagged().id("contract-1@northwind.example").done(),
        msg("p-shipping", "inbox").from("De Fietswinkel", "orders@fietswinkel.example").to(me.0, me.1)
            .subject("Ihre Bestellung ist unterwegs")
            .preview("Gute Nachrichten — Ihre Bestellung ist unterwegs. Voraussichtliche Zustellung: morgen zwischen 9:00 und 17:00 Uhr.")
            .at(ago(now, 2880)).seen().id("ship-1@fietswinkel.example").done(),
        // Eva's own Sent reply on the thread (rides in the conversation with a "Sent" badge).
        msg("p-launch-2", "sent").from(me.0, me.1).to("Tom de Vries", "tom.devries@northwind.example")
            .subject("Re: Q3-Launch — endgültiges Go / No-Go")
            .preview("Die Checkliste sieht solide aus. Eine Anpassung am Rollback-Plan, dann ist es von mir aus ein Go.")
            .at(ago(now, 120)).seen().id("launch-2@example.com")
            .thread("launch-1@northwind.example", &["launch-1@northwind.example"]).done(),
        msg("p-sent-lunch", "sent").from(me.0, me.1).to("Sofia Ruiz", "sofia.ruiz@northwind.example")
            .subject("Donnerstag zusammen Mittag essen?")
            .preview("Lust, am Donnerstag nach dem Review zusammen Mittag zu essen? Ganz in der Nähe hat ein neues Lokal aufgemacht.")
            .at(ago(now, 180)).seen().id("lunch-1@example.com").done(),
        msg("p-archive-notes", "archive").from("Sofia Ruiz", "sofia.ruiz@northwind.example").to(me.0, me.1)
            .subject("Notizen von der Klausurtagung")
            .preview("Zwei richtig gute Tage — hier die Zusammenfassung und die Aufgaben, auf die wir uns geeinigt haben.")
            .at(ago(now, 8640)).seen().id("offsite-1@northwind.example").done(),
        msg("p-draft-board", "drafts").from(me.0, me.1).to("Vorstand", "board@northwind.example")
            .subject("Vorstandsupdate Q3")
            .preview("Entwurf — die wichtigsten Zahlen, der Stand des Launches und die zwei Entscheidungen, die ich vom Vorstand brauche.")
            .at(ago(now, 240)).draft().id("boarddraft-1@example.com").done(),
    ];
    seed(ShowcaseLocale::De, me.1, mailboxes, messages, now)
}

pub(super) fn secondary(now: OffsetDateTime) -> AccountSeed {
    let me = ("Eva Jansen", "eva@northwind.example");
    let mailboxes: Vec<Mailbox> = vec![
        mailbox("inbox", "Posteingang", MailboxRole::Inbox),
        mailbox("sent", "Gesendet", MailboxRole::Sent),
    ];
    let messages: Vec<Message> = vec![
        msg("w-welcome", "inbox").from("Northwind People", "hr@northwind.example").to(me.0, me.1)
            .subject("Willkommen in Ihrer ersten Woche")
            .preview("Schön, dass Sie da sind. Alles für einen guten Start finden Sie im Onboarding-Bereich.")
            .at(ago(now, 240)).seen().id("hr-1@northwind.example").done(),
        msg("w-2fa", "inbox").from("Northwind IT", "it@northwind.example").to(me.0, me.1)
            .subject("Aktion nötig: Zwei-Faktor-Anmeldung einrichten")
            .preview("Bitte aktivieren Sie die Zwei-Faktor-Authentifizierung für Ihr Konto vor Freitag. Es dauert etwa zwei Minuten.")
            .at(ago(now, 1400)).flagged().id("it-1@northwind.example").done(),
        msg("w-review", "inbox").from("Sofia Ruiz", "sofia.ruiz@northwind.example").to(me.0, me.1)
            .subject("Design-Review auf Freitag verschoben")
            .preview("Kurze Info — wir haben das Design-Review auf Freitag, 11:00 Uhr verschoben, damit alle dabei sein können.")
            .at(ago(now, 2880)).seen().id("review-1@northwind.example").done(),
        msg("w-allhands", "inbox").from("Northwind", "allhands@northwind.example").to(me.0, me.1)
            .subject("Zusammenfassung & Folien der All-Hands")
            .preview("Die All-Hands verpasst? Hier sind die Aufzeichnung und die Präsentation.")
            .at(ago(now, 4320)).seen().id("ah-1@northwind.example").done(),
    ];
    seed(ShowcaseLocale::De, me.1, mailboxes, messages, now)
}

/// The three showcase calendars' names and a full week of events spread across them —
/// the same structure as `super::en` (identical keys, calendars, weekdays, times and
/// durations, including the Tuesday clash and the Sunday all-day overflow that exercise
/// column packing and the "+N more" banner); only the language differs.
pub(super) fn calendar(now: OffsetDateTime) -> (CalendarNames, Vec<Event>) {
    let events = vec![
        event(
            "ev-mon-standup",
            "Team-Standup",
            Cal::Work,
            zoned_wd(now, MON, 9, 30),
            15,
        ),
        event(
            "ev-mon-sprint",
            "Sprint-Planung",
            Cal::Work,
            zoned_wd(now, MON, 11, 0),
            60,
        ),
        event(
            "ev-mon-pickup",
            "Kinder abholen",
            Cal::Family,
            zoned_wd(now, MON, 15, 30),
            30,
        ),
        event(
            "ev-mon-designsync",
            "Design-Sync",
            Cal::Work,
            zoned_wd(now, MON, 16, 0),
            45,
        ),
        event(
            "ev-mon-gym",
            "Sport",
            Cal::Personal,
            zoned_wd(now, MON, 18, 30),
            60,
        ),
        event(
            "ev-tue-standup",
            "Team-Standup",
            Cal::Work,
            zoned_wd(now, TUE, 9, 30),
            15,
        ),
        event(
            "ev-tue-triage",
            "Bug-Triage",
            Cal::Work,
            zoned_wd(now, TUE, 10, 0),
            60,
        ),
        event(
            "ev-tue-design",
            "Design-Review",
            Cal::Work,
            zoned_wd(now, TUE, 10, 15),
            45,
        ),
        event(
            "ev-tue-interview",
            "Interview: Backend",
            Cal::Work,
            zoned_wd(now, TUE, 10, 30),
            60,
        ),
        event(
            "ev-tue-lunch",
            "Mittagessen mit Sofia",
            Cal::Personal,
            zoned_wd(now, TUE, 12, 30),
            60,
        ),
        event(
            "ev-tue-1on1",
            "1:1 mit Priya",
            Cal::Work,
            zoned_wd(now, TUE, 15, 0),
            30,
        ),
        event(
            "ev-tue-bookclub",
            "Lesekreis",
            Cal::Personal,
            zoned_wd(now, TUE, 19, 30),
            90,
        ),
        event(
            "ev-wed-dentist",
            "Zahnarzt",
            Cal::Personal,
            zoned_wd(now, WED, 8, 0),
            45,
        ),
        event(
            "ev-wed-standup",
            "Team-Standup",
            Cal::Work,
            zoned_wd(now, WED, 9, 30),
            15,
        ),
        event(
            "ev-wed-review",
            "Q3-Launch-Review",
            Cal::Work,
            zoned_wd(now, WED, 13, 0),
            90,
        ),
        event(
            "ev-wed-football",
            "Fußballtraining",
            Cal::Family,
            zoned_wd(now, WED, 16, 30),
            60,
        ),
        event(
            "ev-wed-dinner",
            "Abendessen mit Freunden",
            Cal::Personal,
            zoned_wd(now, WED, 19, 0),
            120,
        ),
        event(
            "ev-thu-standup",
            "Team-Standup",
            Cal::Work,
            zoned_wd(now, THU, 9, 30),
            15,
        ),
        event(
            "ev-thu-roadmap",
            "Roadmap-Workshop",
            Cal::Work,
            zoned_wd(now, THU, 11, 0),
            90,
        ),
        event(
            "ev-thu-board",
            "Vorstandssitzung",
            Cal::Work,
            zoned_wd(now, THU, 14, 0),
            120,
        ),
        event(
            "ev-thu-piano",
            "Klavierstunde",
            Cal::Family,
            zoned_wd(now, THU, 17, 30),
            45,
        ),
        event(
            "ev-thu-parents",
            "Eltern anrufen",
            Cal::Family,
            zoned_wd(now, THU, 20, 0),
            45,
        ),
        event(
            "ev-fri-standup",
            "Team-Standup",
            Cal::Work,
            zoned_wd(now, FRI, 9, 30),
            15,
        ),
        event(
            "ev-fri-lunchlearn",
            "Lunch & Learn",
            Cal::Work,
            zoned_wd(now, FRI, 12, 30),
            60,
        ),
        event(
            "ev-fri-demo",
            "Sprint-Demo",
            Cal::Work,
            zoned_wd(now, FRI, 15, 0),
            45,
        ),
        event(
            "ev-fri-haircut",
            "Friseur",
            Cal::Personal,
            zoned_wd(now, FRI, 17, 0),
            30,
        ),
        event(
            "ev-fri-datenight",
            "Abend zu zweit",
            Cal::Personal,
            zoned_wd(now, FRI, 20, 0),
            120,
        ),
        event(
            "ev-sat-market",
            "Wochenmarkt",
            Cal::Personal,
            zoned_wd(now, SAT, 10, 0),
            90,
        ),
        event(
            "ev-sat-birthday",
            "Omas Geburtstag",
            Cal::Family,
            zoned_wd(now, SAT, 14, 0),
            150,
        ),
        event(
            "ev-sat-cinema",
            "Kino",
            Cal::Personal,
            zoned_wd(now, SAT, 19, 30),
            150,
        ),
        event(
            "ev-sun-brunch",
            "Brunch mit Sofia",
            Cal::Personal,
            zoned_wd(now, SUN, 11, 0),
            90,
        ),
        event(
            "ev-sun-1on1",
            "1:1 mit Tom",
            Cal::Work,
            zoned_wd(now, SUN, 15, 0),
            30,
        ),
        event(
            "ev-sun-walk",
            "Familienspaziergang",
            Cal::Family,
            zoned_wd(now, SUN, 16, 0),
            90,
        ),
        event(
            "ev-sun-mealprep",
            "Essen vorkochen",
            Cal::Personal,
            zoned_wd(now, SUN, 18, 0),
            45,
        ),
        all_day_event("ev-ad-offsite", "Team-Offsite", Cal::Work, now, WED, 2),
        all_day_event("ev-ad-leave", "Lisa im Urlaub", Cal::Work, now, MON, 3),
        all_day_event("ev-ad-release", "Release-Tag", Cal::Work, now, FRI, 1),
        all_day_event(
            "ev-ad-visiting",
            "Eltern zu Besuch",
            Cal::Family,
            now,
            SAT,
            2,
        ),
        all_day_event(
            "ev-ad-birthday",
            "Toms Geburtstag",
            Cal::Family,
            now,
            SUN,
            1,
        ),
        all_day_event("ev-ad-holiday", "Feiertag", Cal::Personal, now, SUN, 1),
        all_day_event("ev-ad-marathon", "Halbmarathon", Cal::Personal, now, SUN, 1),
    ];
    (
        CalendarNames {
            work: "Arbeit",
            personal: "Privat",
            family: "Familie",
        },
        events,
    )
}

/// The meeting invitation's title, room and notes. See [`super::invite_text`].
pub(super) fn invite_text() -> InviteText {
    InviteText {
        summary: "Kick-off der Partnerschaft",
        location: "Besprechungsraum Amstel · Amsterdam",
        description: "Eine Stunde, um Umfang, Zeitplan und Zuständigkeiten festzulegen. \
                      Bringen Sie den Planentwurf mit — wir gehen ihn gemeinsam durch.",
    }
}

/// The sample text the showcase reply composer opens pre-filled with — Eva's answer to Northwind
/// Legal's signature request (`p-contract`). See [`super::showcase_reply`].
pub(super) fn reply_text() -> &'static str {
    "Vielen Dank — ich habe ihn durchgelesen und aus meiner Sicht passt alles. \
     Ich unterschreibe ihn heute Nachmittag und schicke die gegengezeichnete Fassung noch heute zurück."
}

/// The two signatures the showcase library holds. See [`super::signatures`].
pub(super) fn signatures() -> super::ShowcaseSignatures {
    super::ShowcaseSignatures {
        primary: super::SignatureSeed {
            name: "Arbeit",
            body_html: "<div><b>Eva Jansen</b></div><div>Produktleitung · Northwind</div>\
                        <div>eva.jansen@example.com</div>",
            body_plain: "Eva Jansen\nProduktleitung · Northwind\neva.jansen@example.com",
        },
        secondary: super::SignatureSeed {
            name: "Kurz",
            body_html: "<div>Eva Jansen</div><div>eva@northwind.example</div>",
            body_plain: "Eva Jansen\neva@northwind.example",
        },
    }
}
