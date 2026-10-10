use mailcal_bindings::{AccountCapability, SetupChoice};

use super::{ChosenUses, Picked, UseOffer, chosen, url_host};

const CALDAV: &str = "https://dav.example.org/caldav";
const CARDDAV: &str = "https://dav.example.org/carddav";

fn offer(caldav: &str, carddav: &str) -> UseOffer {
    let choice = |capability, found: bool| SetupChoice {
        capability,
        on: found,
        server_found: found,
    };
    UseOffer {
        choices: vec![
            choice(AccountCapability::Mail, true),
            choice(AccountCapability::Calendar, !caldav.is_empty()),
            choice(
                AccountCapability::Contacts,
                !caldav.is_empty() || !carddav.is_empty(),
            ),
        ],
        caldav_url: caldav.to_owned(),
        carddav_url: carddav.to_owned(),
    }
}

const ON: Option<Picked<'static>> = Some(Picked {
    on: true,
    typed: "",
});
const OFF: Option<Picked<'static>> = Some(Picked {
    on: false,
    typed: "",
});

#[test]
fn everything_found_and_kept_stores_both_endpoints_and_every_use() {
    assert_eq!(
        chosen(&offer(CALDAV, CARDDAV), Some(true), ON, ON),
        ChosenUses {
            uses: Some(vec![
                AccountCapability::Mail,
                AccountCapability::Calendar,
                AccountCapability::Contacts,
            ]),
            caldav_url: CALDAV.to_owned(),
            carddav_url: CARDDAV.to_owned(),
        }
    );
}

#[test]
fn contacts_found_through_the_calendar_keep_its_server_with_the_calendar_off() {
    let kept = chosen(&offer(CALDAV, ""), Some(true), OFF, ON);
    assert_eq!(
        kept.uses,
        Some(vec![AccountCapability::Mail, AccountCapability::Contacts])
    );
    assert_eq!(kept.caldav_url, CALDAV);
    assert!(kept.carddav_url.is_empty());
}

#[test]
fn a_use_switched_off_stores_no_server() {
    let mail_only = chosen(&offer(CALDAV, CARDDAV), Some(true), OFF, OFF);
    assert_eq!(mail_only.uses, Some(vec![AccountCapability::Mail]));
    assert!(mail_only.caldav_url.is_empty() && mail_only.carddav_url.is_empty());

    let without_mail = chosen(&offer(CALDAV, CARDDAV), Some(false), ON, OFF);
    assert_eq!(without_mail.uses, Some(vec![AccountCapability::Calendar]));
    assert!(without_mail.carddav_url.is_empty());
}

#[test]
fn a_server_not_found_is_the_one_typed() {
    let typed = chosen(
        &offer("", ""),
        Some(true),
        Some(Picked {
            on: true,
            typed: " cal.example.org ",
        }),
        Some(Picked {
            on: true,
            typed: "book.example.org",
        }),
    );
    assert_eq!(typed.caldav_url, "cal.example.org");
    assert_eq!(typed.carddav_url, "book.example.org");
    // A box typed into and then switched off stores nothing.
    let dropped = chosen(
        &offer("", ""),
        Some(true),
        Some(Picked {
            on: false,
            typed: "cal.example.org",
        }),
        OFF,
    );
    assert!(dropped.caldav_url.is_empty());
}

#[test]
fn a_card_that_offered_no_choice_leaves_the_uses_to_the_servers() {
    let none = UseOffer {
        caldav_url: CALDAV.to_owned(),
        ..UseOffer::default()
    };
    assert_eq!(chosen(&none, None, None, None).uses, None);
}

#[test]
fn a_discovered_endpoint_is_shown_by_host() {
    assert_eq!(
        url_host("https://caldav.example.test/dav/"),
        "caldav.example.test"
    );
    assert_eq!(url_host("not a url"), "not a url");
}

#[test]
fn contacts_are_looked_for_at_a_calendar_typed_in_place_of_one_found() {
    let typed = chosen(
        &offer("", ""),
        Some(true),
        Some(Picked {
            on: true,
            typed: "cal.example.org",
        }),
        ON,
    );
    assert_eq!(typed.caldav_url, "cal.example.org");
    assert!(
        typed.carddav_url.is_empty(),
        "contacts come from the calendar's server"
    );
    assert_eq!(
        typed.uses,
        Some(vec![
            AccountCapability::Mail,
            AccountCapability::Calendar,
            AccountCapability::Contacts
        ])
    );
}
