//! The English seed of the showcase (screenshot) dataset — see [`super`] for what it is and
//! why it lives in the core. Its Dutch twin is `super::nl`; the two keep the *same* message
//! keys, folder ids, and timings, so a screenshot pair differs only in language.

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
        mailbox("inbox", "Inbox", MailboxRole::Inbox),
        mailbox("sent", "Sent", MailboxRole::Sent),
        mailbox("archive", "Archive", MailboxRole::Archive),
        mailbox("drafts", "Drafts", MailboxRole::Drafts),
    ];
    let messages: Vec<Message> = vec![
        msg("p-welcome", "inbox").from("Allodia", "hello@allodia.eu").to(me.0, me.1)
            .subject("Welcome to Allodia Mail & Calendar")
            .preview("You're all set. Here's how to make Allodia Mail & Calendar your own — connect an account, pick your sync depth, and take control.")
            .at(ago(now, 35)).id("welcome-1@allodia.eu").done(),
        // A three-message conversation: Tom's request, Eva's Sent reply, Tom's follow-up.
        msg("p-launch-1", "inbox").from("Tom de Vries", "tom.devries@northwind.example").to(me.0, me.1)
            .subject("Q3 launch — final go / no-go")
            .preview("Can you take a last look at the checklist before we lock Thursday? I'd like your sign-off on the rollback plan.")
            .at(ago(now, 150)).id("launch-1@northwind.example").done(),
        msg("p-launch-3", "inbox").from("Tom de Vries", "tom.devries@northwind.example").to(me.0, me.1)
            .subject("Re: Q3 launch — final go / no-go")
            .preview("Perfect — thanks for the quick turnaround. We're go for Thursday. I'll let the team know.")
            .at(ago(now, 95)).id("launch-3@northwind.example")
            .thread("launch-1@northwind.example", &["launch-1@northwind.example", "launch-2@example.com"]).done(),
        // The meeting invitation: an iTIP REQUEST whose calendar hold is already in the diary,
        // clashing with Thursday's board meeting (`super::invitation`).
        msg("p-invite", "inbox").from(invite_from.0, invite_from.1).to(me.0, me.1)
            .subject("Invitation: Partnership kickoff — Thursday 14:30")
            .preview("You're invited to the partnership kickoff. Please let me know whether Thursday afternoon works for you.")
            .at(ago(now, 55)).id("kickoff-1@northwind.example").done(),
        msg("p-report", "inbox").from("Example Cloud", "reports@example.org").to(me.0, me.1)
            .subject("Your June usage report")
            .preview("Thanks for using Example Cloud. Your monthly usage report is attached as a CSV.")
            .at(ago(now, 300)).seen().attachment().id("report-1@example.org").done(),
        msg("p-newsletter", "inbox").from("European Digital Weekly", "weekly@europeandigital.example").to(me.0, me.1)
            .subject("This week in European tech")
            .preview("Sovereignty rules move forward, plus three tools we're watching this month.")
            .at(ago(now, 1445)).seen().id("news-1@europeandigital.example").done(),
        msg("p-contract", "inbox").from("Northwind Legal", "legal@northwind.example").to(me.0, me.1)
            .subject("Please sign: partnership agreement")
            .preview("The final version is ready for your signature. Let me know if anything needs changing.")
            .at(ago(now, 1600)).seen().flagged().id("contract-1@northwind.example").done(),
        msg("p-shipping", "inbox").from("De Fietswinkel", "orders@fietswinkel.example").to(me.0, me.1)
            .subject("Your order has shipped")
            .preview("Good news — your order is on its way. Expected delivery: tomorrow between 9:00 and 17:00.")
            .at(ago(now, 2880)).seen().id("ship-1@fietswinkel.example").done(),
        // Eva's own Sent reply on the thread (rides in the conversation with a "Sent" badge).
        msg("p-launch-2", "sent").from(me.0, me.1).to("Tom de Vries", "tom.devries@northwind.example")
            .subject("Re: Q3 launch — final go / no-go")
            .preview("Checklist looks solid. One tweak to the rollback plan and then it's a go from me.")
            .at(ago(now, 120)).seen().id("launch-2@example.com")
            .thread("launch-1@northwind.example", &["launch-1@northwind.example"]).done(),
        msg("p-sent-lunch", "sent").from(me.0, me.1).to("Sofia Ruiz", "sofia.ruiz@northwind.example")
            .subject("Lunch Thursday?")
            .preview("Fancy lunch on Thursday after the review? There's a new place near the office.")
            .at(ago(now, 180)).seen().id("lunch-1@example.com").done(),
        msg("p-archive-notes", "archive").from("Sofia Ruiz", "sofia.ruiz@northwind.example").to(me.0, me.1)
            .subject("Notes from the offsite")
            .preview("Great couple of days — here's the summary and the action items we agreed on.")
            .at(ago(now, 8640)).seen().id("offsite-1@northwind.example").done(),
        msg("p-draft-board", "drafts").from(me.0, me.1).to("Board", "board@northwind.example")
            .subject("Q3 board update")
            .preview("Draft — headline numbers, launch status, and the two decisions I need from the board.")
            .at(ago(now, 240)).draft().id("boarddraft-1@example.com").done(),
    ];
    seed(ShowcaseLocale::En, me.1, mailboxes, messages, now)
}

