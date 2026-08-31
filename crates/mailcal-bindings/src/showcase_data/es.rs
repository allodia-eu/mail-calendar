//! The Spanish seed of the showcase (screenshot) dataset — the twin of `super::en`, so the
//! Spanish store listing gets screenshots whose mail reads in the same language as the chrome.
//! Message keys, folder ids, flags, and timings match English exactly; only the language
//! differs. Folder names are the Spanish ones a Spanish mail server would serve.

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
        mailbox("inbox", "Bandeja de entrada", MailboxRole::Inbox),
        mailbox("sent", "Enviados", MailboxRole::Sent),
        mailbox("archive", "Archivo", MailboxRole::Archive),
        mailbox("drafts", "Borradores", MailboxRole::Drafts),
    ];
    let messages: Vec<Message> = vec![
        msg("p-welcome", "inbox").from("Allodia", "hello@allodia.eu").to(me.0, me.1)
            .subject("Te damos la bienvenida a Allodia Mail & Calendar")
            .preview("Ya está todo listo. Así puedes hacer tuyo Allodia Mail & Calendar: conecta una cuenta, elige la profundidad de sincronización y toma el control.")
            .at(ago(now, 35)).id("welcome-1@allodia.eu").done(),
        // A three-message conversation: Tom's request, Eva's Sent reply, Tom's follow-up.
        msg("p-launch-1", "inbox").from("Tom de Vries", "tom.devries@northwind.example").to(me.0, me.1)
            .subject("Lanzamiento del T3: decisión final")
            .preview("¿Puedes echar un último vistazo a la lista de comprobación antes de cerrar el jueves? Sobre todo quiero tu visto bueno al plan de reversión.")
            .at(ago(now, 150)).id("launch-1@northwind.example").done(),
        msg("p-launch-3", "inbox").from("Tom de Vries", "tom.devries@northwind.example").to(me.0, me.1)
            .subject("Re: Lanzamiento del T3: decisión final")
            .preview("Perfecto, gracias por responder tan rápido. El jueves salimos. Aviso al equipo.")
            .at(ago(now, 95)).id("launch-3@northwind.example")
            .thread("launch-1@northwind.example", &["launch-1@northwind.example", "launch-2@example.com"]).done(),
        // The meeting invitation: an iTIP REQUEST whose calendar hold is already in the diary,
        // clashing with Thursday's board meeting (`super::invitation`).
        msg("p-invite", "inbox").from(invite_from.0, invite_from.1).to(me.0, me.1)
            .subject("Invitación: arranque de la colaboración — jueves 14:30")
            .preview("Te invitamos al arranque de la colaboración. Dime si el jueves por la tarde te viene bien.")
            .at(ago(now, 55)).id("kickoff-1@northwind.example").done(),
        msg("p-report", "inbox").from("Example Cloud", "reports@example.org").to(me.0, me.1)
            .subject("Tu informe de uso de junio")
            .preview("Gracias por usar Example Cloud. Adjuntamos tu informe de uso mensual en formato CSV.")
            .at(ago(now, 300)).seen().attachment().id("report-1@example.org").done(),
        msg("p-newsletter", "inbox").from("European Digital Weekly", "weekly@europeandigital.example").to(me.0, me.1)
            .subject("Esta semana en la tecnología europea")
            .preview("Las normas de soberanía avanzan un paso más, y tres herramientas que seguimos este mes.")
            .at(ago(now, 1445)).seen().id("news-1@europeandigital.example").done(),
        msg("p-contract", "inbox").from("Northwind Legal", "legal@northwind.example").to(me.0, me.1)
            .subject("Para firmar: acuerdo de colaboración")
            .preview("La versión definitiva está lista para tu firma. Dinos si hay que cambiar algo.")
            .at(ago(now, 1600)).seen().flagged().id("contract-1@northwind.example").done(),
        msg("p-shipping", "inbox").from("De Fietswinkel", "orders@fietswinkel.example").to(me.0, me.1)
            .subject("Tu pedido va de camino")
            .preview("Buenas noticias: tu pedido ya está en camino. Entrega prevista: mañana entre las 9:00 y las 17:00.")
            .at(ago(now, 2880)).seen().id("ship-1@fietswinkel.example").done(),
        // Eva's own Sent reply on the thread (rides in the conversation with a "Sent" badge).
        msg("p-launch-2", "sent").from(me.0, me.1).to("Tom de Vries", "tom.devries@northwind.example")
            .subject("Re: Lanzamiento del T3: decisión final")
            .preview("La lista de comprobación tiene buena pinta. Un ajuste en el plan de reversión y, por mi parte, adelante.")
            .at(ago(now, 120)).seen().id("launch-2@example.com")
            .thread("launch-1@northwind.example", &["launch-1@northwind.example"]).done(),
        msg("p-sent-lunch", "sent").from(me.0, me.1).to("Sofia Ruiz", "sofia.ruiz@northwind.example")
            .subject("¿Comemos el jueves?")
            .preview("¿Te apetece comer el jueves después de la revisión? Han abierto un sitio nuevo al lado de la oficina.")
            .at(ago(now, 180)).seen().id("lunch-1@example.com").done(),
        msg("p-archive-notes", "archive").from("Sofia Ruiz", "sofia.ruiz@northwind.example").to(me.0, me.1)
            .subject("Notas de la jornada de trabajo")
            .preview("Han sido dos días estupendos. Aquí tienes el resumen y las tareas que acordamos.")
            .at(ago(now, 8640)).seen().id("offsite-1@northwind.example").done(),
        msg("p-draft-board", "drafts").from(me.0, me.1).to("Consejo", "board@northwind.example")
            .subject("Informe del T3 para el consejo")
            .preview("Borrador: las cifras principales, el estado del lanzamiento y las dos decisiones que necesito del consejo.")
            .at(ago(now, 240)).draft().id("boarddraft-1@example.com").done(),
    ];
    seed(ShowcaseLocale::Es, me.1, mailboxes, messages, now)
}

