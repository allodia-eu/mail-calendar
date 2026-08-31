//! The Portuguese seed of the showcase (screenshot) dataset — the twin of `super::en`, so the
//! Portuguese store listing gets screenshots whose mail reads in the same language as the
//! chrome. Message keys, folder ids, flags, and timings match English exactly; only the
//! language differs. Folder names are the ones a Portuguese mail server would serve, and the
//! wording is European Portuguese (pt-PT), like the catalog.

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
        mailbox("inbox", "Caixa de entrada", MailboxRole::Inbox),
        mailbox("sent", "Enviados", MailboxRole::Sent),
        mailbox("archive", "Arquivo", MailboxRole::Archive),
        mailbox("drafts", "Rascunhos", MailboxRole::Drafts),
    ];
    let messages: Vec<Message> = vec![
        msg("p-welcome", "inbox").from("Allodia", "hello@allodia.eu").to(me.0, me.1)
            .subject("Bem-vinda ao Allodia Mail & Calendar")
            .preview("Está tudo pronto. Veja como tornar o Allodia Mail & Calendar seu: ligue uma conta, escolha a profundidade de sincronização e mantenha o controlo.")
            .at(ago(now, 35)).id("welcome-1@allodia.eu").done(),
        // A three-message conversation: Tom's request, Eva's Sent reply, Tom's follow-up.
        msg("p-launch-1", "inbox").from("Tom de Vries", "tom.devries@northwind.example").to(me.0, me.1)
            .subject("Lançamento do 3.º trimestre — decisão final")
            .preview("Podes dar uma última vista de olhos à lista de verificação antes de fecharmos a quinta-feira? Queria sobretudo o teu aval ao plano de reversão.")
            .at(ago(now, 150)).id("launch-1@northwind.example").done(),
        msg("p-launch-3", "inbox").from("Tom de Vries", "tom.devries@northwind.example").to(me.0, me.1)
            .subject("Re: Lançamento do 3.º trimestre — decisão final")
            .preview("Perfeito — obrigado pela resposta rápida. Avançamos na quinta-feira. Eu aviso a equipa.")
            .at(ago(now, 95)).id("launch-3@northwind.example")
            .thread("launch-1@northwind.example", &["launch-1@northwind.example", "launch-2@example.com"]).done(),
        // The meeting invitation: an iTIP REQUEST whose calendar hold is already in the diary,
        // clashing with Thursday's board meeting (`super::invitation`).
        msg("p-invite", "inbox").from(invite_from.0, invite_from.1).to(me.0, me.1)
            .subject("Convite: arranque da parceria — quinta-feira 14:30")
            .preview("Está convidada para o arranque da parceria. Diga-me se quinta-feira à tarde lhe dá jeito.")
            .at(ago(now, 55)).id("kickoff-1@northwind.example").done(),
        msg("p-report", "inbox").from("Example Cloud", "reports@example.org").to(me.0, me.1)
            .subject("O seu relatório de utilização de junho")
            .preview("Obrigado por usar a Example Cloud. O seu relatório de utilização mensal segue em anexo, em CSV.")
            .at(ago(now, 300)).seen().attachment().id("report-1@example.org").done(),
        msg("p-newsletter", "inbox").from("European Digital Weekly", "weekly@europeandigital.example").to(me.0, me.1)
            .subject("Esta semana na tecnologia europeia")
            .preview("As regras de soberania dão mais um passo, e três ferramentas que estamos a seguir este mês.")
            .at(ago(now, 1445)).seen().id("news-1@europeandigital.example").done(),
        msg("p-contract", "inbox").from("Northwind Legal", "legal@northwind.example").to(me.0, me.1)
            .subject("Para assinar: acordo de parceria")
            .preview("A versão final está pronta para a sua assinatura. Diga-nos se houver algo a alterar.")
            .at(ago(now, 1600)).seen().flagged().id("contract-1@northwind.example").done(),
        msg("p-shipping", "inbox").from("De Fietswinkel", "orders@fietswinkel.example").to(me.0, me.1)
            .subject("A sua encomenda foi expedida")
            .preview("Boas notícias: a sua encomenda está a caminho. Entrega prevista: amanhã, entre as 9:00 e as 17:00.")
            .at(ago(now, 2880)).seen().id("ship-1@fietswinkel.example").done(),
        // Eva's own Sent reply on the thread (rides in the conversation with a "Sent" badge).
        msg("p-launch-2", "sent").from(me.0, me.1).to("Tom de Vries", "tom.devries@northwind.example")
            .subject("Re: Lançamento do 3.º trimestre — decisão final")
            .preview("A lista de verificação está sólida. Um ajuste ao plano de reversão e, da minha parte, podemos avançar.")
            .at(ago(now, 120)).seen().id("launch-2@example.com")
            .thread("launch-1@northwind.example", &["launch-1@northwind.example"]).done(),
        msg("p-sent-lunch", "sent").from(me.0, me.1).to("Sofia Ruiz", "sofia.ruiz@northwind.example")
            .subject("Almoçamos na quinta-feira?")
            .preview("Apetece-te almoçar na quinta-feira, depois da revisão? Abriu um sítio novo ao pé do escritório.")
            .at(ago(now, 180)).seen().id("lunch-1@example.com").done(),
        msg("p-archive-notes", "archive").from("Sofia Ruiz", "sofia.ruiz@northwind.example").to(me.0, me.1)
            .subject("Notas do encontro fora do escritório")
            .preview("Foram dois dias muito bons — aqui ficam o resumo e as ações que combinámos.")
            .at(ago(now, 8640)).seen().id("offsite-1@northwind.example").done(),
        msg("p-draft-board", "drafts").from(me.0, me.1).to("Direção", "board@northwind.example")
            .subject("Ponto de situação do 3.º trimestre para a direção")
            .preview("Rascunho — os números principais, o estado do lançamento e as duas decisões que preciso da direção.")
            .at(ago(now, 240)).draft().id("boarddraft-1@example.com").done(),
    ];
    seed(ShowcaseLocale::Pt, me.1, mailboxes, messages, now)
}