pub(super) fn secondary(now: OffsetDateTime) -> AccountSeed {
    let me = ("Eva Jansen", "eva@northwind.example");
    let mailboxes: Vec<Mailbox> = vec![
        mailbox("inbox", "Inbox", MailboxRole::Inbox),
        mailbox("sent", "Sent", MailboxRole::Sent),
    ];
    let messages: Vec<Message> = vec![
        msg("w-welcome", "inbox").from("Northwind People", "hr@northwind.example").to(me.0, me.1)
            .subject("Welcome to your first week")
            .preview("We're glad you're here. Everything you need for a smooth start is in the onboarding space.")
            .at(ago(now, 240)).seen().id("hr-1@northwind.example").done(),
        msg("w-2fa", "inbox").from("Northwind IT", "it@northwind.example").to(me.0, me.1)
            .subject("Action needed: set up 2-step sign-in")
            .preview("Please enable two-step verification on your account before Friday. It takes about two minutes.")
            .at(ago(now, 1400)).flagged().id("it-1@northwind.example").done(),
        msg("w-review", "inbox").from("Sofia Ruiz", "sofia.ruiz@northwind.example").to(me.0, me.1)
            .subject("Design review moved to Friday")
            .preview("Heads up — we pushed the design review to Friday 11:00 so everyone can join.")
            .at(ago(now, 2880)).seen().id("review-1@northwind.example").done(),
        msg("w-allhands", "inbox").from("Northwind", "allhands@northwind.example").to(me.0, me.1)
            .subject("All-hands recap & slides")
            .preview("Missed the all-hands? Here's the recording and the deck.")
            .at(ago(now, 4320)).seen().id("ah-1@northwind.example").done(),
    ];
    seed(ShowcaseLocale::En, me.1, mailboxes, messages, now)
}

