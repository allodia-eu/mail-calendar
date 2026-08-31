//! The Italian seed of the showcase (screenshot) dataset — the twin of `super::en`, so the
//! Italian store listing gets screenshots whose mail reads in the same language as the chrome.
//! Message keys, folder ids, flags, and timings match English exactly; only the language
//! differs. Folder names are the Italian ones an Italian mail server would serve.

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
        mailbox("inbox", "Posta in arrivo", MailboxRole::Inbox),
        mailbox("sent", "Posta inviata", MailboxRole::Sent),
        mailbox("archive", "Archivio", MailboxRole::Archive),
        mailbox("drafts", "Bozze", MailboxRole::Drafts),
    ];
    let messages: Vec<Message> = vec![
        msg("p-welcome", "inbox").from("Allodia", "hello@allodia.eu").to(me.0, me.1)
            .subject("Ti diamo il benvenuto in Allodia Mail & Calendar")
            .preview("È tutto pronto. Ecco come rendere Allodia Mail & Calendar davvero tuo: collega un account, scegli la profondità di sincronizzazione e mantieni il controllo.")
            .at(ago(now, 35)).id("welcome-1@allodia.eu").done(),
        // A three-message conversation: Tom's request, Eva's Sent reply, Tom's follow-up.
        msg("p-launch-1", "inbox").from("Tom de Vries", "tom.devries@northwind.example").to(me.0, me.1)
            .subject("Lancio del Q3 — decisione finale")
            .preview("Puoi dare un’ultima occhiata alla checklist prima che fissiamo giovedì? Mi interessa soprattutto il tuo via libera sul piano di rollback.")
            .at(ago(now, 150)).id("launch-1@northwind.example").done(),
        msg("p-launch-3", "inbox").from("Tom de Vries", "tom.devries@northwind.example").to(me.0, me.1)
            .subject("Re: Lancio del Q3 — decisione finale")
            .preview("Perfetto — grazie per la risposta rapida. Giovedì si parte. Avviso io il team.")
            .at(ago(now, 95)).id("launch-3@northwind.example")
            .thread("launch-1@northwind.example", &["launch-1@northwind.example", "launch-2@example.com"]).done(),
        // The meeting invitation: an iTIP REQUEST whose calendar hold is already in the diary,
        // clashing with Thursday's board meeting (`super::invitation`).
        msg("p-invite", "inbox").from(invite_from.0, invite_from.1).to(me.0, me.1)
            .subject("Invito: avvio della collaborazione — giovedì 14:30")
            .preview("Sei invitata all’avvio della collaborazione. Fammi sapere se giovedì pomeriggio ti va bene.")
            .at(ago(now, 55)).id("kickoff-1@northwind.example").done(),
        msg("p-report", "inbox").from("Example Cloud", "reports@example.org").to(me.0, me.1)
            .subject("Il tuo report di utilizzo di giugno")
            .preview("Grazie per aver scelto Example Cloud. Il report di utilizzo mensile è allegato in formato CSV.")
            .at(ago(now, 300)).seen().attachment().id("report-1@example.org").done(),
        msg("p-newsletter", "inbox").from("European Digital Weekly", "weekly@europeandigital.example").to(me.0, me.1)
            .subject("Questa settimana nel tech europeo")
            .preview("Le regole sulla sovranità fanno un passo avanti, più tre strumenti che stiamo seguendo questo mese.")
            .at(ago(now, 1445)).seen().id("news-1@europeandigital.example").done(),
        msg("p-contract", "inbox").from("Northwind Legal", "legal@northwind.example").to(me.0, me.1)
            .subject("Da firmare: accordo di partnership")
            .preview("La versione definitiva è pronta per la tua firma. Facci sapere se c’è ancora qualcosa da modificare.")
            .at(ago(now, 1600)).seen().flagged().id("contract-1@northwind.example").done(),
        msg("p-shipping", "inbox").from("De Fietswinkel", "orders@fietswinkel.example").to(me.0, me.1)
            .subject("Il tuo ordine è stato spedito")
            .preview("Buone notizie: il tuo ordine è in viaggio. Consegna prevista: domani tra le 9:00 e le 17:00.")
            .at(ago(now, 2880)).seen().id("ship-1@fietswinkel.example").done(),
        // Eva's own Sent reply on the thread (rides in the conversation with a "Sent" badge).
        msg("p-launch-2", "sent").from(me.0, me.1).to("Tom de Vries", "tom.devries@northwind.example")
            .subject("Re: Lancio del Q3 — decisione finale")
            .preview("La checklist regge. Una modifica al piano di rollback e per me si può partire.")
            .at(ago(now, 120)).seen().id("launch-2@example.com")
            .thread("launch-1@northwind.example", &["launch-1@northwind.example"]).done(),
        msg("p-sent-lunch", "sent").from(me.0, me.1).to("Sofia Ruiz", "sofia.ruiz@northwind.example")
            .subject("Pranzo giovedì?")
            .preview("Ti va di pranzare giovedì dopo la revisione? Ha aperto un posto nuovo vicino all’ufficio.")
            .at(ago(now, 180)).seen().id("lunch-1@example.com").done(),
        msg("p-archive-notes", "archive").from("Sofia Ruiz", "sofia.ruiz@northwind.example").to(me.0, me.1)
            .subject("Appunti dell’offsite")
            .preview("Due giornate davvero buone — ecco il riepilogo e le azioni che abbiamo concordato.")
            .at(ago(now, 8640)).seen().id("offsite-1@northwind.example").done(),
        msg("p-draft-board", "drafts").from(me.0, me.1).to("Consiglio", "board@northwind.example")
            .subject("Aggiornamento Q3 per il consiglio")
            .preview("Bozza — i numeri principali, lo stato del lancio e le due decisioni che mi servono dal consiglio.")
            .at(ago(now, 240)).draft().id("boarddraft-1@example.com").done(),
    ];
    seed(ShowcaseLocale::It, me.1, mailboxes, messages, now)
}