pub(super) fn secondary(now: OffsetDateTime) -> AccountSeed {
    let me = ("Eva Jansen", "eva@northwind.example");
    let mailboxes: Vec<Mailbox> = vec![
        mailbox("inbox", "Bandeja de entrada", MailboxRole::Inbox),
        mailbox("sent", "Enviados", MailboxRole::Sent),
    ];
    let messages: Vec<Message> = vec![
        msg("w-welcome", "inbox").from("Northwind People", "hr@northwind.example").to(me.0, me.1)
            .subject("Bienvenida a tu primera semana")
            .preview("Nos alegra tenerte aquí. Todo lo que necesitas para empezar con buen pie está en el espacio de incorporación.")
            .at(ago(now, 240)).seen().id("hr-1@northwind.example").done(),
        msg("w-2fa", "inbox").from("Northwind IT", "it@northwind.example").to(me.0, me.1)
            .subject("Acción necesaria: activa el inicio de sesión en dos pasos")
            .preview("Activa la verificación en dos pasos en tu cuenta antes del viernes. Se tarda unos dos minutos.")
            .at(ago(now, 1400)).flagged().id("it-1@northwind.example").done(),
        msg("w-review", "inbox").from("Sofia Ruiz", "sofia.ruiz@northwind.example").to(me.0, me.1)
            .subject("La revisión de diseño se pasa al viernes")
            .preview("Aviso: hemos movido la revisión de diseño al viernes a las 11:00 para que pueda venir todo el mundo.")
            .at(ago(now, 2880)).seen().id("review-1@northwind.example").done(),
        msg("w-allhands", "inbox").from("Northwind", "allhands@northwind.example").to(me.0, me.1)
            .subject("Resumen y diapositivas de la reunión general")
            .preview("¿Te perdiste la reunión general? Aquí tienes la grabación y la presentación.")
            .at(ago(now, 4320)).seen().id("ah-1@northwind.example").done(),
    ];
    seed(ShowcaseLocale::Es, me.1, mailboxes, messages, now)
}