/// The three showcase calendars' names and a full week of events spread across them — a real
/// work-and-private-life week (Europe/Amsterdam). Every event is anchored to a **weekday of the
/// current week** and to a time of day, so the visible week fills whatever day the screenshot is
/// taken and whatever hour the grid scrolls to.
pub(super) fn calendar(now: OffsetDateTime) -> (CalendarNames, Vec<Event>) {
    let events = vec![
        // ---- Monday --------------------------------------------------------------------------
        event(
            "ev-mon-standup",
            "Team standup",
            Cal::Work,
            zoned_wd(now, MON, 9, 30),
            15,
        ),
        event(
            "ev-mon-sprint",
            "Sprint planning",
            Cal::Work,
            zoned_wd(now, MON, 11, 0),
            60,
        ),
        event(
            "ev-mon-pickup",
            "School pickup",
            Cal::Family,
            zoned_wd(now, MON, 15, 30),
            30,
        ),
        event(
            "ev-mon-designsync",
            "Design sync",
            Cal::Work,
            zoned_wd(now, MON, 16, 0),
            45,
        ),
        event(
            "ev-mon-gym",
            "Gym",
            Cal::Personal,
            zoned_wd(now, MON, 18, 30),
            60,
        ),
        // ---- Tuesday: THREE of these overlap, and that is the point — the core packs a clash
        // into columns (`calendar/packing.rs`); a client that ignored `column`/`columns` would
        // draw them full width, one on top of another, and an event hidden behind another is an
        // event you miss. A showcase where nothing ever clashes never shows the bug.
        event(
            "ev-tue-standup",
            "Team standup",
            Cal::Work,
            zoned_wd(now, TUE, 9, 30),
            15,
        ),
        event(
            "ev-tue-triage",
            "Bug triage",
            Cal::Work,
            zoned_wd(now, TUE, 10, 0),
            60,
        ),
        event(
            "ev-tue-design",
            "Design review",
            Cal::Work,
            zoned_wd(now, TUE, 10, 15),
            45,
        ),
        event(
            "ev-tue-interview",
            "Interview: backend",
            Cal::Work,
            zoned_wd(now, TUE, 10, 30),
            60,
        ),
        event(
            "ev-tue-lunch",
            "Lunch with Sofia",
            Cal::Personal,
            zoned_wd(now, TUE, 12, 30),
            60,
        ),
        event(
            "ev-tue-1on1",
            "1:1 with Priya",
            Cal::Work,
            zoned_wd(now, TUE, 15, 0),
            30,
        ),
        event(
            "ev-tue-bookclub",
            "Book club",
            Cal::Personal,
            zoned_wd(now, TUE, 19, 30),
            90,
        ),
        // ---- Wednesday -----------------------------------------------------------------------
        event(
            "ev-wed-dentist",
            "Dentist",
            Cal::Personal,
            zoned_wd(now, WED, 8, 0),
            45,
        ),
        event(
            "ev-wed-standup",
            "Team standup",
            Cal::Work,
            zoned_wd(now, WED, 9, 30),
            15,
        ),
        event(
            "ev-wed-review",
            "Q3 launch review",
            Cal::Work,
            zoned_wd(now, WED, 13, 0),
            90,
        ),
        event(
            "ev-wed-football",
            "Football practice",
            Cal::Family,
            zoned_wd(now, WED, 16, 30),
            60,
        ),
        event(
            "ev-wed-dinner",
            "Dinner with friends",
            Cal::Personal,
            zoned_wd(now, WED, 19, 0),
            120,
        ),
        // ---- Thursday ------------------------------------------------------------------------
        event(
            "ev-thu-standup",
            "Team standup",
            Cal::Work,
            zoned_wd(now, THU, 9, 30),
            15,
        ),
        event(
            "ev-thu-roadmap",
            "Roadmap workshop",
            Cal::Work,
            zoned_wd(now, THU, 11, 0),
            90,
        ),
        event(
            "ev-thu-board",
            "Board meeting",
            Cal::Work,
            zoned_wd(now, THU, 14, 0),
            120,
        ),
        event(
            "ev-thu-piano",
            "Piano lesson",
            Cal::Family,
            zoned_wd(now, THU, 17, 30),
            45,
        ),
        event(
            "ev-thu-parents",
            "Call with parents",
            Cal::Family,
            zoned_wd(now, THU, 20, 0),
            45,
        ),
        // ---- Friday --------------------------------------------------------------------------
        event(
            "ev-fri-standup",
            "Team standup",
            Cal::Work,
            zoned_wd(now, FRI, 9, 30),
            15,
        ),
        event(
            "ev-fri-lunchlearn",
            "Lunch & learn",
            Cal::Work,
            zoned_wd(now, FRI, 12, 30),
            60,
        ),
        event(
            "ev-fri-demo",
            "Sprint demo",
            Cal::Work,
            zoned_wd(now, FRI, 15, 0),
            45,
        ),
        event(
            "ev-fri-haircut",
            "Haircut",
            Cal::Personal,
            zoned_wd(now, FRI, 17, 0),
            30,
        ),
        event(
            "ev-fri-datenight",
            "Date night",
            Cal::Personal,
            zoned_wd(now, FRI, 20, 0),
            120,
        ),
        // ---- Saturday ------------------------------------------------------------------------
        event(
            "ev-sat-market",
            "Farmers market",
            Cal::Personal,
            zoned_wd(now, SAT, 10, 0),
            90,
        ),
        event(
            "ev-sat-birthday",
            "Grandma's birthday",
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
        // ---- Sunday --------------------------------------------------------------------------
        event(
            "ev-sun-brunch",
            "Brunch with Sofia",
            Cal::Personal,
            zoned_wd(now, SUN, 11, 0),
            90,
        ),
        event(
            "ev-sun-1on1",
            "1:1 with Tom",
            Cal::Work,
            zoned_wd(now, SUN, 15, 0),
            30,
        ),
        event(
            "ev-sun-walk",
            "Family walk",
            Cal::Family,
            zoned_wd(now, SUN, 16, 0),
            90,
        ),
        event(
            "ev-sun-mealprep",
            "Meal prep",
            Cal::Personal,
            zoned_wd(now, SUN, 18, 0),
            45,
        ),
        // ---- All-day and multi-day, in the banner above the grid. FOUR overlap on Sunday —
        // one lane more than the collapsed banner shows — so the "+N more" overflow is exercised
        // rather than merely implemented; the weekday bars (offsite, leave) span multiple days.
        all_day_event("ev-ad-offsite", "Team offsite", Cal::Work, now, WED, 2),
        all_day_event("ev-ad-leave", "Lisa on leave", Cal::Work, now, MON, 3),
        all_day_event("ev-ad-release", "Release day", Cal::Work, now, FRI, 1),
        all_day_event(
            "ev-ad-visiting",
            "Mum & Dad visiting",
            Cal::Family,
            now,
            SAT,
            2,
        ),
        all_day_event("ev-ad-birthday", "Tom's birthday", Cal::Family, now, SUN, 1),
        all_day_event(
            "ev-ad-holiday",
            "Public holiday",
            Cal::Personal,
            now,
            SUN,
            1,
        ),
        all_day_event(
            "ev-ad-marathon",
            "Half marathon",
            Cal::Personal,
            now,
            SUN,
            1,
        ),
    ];
    (
        CalendarNames {
            work: "Work",
            personal: "Personal",
            family: "Family",
        },
        events,
    )
}

/// The meeting invitation's title, room and notes. See [`super::invite_text`].
pub(super) fn invite_text() -> InviteText {
    InviteText {
        summary: "Partnership kickoff",
        location: "Meeting room Amstel · Amsterdam",
        description: "An hour to agree the scope, the timeline and who owns what. \
                      Bring the draft plan — we'll go through it together.",
    }
}

/// The sample text the showcase reply composer opens pre-filled with — Eva's answer to Northwind
/// Legal's signature request (`p-contract`). See [`super::showcase_reply`].
pub(super) fn reply_text() -> &'static str {
    "Thanks — I've read it through and it all looks good to me. \
     I'll sign it this afternoon and send the countersigned copy back today."
}

/// The two signatures the showcase library holds. See [`super::signatures`].
pub(super) fn signatures() -> super::ShowcaseSignatures {
    super::ShowcaseSignatures {
        primary: super::SignatureSeed {
            name: "Work",
            body_html: "<div><b>Eva Jansen</b></div><div>Product lead · Northwind</div>\
                        <div>eva.jansen@example.com</div>",
            body_plain: "Eva Jansen\nProduct lead · Northwind\neva.jansen@example.com",
        },
        secondary: super::SignatureSeed {
            name: "Short",
            body_html: "<div>Eva Jansen</div><div>eva@northwind.example</div>",
            body_plain: "Eva Jansen\neva@northwind.example",
        },
    }
}