pub(super) fn secondary(now: OffsetDateTime) -> AccountSeed {
    let me = ("Eva Jansen", "eva@northwind.example");
    let mailboxes: Vec<Mailbox> = vec![
        mailbox("inbox", "Posta in arrivo", MailboxRole::Inbox),
        mailbox("sent", "Posta inviata", MailboxRole::Sent),
    ];
    let messages: Vec<Message> = vec![
        msg("w-welcome", "inbox").from("Northwind People", "hr@northwind.example").to(me.0, me.1)
            .subject("Benvenuta nella tua prima settimana")
            .preview("Siamo felici di averti con noi. Tutto ciò che ti serve per partire bene è nello spazio di onboarding.")
            .at(ago(now, 240)).seen().id("hr-1@northwind.example").done(),
        msg("w-2fa", "inbox").from("Northwind IT", "it@northwind.example").to(me.0, me.1)
            .subject("Azione necessaria: attiva l’accesso in due passaggi")
            .preview("Attiva la verifica in due passaggi sul tuo account entro venerdì. Bastano circa due minuti.")
            .at(ago(now, 1400)).flagged().id("it-1@northwind.example").done(),
        msg("w-review", "inbox").from("Sofia Ruiz", "sofia.ruiz@northwind.example").to(me.0, me.1)
            .subject("Revisione del design spostata a venerdì")
            .preview("Un avviso: abbiamo spostato la revisione del design a venerdì alle 11:00 così possono esserci tutti.")
            .at(ago(now, 2880)).seen().id("review-1@northwind.example").done(),
        msg("w-allhands", "inbox").from("Northwind", "allhands@northwind.example").to(me.0, me.1)
            .subject("Riepilogo e slide dell’all-hands")
            .preview("Ti sei persa l’all-hands? Ecco la registrazione e le slide.")
            .at(ago(now, 4320)).seen().id("ah-1@northwind.example").done(),
    ];
    seed(ShowcaseLocale::It, me.1, mailboxes, messages, now)
}

