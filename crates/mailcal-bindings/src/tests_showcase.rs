//! The showcase (store-screenshot) dataset, across the FFI: what `MAILCAL_SHOWCASE` seeds
//! for each locale, and that the reply composer opens on a real seeded message.
//!
//! Split from `tests.rs`, which sat exactly at the 500-line limit: so any change to it,
//! anywhere, broke the file-length gate.

use std::{sync::mpsc, time::Duration};

use super::*;
use crate::tests::{ChannelObserver, NullLogger};

#[test]
fn showcase_seeds_two_accounts_with_an_attachment_and_a_remote_image() {
    // The screenshot dataset (new_showcase) is separate from the four-row demo fixture. Prove
    // it plumbs through the same FFI loop: two accounts in the switcher, a unified inbox, and,
    // the parts most likely to break: a decoded CSV attachment off a multipart source and a
    // flagged remote image off the newsletter.
    let (tx, rx) = mpsc::channel();
    let app = MailcalApp::new_showcase(
        Box::new(ChannelObserver { tx }),
        Box::new(NullLogger),
        LogLevel::Info,
        "Europe/Amsterdam".to_owned(),
        ShowcaseLocale::En,
    );
    // Drains signals until one matching `pred` arrives (a refresh emits sync-progress signals
    // before the list/reading surface). A single reused closure keeps the one `rx` borrow.
    let wait_for = |pred: fn(&Surface) -> bool| -> bool {
        while let Ok(surface) = rx.recv_timeout(Duration::from_secs(5)) {
            if pred(&surface) {
                return true;
            }
        }
        false
    };

    app.dispatch(Intent::RefreshMail);
    assert!(wait_for(|s| matches!(s, Surface::MailboxList)));
    app.dispatch(Intent::SetViewMode {
        mode: ViewMode::Flat,
    });
    assert!(wait_for(|s| matches!(s, Surface::MailboxList)));

    let snapshot = app.mailbox_list();
    // Both accounts show in the switcher, and the unified inbox merges their inbox messages.
    assert_eq!(snapshot.accounts.len(), 2);
    assert!(
        snapshot.rows.len() >= 8,
        "unified inbox merges both accounts, got {}",
        snapshot.rows.len()
    );

    let find = |needle: &str| {
        snapshot.rows.iter().find_map(|row| match row {
            SnapshotRow::Flat { row } if row.subject.contains(needle) => {
                Some((row.account.clone(), row.key.clone(), row.has_attachment))
            }
            _ => None,
        })
    };

    // The usage report advertises an attachment on its row, and opening it decodes the CSV part.
    let (report_account, report_key, report_has_attachment) =
        find("usage report").expect("the report row is present");
    assert!(report_has_attachment, "the report row shows the paperclip");
    app.dispatch(Intent::OpenMessage {
        account: report_account,
        key: report_key,
    });
    assert!(wait_for(|s| matches!(s, Surface::Reading)));
    let reading = app.reading_view();
    assert!(
        reading
            .attachments
            .iter()
            .any(|a| a.file_name == "june-report.csv"),
        "the CSV attachment is decoded from the multipart source"
    );
    // The reading header carries the full `Name <email>` sender (the list row shows just the
    // name); the report is seeded from a named address.
    assert_eq!(
        reading.from, "Example Cloud <reports@example.org>",
        "the reading view formats the sender as Name <email>"
    );

    // The newsletter's remote image is kept but flagged, so the reading view offers to load it.
    let (news_account, news_key, _) = find("European tech").expect("the newsletter row is present");
    app.dispatch(Intent::OpenMessage {
        account: news_account,
        key: news_key,
    });
    assert!(wait_for(|s| matches!(s, Surface::Reading)));
    assert!(app.reading_view().has_remote_images);
}