pub(super) fn secondary(now: OffsetDateTime) -> AccountSeed {
    let me = ("Eva Jansen", "eva@northwind.example");
    let mailboxes: Vec<Mailbox> = vec![
        mailbox("inbox", "Caixa de entrada", MailboxRole::Inbox),
        mailbox("sent", "Enviados", MailboxRole::Sent),
    ];
    let messages: Vec<Message> = vec![
        msg("w-welcome", "inbox").from("Northwind People", "hr@northwind.example").to(me.0, me.1)
            .subject("Bem-vinda à sua primeira semana")
            .preview("Ainda bem que está connosco. Tudo o que precisa para começar bem está no espaço de integração.")
            .at(ago(now, 240)).seen().id("hr-1@northwind.example").done(),
        msg("w-2fa", "inbox").from("Northwind IT", "it@northwind.example").to(me.0, me.1)
            .subject("Ação necessária: ative o início de sessão em dois passos")
            .preview("Ative a verificação em dois passos na sua conta antes de sexta-feira. Demora cerca de dois minutos.")
            .at(ago(now, 1400)).flagged().id("it-1@northwind.example").done(),
        msg("w-review", "inbox").from("Sofia Ruiz", "sofia.ruiz@northwind.example").to(me.0, me.1)
            .subject("Revisão de design adiada para sexta-feira")
            .preview("Um aviso: passámos a revisão de design para sexta-feira às 11:00, para que todos possam participar.")
            .at(ago(now, 2880)).seen().id("review-1@northwind.example").done(),
        msg("w-allhands", "inbox").from("Northwind", "allhands@northwind.example").to(me.0, me.1)
            .subject("Resumo e diapositivos da reunião geral")
            .preview("Perdeu a reunião geral? Aqui ficam a gravação e a apresentação.")
            .at(ago(now, 4320)).seen().id("ah-1@northwind.example").done(),
    ];
    seed(ShowcaseLocale::Pt, me.1, mailboxes, messages, now)
}