/// The three showcase calendars' names and a full week of events spread across them —
/// the same structure as `super::en` (identical keys, calendars, weekdays, times and
/// durations, including the Tuesday clash and the Sunday all-day overflow that exercise
/// column packing and the "+N more" banner); only the language differs.
pub(super) fn calendar(now: OffsetDateTime) -> (CalendarNames, Vec<Event>) {
    let events = vec![
        event(
            "ev-mon-standup",
            "Standup del team",
            Cal::Work,
            zoned_wd(now, MON, 9, 30),
            15,
        ),
        event(
            "ev-mon-sprint",
            "Pianificazione dello sprint",
            Cal::Work,
            zoned_wd(now, MON, 11, 0),
            60,
        ),
        event(
            "ev-mon-pickup",
            "Uscita da scuola",
            Cal::Family,
            zoned_wd(now, MON, 15, 30),
            30,
        ),
        event(
            "ev-mon-designsync",
            "Allineamento design",
            Cal::Work,
            zoned_wd(now, MON, 16, 0),
            45,
        ),
        event(
            "ev-mon-gym",
            "Palestra",
            Cal::Personal,
            zoned_wd(now, MON, 18, 30),
            60,
        ),
        event(
            "ev-tue-standup",
            "Standup del team",
            Cal::Work,
            zoned_wd(now, TUE, 9, 30),
            15,
        ),
        event(
            "ev-tue-triage",
            "Triage dei bug",
            Cal::Work,
            zoned_wd(now, TUE, 10, 0),
            60,
        ),
        event(
            "ev-tue-design",
            "Revisione del design",
            Cal::Work,
            zoned_wd(now, TUE, 10, 15),
            45,
        ),
        event(
            "ev-tue-interview",
            "Colloquio: backend",
            Cal::Work,
            zoned_wd(now, TUE, 10, 30),
            60,
        ),
        event(
            "ev-tue-lunch",
            "Pranzo con Sofia",
            Cal::Personal,
            zoned_wd(now, TUE, 12, 30),
            60,
        ),
        event(
            "ev-tue-1on1",
            "1:1 con Priya",
            Cal::Work,
            zoned_wd(now, TUE, 15, 0),
            30,
        ),
        event(
            "ev-tue-bookclub",
            "Gruppo di lettura",
            Cal::Personal,
            zoned_wd(now, TUE, 19, 30),
            90,
        ),
        event(
            "ev-wed-dentist",
            "Dentista",
            Cal::Personal,
            zoned_wd(now, WED, 8, 0),
            45,
        ),
        event(
            "ev-wed-standup",
            "Standup del team",
            Cal::Work,
            zoned_wd(now, WED, 9, 30),
            15,
        ),
        event(
            "ev-wed-review",
            "Revisione del lancio Q3",
            Cal::Work,
            zoned_wd(now, WED, 13, 0),
            90,
        ),
        event(
            "ev-wed-football",
            "Allenamento di calcio",
            Cal::Family,
            zoned_wd(now, WED, 16, 30),
            60,
        ),
        event(
            "ev-wed-dinner",
            "Cena con gli amici",
            Cal::Personal,
            zoned_wd(now, WED, 19, 0),
            120,
        ),
        event(
            "ev-thu-standup",
            "Standup del team",
            Cal::Work,
            zoned_wd(now, THU, 9, 30),
            15,
        ),
        event(
            "ev-thu-roadmap",
            "Workshop sulla roadmap",
            Cal::Work,
            zoned_wd(now, THU, 11, 0),
            90,
        ),
        event(
            "ev-thu-board",
            "Consiglio di amministrazione",
            Cal::Work,
            zoned_wd(now, THU, 14, 0),
            120,
        ),
        event(
            "ev-thu-piano",
            "Lezione di pianoforte",
            Cal::Family,
            zoned_wd(now, THU, 17, 30),
            45,
        ),
        event(
            "ev-thu-parents",
            "Telefonata ai genitori",
            Cal::Family,
            zoned_wd(now, THU, 20, 0),
            45,
        ),
        event(
            "ev-fri-standup",
            "Standup del team",
            Cal::Work,
            zoned_wd(now, FRI, 9, 30),
            15,
        ),
        event(
            "ev-fri-lunchlearn",
            "Pranzo formativo",
            Cal::Work,
            zoned_wd(now, FRI, 12, 30),
            60,
        ),
        event(
            "ev-fri-demo",
            "Demo dello sprint",
            Cal::Work,
            zoned_wd(now, FRI, 15, 0),
            45,
        ),
        event(
            "ev-fri-haircut",
            "Parrucchiere",
            Cal::Personal,
            zoned_wd(now, FRI, 17, 0),
            30,
        ),
        event(
            "ev-fri-datenight",
            "Serata di coppia",
            Cal::Personal,
            zoned_wd(now, FRI, 20, 0),
            120,
        ),
        event(
            "ev-sat-market",
            "Mercato contadino",
            Cal::Personal,
            zoned_wd(now, SAT, 10, 0),
            90,
        ),
        event(
            "ev-sat-birthday",
            "Compleanno della nonna",
            Cal::Family,
            zoned_wd(now, SAT, 14, 0),
            150,
        ),
        event(
            "ev-sat-cinema",
            "Cinema",
            Cal::Personal,
            zoned_wd(now, SAT, 19, 30),
            150,
        ),
        event(
            "ev-sun-brunch",
            "Brunch con Sofia",
            Cal::Personal,
            zoned_wd(now, SUN, 11, 0),
            90,
        ),
        event(
            "ev-sun-1on1",
            "1:1 con Tom",
            Cal::Work,
            zoned_wd(now, SUN, 15, 0),
            30,
        ),
        event(
            "ev-sun-walk",
            "Passeggiata in famiglia",
            Cal::Family,
            zoned_wd(now, SUN, 16, 0),
            90,
        ),
        event(
            "ev-sun-mealprep",
            "Preparazione dei pasti",
            Cal::Personal,
            zoned_wd(now, SUN, 18, 0),
            45,
        ),
        all_day_event("ev-ad-offsite", "Offsite del team", Cal::Work, now, WED, 2),
        all_day_event("ev-ad-leave", "Lisa in ferie", Cal::Work, now, MON, 3),
        all_day_event(
            "ev-ad-release",
            "Giorno del rilascio",
            Cal::Work,
            now,
            FRI,
            1,
        ),
        all_day_event(
            "ev-ad-visiting",
            "Mamma e papà in visita",
            Cal::Family,
            now,
            SAT,
            2,
        ),
        all_day_event(
            "ev-ad-birthday",
            "Compleanno di Tom",
            Cal::Family,
            now,
            SUN,
            1,
        ),
        all_day_event(
            "ev-ad-holiday",
            "Giorno festivo",
            Cal::Personal,
            now,
            SUN,
            1,
        ),
        all_day_event(
            "ev-ad-marathon",
            "Mezza maratona",
            Cal::Personal,
            now,
            SUN,
            1,
        ),
    ];
    (
        CalendarNames {
            work: "Lavoro",
            personal: "Personale",
            family: "Famiglia",
        },
        events,
    )
}