#[test]
fn showcase_seeds_dutch_mail_folders_and_calendar_for_the_nl_locale() {
    // The store needs a screenshot per listing language, so the showcase dataset is seeded in
    // the locale the host is about to render; Dutch mail under Dutch chrome, not half of each.
    // Both locales carry the same messages, so the attachment survives the translation too.
    let (tx, rx) = mpsc::channel();
    let app = MailcalApp::new_showcase(
        Box::new(ChannelObserver { tx }),
        Box::new(NullLogger),
        LogLevel::Info,
        "Europe/Amsterdam".to_owned(),
        ShowcaseLocale::Nl,
    );
    // Drains signals until one matching `pred` arrives (a refresh emits sync-progress signals
    // before the list/calendar surface).
    let wait_for = |pred: fn(&Surface) -> bool| -> bool {
        while let Ok(surface) = rx.recv_timeout(Duration::from_secs(5)) {
            if pred(&surface) {
                return true;
            }
        }
        false
    };

    // The first list signal can land before the mailbox sync has stored the folders, so take
    // the snapshot from the first re-render that carries them.
    app.dispatch(Intent::RefreshMail);
    let snapshot = loop {
        assert!(wait_for(|s| matches!(s, Surface::MailboxList)));
        let snapshot = app.mailbox_list();
        if !snapshot.account_folders.is_empty() {
            break snapshot;
        }
    };
    let folders: Vec<&str> = snapshot
        .account_folders
        .iter()
        .flat_map(|account| account.folders.iter().map(|f| f.name.as_str()))
        .collect();
    assert!(folders.contains(&"Postvak IN"), "folders: {folders:?}");
    assert!(folders.contains(&"Verzonden"), "folders: {folders:?}");

    let subjects: Vec<String> = snapshot
        .rows
        .iter()
        .map(|row| match row {
            SnapshotRow::Thread { row } => row.subject.clone(),
            SnapshotRow::Flat { row } => row.subject.clone(),
        })
        .collect();
    assert!(
        subjects
            .iter()
            .any(|s| s.contains("Welkom bij Allodia Mail & Calendar")),
        "the Dutch welcome mail is seeded, got {subjects:?}"
    );
    assert!(
        !subjects.iter().any(|s| s.contains("Welcome to")),
        "no English seed leaks into the Dutch showcase, got {subjects:?}"
    );

    // The calendar comes across in Dutch too: an agenda screenshot sits beside the mail one.
    app.dispatch(Intent::RefreshCalendar);
    assert!(wait_for(|s| matches!(s, Surface::Calendar)));
    assert!(
        app.calendar_list()
            .events
            .iter()
            .any(|event| event.title == "Bestuursvergadering"),
        "the Dutch calendar events are seeded"
    );
}
/// Every locale the showcase ships. Adding a `ShowcaseLocale` variant without adding it here
/// leaves the new language untested, so keep the two in step. Shared with
/// `tests_showcase_invitation`, which asks the same question of the seeded meeting.
pub(crate) const ALL_SHOWCASE_LOCALES: [ShowcaseLocale; 7] = [
    ShowcaseLocale::En,
    ShowcaseLocale::Nl,
    ShowcaseLocale::De,
    ShowcaseLocale::Fr,
    ShowcaseLocale::Es,
    ShowcaseLocale::It,
    ShowcaseLocale::Pt,
];

#[test]
fn showcase_reply_targets_a_seeded_primary_message_in_every_locale() {
    // The reply screenshot's target is a pair of constants, so a reworded seed could silently
    // orphan them and the composer would open on nothing. Pin both ends together.
    let now = time::OffsetDateTime::now_utc();
    for locale in ALL_SHOWCASE_LOCALES {
        let reply = showcase_reply(locale);
        let seed = crate::showcase_data::primary(locale, now);
        assert_eq!(
            reply.account, seed.identity,
            "{locale:?}: not the primary account"
        );
        assert!(
            seed.messages
                .iter()
                .any(|m| m.id.key().as_str() == reply.message_key),
            "{locale:?}: no seeded message keyed {}",
            reply.message_key
        );
        assert!(
            !reply.text.trim().is_empty(),
            "{locale:?}: empty reply text"
        );
    }
    // Each locale needs its own reply text, else e.g. a German screenshot shows an English reply.
    let texts: std::collections::BTreeSet<String> = ALL_SHOWCASE_LOCALES
        .iter()
        .map(|locale| showcase_reply(*locale).text)
        .collect();
    assert_eq!(
        texts.len(),
        ALL_SHOWCASE_LOCALES.len(),
        "two locales share a reply text; one was left untranslated: {texts:?}"
    );
}