/// The three showcase calendars' names and a full week of events spread across them —
/// the same structure as `super::en` (identical keys, calendars, weekdays, times and
/// durations, including the Tuesday clash and the Sunday all-day overflow that exercise
/// column packing and the "+N more" banner); only the language differs.
pub(super) fn calendar(now: OffsetDateTime) -> (CalendarNames, Vec<Event>) {
    let events = vec![
        event(
            "ev-mon-standup",
            "Reunión diaria",
            Cal::Work,
            zoned_wd(now, MON, 9, 30),
            15,
        ),
        event(
            "ev-mon-sprint",
            "Planificación del sprint",
            Cal::Work,
            zoned_wd(now, MON, 11, 0),
            60,
        ),
        event(
            "ev-mon-pickup",
            "Recoger a los niños",
            Cal::Family,
            zoned_wd(now, MON, 15, 30),
            30,
        ),
        event(
            "ev-mon-designsync",
            "Reunión de diseño",
            Cal::Work,
            zoned_wd(now, MON, 16, 0),
            45,
        ),
        event(
            "ev-mon-gym",
            "Gimnasio",
            Cal::Personal,
            zoned_wd(now, MON, 18, 30),
            60,
        ),
        event(
            "ev-tue-standup",
            "Reunión diaria",
            Cal::Work,
            zoned_wd(now, TUE, 9, 30),
            15,
        ),
        event(
            "ev-tue-triage",
            "Triaje de errores",
            Cal::Work,
            zoned_wd(now, TUE, 10, 0),
            60,
        ),
        event(
            "ev-tue-design",
            "Revisión de diseño",
            Cal::Work,
            zoned_wd(now, TUE, 10, 15),
            45,
        ),
        event(
            "ev-tue-interview",
            "Entrevista: backend",
            Cal::Work,
            zoned_wd(now, TUE, 10, 30),
            60,
        ),
        event(
            "ev-tue-lunch",
            "Comida con Sofía",
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
            "Club de lectura",
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
            "Reunión diaria",
            Cal::Work,
            zoned_wd(now, WED, 9, 30),
            15,
        ),
        event(
            "ev-wed-review",
            "Revisión del lanzamiento del T3",
            Cal::Work,
            zoned_wd(now, WED, 13, 0),
            90,
        ),
        event(
            "ev-wed-football",
            "Entrenamiento de fútbol",
            Cal::Family,
            zoned_wd(now, WED, 16, 30),
            60,
        ),
        event(
            "ev-wed-dinner",
            "Cena con amigos",
            Cal::Personal,
            zoned_wd(now, WED, 19, 0),
            120,
        ),
        event(
            "ev-thu-standup",
            "Reunión diaria",
            Cal::Work,
            zoned_wd(now, THU, 9, 30),
            15,
        ),
        event(
            "ev-thu-roadmap",
            "Taller de hoja de ruta",
            Cal::Work,
            zoned_wd(now, THU, 11, 0),
            90,
        ),
        event(
            "ev-thu-board",
            "Reunión del consejo",
            Cal::Work,
            zoned_wd(now, THU, 14, 0),
            120,
        ),
        event(
            "ev-thu-piano",
            "Clase de piano",
            Cal::Family,
            zoned_wd(now, THU, 17, 30),
            45,
        ),
        event(
            "ev-thu-parents",
            "Llamar a mis padres",
            Cal::Family,
            zoned_wd(now, THU, 20, 0),
            45,
        ),
        event(
            "ev-fri-standup",
            "Reunión diaria",
            Cal::Work,
            zoned_wd(now, FRI, 9, 30),
            15,
        ),
        event(
            "ev-fri-lunchlearn",
            "Comida formativa",
            Cal::Work,
            zoned_wd(now, FRI, 12, 30),
            60,
        ),
        event(
            "ev-fri-demo",
            "Demo del sprint",
            Cal::Work,
            zoned_wd(now, FRI, 15, 0),
            45,
        ),
        event(
            "ev-fri-haircut",
            "Peluquería",
            Cal::Personal,
            zoned_wd(now, FRI, 17, 0),
            30,
        ),
        event(
            "ev-fri-datenight",
            "Noche de pareja",
            Cal::Personal,
            zoned_wd(now, FRI, 20, 0),
            120,
        ),
        event(
            "ev-sat-market",
            "Mercadillo",
            Cal::Personal,
            zoned_wd(now, SAT, 10, 0),
            90,
        ),
        event(
            "ev-sat-birthday",
            "Cumpleaños de la abuela",
            Cal::Family,
            zoned_wd(now, SAT, 14, 0),
            150,
        ),
        event(
            "ev-sat-cinema",
            "Cine",
            Cal::Personal,
            zoned_wd(now, SAT, 19, 30),
            150,
        ),
        event(
            "ev-sun-brunch",
            "Brunch con Sofía",
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
            "Paseo en familia",
            Cal::Family,
            zoned_wd(now, SUN, 16, 0),
            90,
        ),
        event(
            "ev-sun-mealprep",
            "Cocinar para la semana",
            Cal::Personal,
            zoned_wd(now, SUN, 18, 0),
            45,
        ),
        all_day_event("ev-ad-offsite", "Jornada de equipo", Cal::Work, now, WED, 2),
        all_day_event("ev-ad-leave", "Lisa ausente", Cal::Work, now, MON, 3),
        all_day_event(
            "ev-ad-release",
            "Día de lanzamiento",
            Cal::Work,
            now,
            FRI,
            1,
        ),
        all_day_event(
            "ev-ad-visiting",
            "Visita de mis padres",
            Cal::Family,
            now,
            SAT,
            2,
        ),
        all_day_event(
            "ev-ad-birthday",
            "Cumpleaños de Tom",
            Cal::Family,
            now,
            SUN,
            1,
        ),
        all_day_event("ev-ad-holiday", "Día festivo", Cal::Personal, now, SUN, 1),
        all_day_event(
            "ev-ad-marathon",
            "Media maratón",
            Cal::Personal,
            now,
            SUN,
            1,
        ),
    ];
    (
        CalendarNames {
            work: "Trabajo",
            personal: "Personal",
            family: "Familia",
        },
        events,
    )
}