/// The meeting invitation's title, room and notes. See [`super::invite_text`].
pub(super) fn invite_text() -> InviteText {
    InviteText {
        summary: "Avvio della collaborazione",
        location: "Sala riunioni Amstel · Amsterdam",
        description: "Un’ora per concordare l’ambito, i tempi e chi fa cosa. \
                      Porta la bozza del piano: la rivediamo insieme.",
    }
}

/// The sample text the showcase reply composer opens pre-filled with — Eva's answer to Northwind
/// Legal's signature request (`p-contract`). See [`super::showcase_reply`].
pub(super) fn reply_text() -> &'static str {
    "Grazie — l’ho letto tutto e per me va bene così. \
     Lo firmo questo pomeriggio e vi rimando la copia controfirmata in giornata."
}

/// The two signatures the showcase library holds. See [`super::signatures`].
pub(super) fn signatures() -> super::ShowcaseSignatures {
    super::ShowcaseSignatures {
        primary: super::SignatureSeed {
            name: "Lavoro",
            body_html: "<div><b>Eva Jansen</b></div><div>Responsabile di prodotto · Northwind</div>\
                        <div>eva.jansen@example.com</div>",
            body_plain: "Eva Jansen\nResponsabile di prodotto · Northwind\neva.jansen@example.com",
        },
        secondary: super::SignatureSeed {
            name: "Breve",
            body_html: "<div>Eva Jansen</div><div>eva@northwind.example</div>",
            body_plain: "Eva Jansen\neva@northwind.example",
        },
    }
}
