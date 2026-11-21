//! The sent mail `MAILCAL_OFFER_AI` adds to the showcase's first account, so its review is `Ready`
//! and Learn can be pressed (`docs/ai.md` rule 11). Each message says enough to learn from, in
//! English or in Dutch. The second account's Sent folder is left as seeded, so its review is
//! `NothingUsable` and both reviews can be reached.

use engine_core::{
    ids::{MessageId, MessageIdHeader},
    mail::{EmailAddress, Keyword, MailboxRole, Message, SystemKeyword},
    membership::Memberships,
    time::UtcDateTime,
};
use time::OffsetDateTime;

use crate::showcase_data::AccountSeed;

/// Who each message went to, its subject and its words.
const SENT: [(&str, &str, &str, &str); 7] = [
    (
        "Tom de Vries",
        "tom.devries@northwind.example",
        "Rollback plan for Thursday",
        "Hi Tom,\n\nThanks for sending the checklist over so quickly. I went through it twice this \
         morning and it looks sound to me. The one thing I would change is the rollback plan: it \
         assumes the database team is online on Thursday evening, and two of them are travelling. \
         Could we ask Priya to stand by instead?\n\nLet me know what you think.\n\nBest,\nEva",
    ),
    (
        "Sofia Ruiz",
        "sofia.ruiz@northwind.example",
        "Offsite notes",
        "Hi Sofia,\n\nThank you for writing up the notes from the offsite, they are a great \
         summary of two long days. I have added the three action items that fell to me to my list \
         and will report back at the next review. Happy to help with the budget section as well \
         if you still need someone to check the figures before Friday.\n\nBest,\nEva",
    ),
    (
        "Northwind Legal",
        "legal@northwind.example",
        "Partnership agreement",
        "Hello,\n\nThank you for the final version of the agreement. I have read it carefully and \
         I am happy with every clause except the notice period in section seven, which still \
         says sixty days where we agreed on thirty. Could you correct that one line and send it \
         back? I will sign it the same day so we can keep to the schedule.\n\nKind regards,\nEva",
    ),
    (
        "Max Becker",
        "max.becker@northwind.example",
        "Next week's planning",
        "Hi Max,\n\nI am afraid I cannot make Tuesday morning after all, as the board meeting was \
         moved and now overlaps. Would Wednesday at ten work for you instead? If not, I can join \
         the first half hour on Tuesday by phone and catch up on the rest from your notes \
         afterwards. Sorry for the short notice, and thanks for being flexible.\n\nBest,\nEva",
    ),
    (
        "Lotte Bakker",
        "lotte.bakker@northwind.example",
        "Overleg volgende week",
        "Hoi Lotte,\n\nBedankt voor je uitnodiging voor het overleg van volgende week. Dinsdag \
         lukt bij mij helaas niet, want dan zit ik de hele dag bij een klant in Utrecht. \
         Woensdagmiddag of donderdagochtend kan ik wel, dus kies maar wat jou het beste uitkomt. \
         Ik neem de cijfers van het afgelopen kwartaal mee, dan kunnen we die meteen samen \
         doorlopen.\n\nGroet,\nEva",
    ),
    (
        "Daan Visser",
        "daan.visser@northwind.example",
        "Offerte nieuwe laptops",
        "Hoi Daan,\n\nDank je wel voor de offerte voor de nieuwe laptops. Ik heb hem doorgestuurd \
         naar financiën en verwacht voor het einde van de week een antwoord. Eén vraag nog: zit \
         de verlengde garantie al in de prijs, of komt die er nog bij? Als je me dat laat weten, \
         kan ik het verzoek meteen compleet maken en hoef ik ze niet twee keer te storen.\n\n\
         Groet,\nEva",
    ),
    (
        "Sanne de Jong",
        "sanne.dejong@northwind.example",
        "Presentatie vrijdag",
        "Hoi Sanne,\n\nDe presentatie voor vrijdag ziet er goed uit, mooi werk. Ik zou alleen de \
         dia over de planning naar voren halen, want daar hebben de meeste mensen vragen over. \
         Verder heb ik een paar kleine tekstuele dingen aangepast in de laatste versie. Laat maar \
         weten of je nog hulp nodig hebt met het oefenen, dan plannen we donderdag een halfuur \
         in.\n\nGroet,\nEva",
    ),
];

fn instant(at: OffsetDateTime) -> UtcDateTime {
    format!(
        "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}Z",
        at.year(),
        u8::from(at.month()),
        at.day(),
        at.hour(),
        at.minute(),
        at.second(),
    )
    .parse()
    .expect("a formatted instant parses")
}

/// `seed` with the staged sent mail in its Sent folder, one message every three days back from
/// two days before `now`, each read and with its source.
pub(crate) fn with_sent_mail(mut seed: AccountSeed, now: OffsetDateTime) -> AccountSeed {
    let sent = seed
        .mailboxes
        .iter()
        .find(|mailbox| mailbox.role == Some(MailboxRole::Sent))
        .map(|mailbox| mailbox.id.clone())
        .expect("the first showcase account has a Sent folder");
    let me = EmailAddress::named(&seed.sender_name, &seed.identity);
    for (days, (name, address, subject, text)) in (2_i64..).step_by(3).zip(SENT) {
        let key = format!("ai-sent-{days}");
        let message_id = format!("{key}@example.com");
        let mut message = Message::new(
            MessageId::try_from(key.as_str()).expect("valid message id"),
            Memberships::of_one(sent.clone()),
        );
        message.envelope.from = vec![me.clone()];
        message.envelope.to = vec![EmailAddress::named(name, address)];
        message.envelope.subject = Some(subject.to_owned());
        message.envelope.message_id =
            vec![MessageIdHeader::new(&message_id).expect("valid message-id")];
        message.preview = text.lines().nth(2).map(str::to_owned);
        message.received_at = Some(instant(now - time::Duration::days(days)));
        message.sent_at = message.received_at;
        message
            .keywords
            .insert(Keyword::system(SystemKeyword::Seen));
        let source = format!(
            "From: {} <{}>\r\nTo: {name} <{address}>\r\nSubject: {subject}\r\nMessage-ID: \
             <{message_id}>\r\nMIME-Version: 1.0\r\nContent-Type: text/plain; charset=utf-8\r\n\
             Content-Transfer-Encoding: 8bit\r\n\r\n{}\r\n",
            seed.sender_name,
            seed.identity,
            text.replace('\n', "\r\n"),
        );
        seed.bodies.insert(key, source.into_bytes());
        seed.messages.push(message);
    }
    seed
}