/// The meeting invitation's title, room and notes. See [`super::invite_text`].
pub(super) fn invite_text() -> InviteText {
    InviteText {
        summary: "Arranque de la colaboración",
        location: "Sala de reuniones Amstel · Ámsterdam",
        description: "Una hora para acordar el alcance, el calendario y quién se encarga de qué. \
                      Trae el borrador del plan: lo repasamos juntos.",
    }
}

/// The sample text the showcase reply composer opens pre-filled with — Eva's answer to Northwind
/// Legal's signature request (`p-contract`). See [`super::showcase_reply`].
pub(super) fn reply_text() -> &'static str {
    "Gracias: lo he leído entero y me parece todo correcto. \
     Lo firmo esta tarde y os devuelvo hoy mismo la copia firmada."
}

/// The two signatures the showcase library holds. See [`super::signatures`].
pub(super) fn signatures() -> super::ShowcaseSignatures {
    super::ShowcaseSignatures {
        primary: super::SignatureSeed {
            name: "Trabajo",
            body_html: "<div><b>Eva Jansen</b></div><div>Responsable de producto · Northwind</div>\
                        <div>eva.jansen@example.com</div>",
            body_plain: "Eva Jansen\nResponsable de producto · Northwind\neva.jansen@example.com",
        },
        secondary: super::SignatureSeed {
            name: "Corta",
            body_html: "<div>Eva Jansen</div><div>eva@northwind.example</div>",
            body_plain: "Eva Jansen\neva@northwind.example",
        },
    }
}
