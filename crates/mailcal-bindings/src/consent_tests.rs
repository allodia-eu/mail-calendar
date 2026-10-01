use engine_api::AccountId;
use mailcal_account::{AccountLinks, AccountShape, Capabilities, Capability};

use super::{keep_stored_shape, opened, refuse_an_empty_grant};

fn set(capabilities: &[Capability]) -> Capabilities {
    capabilities.iter().copied().collect()
}

#[test]
fn a_use_the_grant_withholds_is_not_opened_mail_included() {
    // A person who kept their contacts and unticked mail and the calendar.
    let chosen = set(&[Capability::Mail, Capability::Calendar, Capability::Contacts]);
    let withheld = set(&[Capability::Mail, Capability::Calendar]);
    assert_eq!(opened(&chosen, &withheld), set(&[Capability::Contacts]));
}

#[test]
fn a_grant_allowing_nothing_chosen_is_refused() {
    let chosen = set(&[Capability::Calendar, Capability::Colleagues]);
    assert!(refuse_an_empty_grant(&chosen, &set(&[Capability::Calendar])).is_err());
    assert!(refuse_an_empty_grant(&chosen, &Capabilities::default()).is_ok());
    let mail = set(&[Capability::Mail]);
    assert!(refuse_an_empty_grant(&mail, &mail).is_err());
}

#[test]
fn signing_in_again_keeps_the_pinned_id_the_links_and_an_unchanged_choice() {
    let calendar = AccountId::try_from("alice@dav:cloud.example").unwrap();
    let stored = AccountShape {
        id: Some(AccountId::try_from("alice@example.org@graph.microsoft.com").unwrap()),
        capabilities: Some(set(&[Capability::Mail])),
        links: AccountLinks {
            calendar: Some(calendar),
            ..AccountLinks::default()
        },
    };
    let mut signed_in = AccountShape::default();
    keep_stored_shape(&mut signed_in, Some(stored.clone()));
    assert_eq!(signed_in, stored);

    let mut chose_again = AccountShape {
        capabilities: Some(set(&[Capability::Mail, Capability::Calendar])),
        ..AccountShape::default()
    };
    keep_stored_shape(&mut chose_again, Some(stored.clone()));
    assert_eq!(
        chose_again.capabilities,
        Some(set(&[Capability::Mail, Capability::Calendar]))
    );
    assert_eq!(chose_again.links, stored.links);
}