#[test]
fn every_showcase_locale_seeds_the_same_shape() {
    // English is the reference seed; every other locale must carry the *same* message keys,
    // folder ids, and tailored bodies, differing only in language. Without this a new locale
    // can silently ship a missing message (a hole in the screenshot) or a missing body (the
    // attachment or the remote-image newsletter quietly falling back to a preview-only stub).
    let now = time::OffsetDateTime::now_utc();
    let shape = |locale: ShowcaseLocale| {
        let mut keys: Vec<String> = Vec::new();
        let mut folders: Vec<String> = Vec::new();
        let mut bodies: Vec<String> = Vec::new();
        for seed in [
            crate::showcase_data::primary(locale, now),
            crate::showcase_data::secondary(locale, now),
        ] {
            folders.extend(seed.mailboxes.iter().map(|m| m.id.as_str().to_owned()));
            for message in &seed.messages {
                let key = message.id.key().as_str().to_owned();
                if crate::showcase_bodies::body(locale, &key, now).is_some() {
                    bodies.push(key.clone());
                }
                keys.push(key);
            }
        }
        keys.sort();
        folders.sort();
        bodies.sort();
        (keys, folders, bodies)
    };

    let reference = shape(ShowcaseLocale::En);
    assert!(
        !reference.2.is_empty(),
        "the English seed has tailored bodies to compare against"
    );
    for locale in ALL_SHOWCASE_LOCALES {
        assert_eq!(
            shape(locale),
            reference,
            "{locale:?}: seed shape differs from English (message keys / folder ids / bodies)"
        );
    }

    // Language actually differs: the calendars' own names are translated in every locale.
    let names: std::collections::BTreeSet<String> = ALL_SHOWCASE_LOCALES
        .iter()
        .map(|locale| calendar_names(*locale, now))
        .collect();
    assert!(
        names.len() >= ALL_SHOWCASE_LOCALES.len() - 1,
        "calendar names barely differ across locales: a seed was left untranslated: {names:?}"
    );

    // …and so do the events *inside* them. The calendar names are three words; a seed can be
    // copied wholesale from English with only those three changed and still pass the check above,
    // which is precisely the hole a screenshot language ships. Compare the whole title list.
    let titles: std::collections::BTreeSet<String> = ALL_SHOWCASE_LOCALES
        .iter()
        .map(|locale| event_titles(*locale, now))
        .collect();
    assert_eq!(
        titles.len(),
        ALL_SHOWCASE_LOCALES.len(),
        "two locales seed identical calendar event titles; one was left untranslated"
    );
}

/// The primary calendar set's names in `locale`, joined: the seed's translated calendar labels.
fn calendar_names(locale: ShowcaseLocale, now: time::OffsetDateTime) -> String {
    crate::showcase_data::primary_calendar(locale, now)
        .0
        .iter()
        .map(|calendar| calendar.name.clone())
        .collect::<Vec<_>>()
        .join(" / ")
}

/// Every seeded event title in `locale`, in seed order and joined.
fn event_titles(locale: ShowcaseLocale, now: time::OffsetDateTime) -> String {
    crate::showcase_data::primary_calendar(locale, now)
        .1
        .iter()
        .map(|event| event.title.clone())
        .collect::<Vec<_>>()
        .join(" / ")
}

#[test]
fn showcase_locale_follows_the_chrome_language() {
    // The clients resolve their chrome language to a bare code and ask the core which sample
    // content to seed, so all three stay in step. An unshipped code falls back to English;
    // the same fallback the generated L10n makes, so chrome and mail can't disagree.
    assert_eq!(
        showcase_locale_for_language("de".to_owned()),
        ShowcaseLocale::De
    );
    assert_eq!(
        showcase_locale_for_language("pt".to_owned()),
        ShowcaseLocale::Pt
    );
    assert_eq!(
        showcase_locale_for_language("en".to_owned()),
        ShowcaseLocale::En
    );
    // An unshipped language and an empty code both fall back to the catalog's base locale.
    assert_eq!(
        showcase_locale_for_language("sv".to_owned()),
        ShowcaseLocale::En
    );
    assert_eq!(
        showcase_locale_for_language(String::new()),
        ShowcaseLocale::En
    );
}

#[test]
fn showcase_inbox_is_populated_at_boot_without_an_explicit_refresh() {
    // Regression: the showcase used to build an *empty* mail snapshot and depend on a single
    // post-boot RefreshMail signal landing. The calendar's own redundant refresh masked that, but
    // the mail list did not: so the Android store screenshot came up with a blank inbox.
    // build_showcase now syncs the seeded mail in before returning (mirroring the real path's
    // prime_snapshot), so the inbox is populated the instant a host connects, with no dispatch.
    let (tx, _rx) = mpsc::channel();
    let app = MailcalApp::new_showcase(
        Box::new(ChannelObserver { tx }),
        Box::new(NullLogger),
        LogLevel::Info,
        "Europe/Amsterdam".to_owned(),
        ShowcaseLocale::En,
    );
    // No wait_for / dispatch here: reading the snapshot straight after construction is the point.
    let snapshot = app.mailbox_list();
    assert_eq!(snapshot.accounts.len(), 2);
    assert!(
        snapshot.rows.len() >= 8,
        "the showcase inbox should be primed at boot, got {}",
        snapshot.rows.len()
    );
}