/// The three showcase calendars' names and a full week of events spread across them —
/// the same structure as `super::en` (identical keys, calendars, weekdays, times and
/// durations, including the Tuesday clash and the Sunday all-day overflow that exercise
/// column packing and the "+N more" banner); only the language differs.
pub(super) fn calendar(now: OffsetDateTime) -> (CalendarNames, Vec<Event>) {
    let events = vec![
        event(
            "ev-mon-standup",
            "Reunião diária",
            Cal::Work,
            zoned_wd(now, MON, 9, 30),
            15,
        ),
        event(
            "ev-mon-sprint",
            "Planeamento do sprint",
            Cal::Work,
            zoned_wd(now, MON, 11, 0),
            60,
        ),
        event(
            "ev-mon-pickup",
            "Ir buscar à escola",
            Cal::Family,
            zoned_wd(now, MON, 15, 30),
            30,
        ),
        event(
            "ev-mon-designsync",
            "Alinhamento de design",
            Cal::Work,
            zoned_wd(now, MON, 16, 0),
            45,
        ),
        event(
            "ev-mon-gym",
            "Ginásio",
            Cal::Personal,
            zoned_wd(now, MON, 18, 30),
            60,
        ),
        event(
            "ev-tue-standup",
            "Reunião diária",
            Cal::Work,
            zoned_wd(now, TUE, 9, 30),
            15,
        ),
        event(
            "ev-tue-triage",
            "Triagem de bugs",
            Cal::Work,
            zoned_wd(now, TUE, 10, 0),
            60,
        ),
        event(
            "ev-tue-design",
            "Revisão de design",
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
            "Almoço com a Sofia",
            Cal::Personal,
            zoned_wd(now, TUE, 12, 30),
            60,
        ),
        event(
            "ev-tue-1on1",
            "1:1 com a Priya",
            Cal::Work,
            zoned_wd(now, TUE, 15, 0),
            30,
        ),
        event(
            "ev-tue-bookclub",
            "Clube de leitura",
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
            "Reunião diária",
            Cal::Work,
            zoned_wd(now, WED, 9, 30),
            15,
        ),
        event(
            "ev-wed-review",
            "Revisão do lançamento Q3",
            Cal::Work,
            zoned_wd(now, WED, 13, 0),
            90,
        ),
        event(
            "ev-wed-football",
            "Treino de futebol",
            Cal::Family,
            zoned_wd(now, WED, 16, 30),
            60,
        ),
        event(
            "ev-wed-dinner",
            "Jantar com amigos",
            Cal::Personal,
            zoned_wd(now, WED, 19, 0),
            120,
        ),
        event(
            "ev-thu-standup",
            "Reunião diária",
            Cal::Work,
            zoned_wd(now, THU, 9, 30),
            15,
        ),
        event(
            "ev-thu-roadmap",
            "Workshop de roadmap",
            Cal::Work,
            zoned_wd(now, THU, 11, 0),
            90,
        ),
        event(
            "ev-thu-board",
            "Reunião da direção",
            Cal::Work,
            zoned_wd(now, THU, 14, 0),
            120,
        ),
        event(
            "ev-thu-piano",
            "Aula de piano",
            Cal::Family,
            zoned_wd(now, THU, 17, 30),
            45,
        ),
        event(
            "ev-thu-parents",
            "Chamada com os pais",
            Cal::Family,
            zoned_wd(now, THU, 20, 0),
            45,
        ),
        event(
            "ev-fri-standup",
            "Reunião diária",
            Cal::Work,
            zoned_wd(now, FRI, 9, 30),
            15,
        ),
        event(
            "ev-fri-lunchlearn",
            "Almoço formativo",
            Cal::Work,
            zoned_wd(now, FRI, 12, 30),
            60,
        ),
        event(
            "ev-fri-demo",
            "Demo do sprint",
            Cal::Work,
            zoned_wd(now, FRI, 15, 0),
            45,
        ),
        event(
            "ev-fri-haircut",
            "Cabeleireiro",
            Cal::Personal,
            zoned_wd(now, FRI, 17, 0),
            30,
        ),
        event(
            "ev-fri-datenight",
            "Noite a dois",
            Cal::Personal,
            zoned_wd(now, FRI, 20, 0),
            120,
        ),
        event(
            "ev-sat-market",
            "Mercado de produtores",
            Cal::Personal,
            zoned_wd(now, SAT, 10, 0),
            90,
        ),
        event(
            "ev-sat-birthday",
            "Aniversário da avó",
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
            "Brunch com a Sofia",
            Cal::Personal,
            zoned_wd(now, SUN, 11, 0),
            90,
        ),
        event(
            "ev-sun-1on1",
            "1:1 com o Tom",
            Cal::Work,
            zoned_wd(now, SUN, 15, 0),
            30,
        ),
        event(
            "ev-sun-walk",
            "Passeio em família",
            Cal::Family,
            zoned_wd(now, SUN, 16, 0),
            90,
        ),
        event(
            "ev-sun-mealprep",
            "Preparar as refeições",
            Cal::Personal,
            zoned_wd(now, SUN, 18, 0),
            45,
        ),
        all_day_event(
            "ev-ad-offsite",
            "Encontro da equipa",
            Cal::Work,
            now,
            WED,
            2,
        ),
        all_day_event("ev-ad-leave", "Lisa ausente", Cal::Work, now, MON, 3),
        all_day_event("ev-ad-release", "Dia do lançamento", Cal::Work, now, FRI, 1),
        all_day_event(
            "ev-ad-visiting",
            "Visita dos pais",
            Cal::Family,
            now,
            SAT,
            2,
        ),
        all_day_event(
            "ev-ad-birthday",
            "Aniversário do Tom",
            Cal::Family,
            now,
            SUN,
            1,
        ),
        all_day_event("ev-ad-holiday", "Feriado", Cal::Personal, now, SUN, 1),
        all_day_event(
            "ev-ad-marathon",
            "Meia maratona",
            Cal::Personal,
            now,
            SUN,
            1,
        ),
    ];
    (
        CalendarNames {
            work: "Trabalho",
            personal: "Pessoal",
            family: "Família",
        },
        events,
    )
}

/// The meeting invitation's title, room and notes. See [`super::invite_text`].
pub(super) fn invite_text() -> InviteText {
    InviteText {
        summary: "Arranque da parceria",
        location: "Sala de reuniões Amstel · Amesterdão",
        description: "Uma hora para acordar o âmbito, o calendário e quem fica com o quê. \
                      Traga o rascunho do plano — vemo-lo em conjunto.",
    }
}

/// The sample text the showcase reply composer opens pre-filled with — Eva's answer to Northwind
/// Legal's signature request (`p-contract`). See [`super::showcase_reply`].
pub(super) fn reply_text() -> &'static str {
    "Obrigada — li tudo e, da minha parte, está tudo em ordem. \
     Assino esta tarde e devolvo hoje mesmo a versão assinada."
}

/// The two signatures the showcase library holds. See [`super::signatures`].
pub(super) fn signatures() -> super::ShowcaseSignatures {
    super::ShowcaseSignatures {
        primary: super::SignatureSeed {
            name: "Trabalho",
            body_html: "<div><b>Eva Jansen</b></div><div>Responsável de produto · Northwind</div>\
                        <div>eva.jansen@example.com</div>",
            body_plain: "Eva Jansen\nResponsável de produto · Northwind\neva.jansen@example.com",
        },
        secondary: super::SignatureSeed {
            name: "Curta",
            body_html: "<div>Eva Jansen</div><div>eva@northwind.example</div>",
            body_plain: "Eva Jansen\neva@northwind.example",
        },
    }
}