#[test]
fn showcase_seeds_a_signature_per_account_in_both_slots() {
    // The store screenshots turn on this: with an empty library the composer's Signature control
    // is hidden entirely and the Settings category renders its empty state, so a capture would
    // advertise neither (`docs/store-listing.md`). It is also the one part of the showcase that
    // cannot be seen in the seed data itself: the library is built by *calling* the use cases at
    // boot, so nothing but a test says whether that actually happened.
    let (tx, rx) = mpsc::channel();
    drop(rx);
    let app = MailcalApp::new_showcase(
        Box::new(ChannelObserver { tx }),
        Box::new(NullLogger),
        LogLevel::Info,
        "Europe/Amsterdam".to_owned(),
        ShowcaseLocale::En,
    );

    let snapshot = app.signatures();
    assert_eq!(
        snapshot.signatures.len(),
        2,
        "the showcase library should hold one signature per account"
    );
    assert_eq!(
        snapshot.accounts.len(),
        2,
        "both showcase accounts should appear in the signatures surface"
    );

    // Both slots, on both accounts: the reply capture composes from the primary, and a
    // new-message-only assignment would leave it with no signature while Settings said it had one.
    for account in &snapshot.accounts {
        let new_message = account
            .new_message
            .as_ref()
            .unwrap_or_else(|| panic!("{} has no new-message signature", account.email));
        let reply_forward = account
            .reply_forward
            .as_ref()
            .unwrap_or_else(|| panic!("{} has no reply/forward signature", account.email));
        assert_eq!(new_message, reply_forward, "both slots take the same one");
        // And it resolves to a body the composer can actually seed.
        let body = app
            .resolve_signature(account.account_id.clone(), SignatureSlotKind::NewMessage)
            .expect("the assigned signature resolves");
        assert!(!body.body_html.is_empty(), "the seeded body is not empty");
        assert!(
            !body.body_plain.is_empty(),
            "the plain rendering is not empty"
        );
    }
}

#[test]
fn showcase_account_detection_answers_from_the_script_not_the_network() {
    // `autodetect_tests` proves the script itself maps each domain to the right route. This
    // proves the *wiring*, that a showcase app consults it at all. Delete the `if self.showcase`
    // branch in `detect_account_settings` and those unit tests all still pass, while this one
    // fails: `northwind.example` is RFC 2606 reserved, so a real lookup can only ever come back
    // `Manual`. That is also why it is safe to run offline and in CI.
    let (tx, _rx) = mpsc::channel();
    let app = MailcalApp::new_showcase(
        Box::new(ChannelObserver { tx }),
        Box::new(NullLogger),
        LogLevel::Info,
        "Europe/Amsterdam".to_owned(),
        ShowcaseLocale::En,
    );
    // No host resolver, exactly as a screenshot run passes none.
    let SetupRecommendation::Imap { is_trusted, .. } =
        app.detect_account_settings("eva@northwind.example".to_owned(), None)
    else {
        panic!("the showcase build did not answer detection from its script");
    };
    assert!(is_trusted);
    assert!(matches!(
        app.detect_account_settings("bram@oldschool.example".to_owned(), None),
        SetupRecommendation::Imap {
            is_trusted: false,
            ..
        }
    ));
}

#[test]
fn the_showcase_clock_is_pinned_to_a_wall_time_not_the_moment_of_capture() {
    // Every screenshot is content-addressed, so a dataset dated from the real clock republishes
    // the whole set on every capture: three consecutive runs of an unchanged app produced three
    // different images, because the mailbox renders a same-day message as its *time*. Two seeds
    // taken moments apart must therefore date from the same instant, and that instant must read
    // as the pinned wall clock in the device's zone rather than as whatever time it is now.
    let zone = engine_core::time::TimeZoneId::iana("Europe/Amsterdam").expect("valid zone");
    let first = crate::showcase_data::seeded_now(&zone);
    let second = crate::showcase_data::seeded_now(&zone);
    assert_eq!(
        first, second,
        "two showcase seeds must be dated from the same instant"
    );

    let instant: engine_core::time::UtcDateTime = format!(
        "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}Z",
        first.year(),
        u8::from(first.month()),
        first.day(),
        first.hour(),
        first.minute(),
        first.second(),
    )
    .parse()
    .expect("a formatted instant parses");
    let local = engine_api::to_local(instant, &zone).expect("a supported zone localizes");
    assert_eq!(
        (local.hour(), local.minute(), local.second()),
        (9, 41, 0),
        "the seed must land on the pinned wall clock in the device zone"
    );
}
