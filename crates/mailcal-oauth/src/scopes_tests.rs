use std::collections::BTreeSet;

use super::{GOOGLE, MICROSOFT, ProviderScopes};
use crate::{GOOGLE_SCOPES, MICROSOFT_GRAPH_SCOPES};

fn every_group(provider: &ProviderScopes) -> BTreeSet<&'static str> {
    let groups = [
        provider.mail,
        provider.calendar,
        provider.contacts,
        provider.colleagues,
    ];
    for group in groups {
        assert!(
            group.requested.contains(&group.needed),
            "{} is needed but not requested",
            group.needed
        );
    }
    provider
        .always
        .iter()
        .copied()
        .chain(
            groups
                .iter()
                .flat_map(|group| group.requested.iter().copied()),
        )
        .collect()
}

#[test]
fn the_groups_together_ask_for_exactly_what_an_account_used_for_everything_asks_for() {
    assert_eq!(
        every_group(&MICROSOFT),
        MICROSOFT_GRAPH_SCOPES.iter().copied().collect()
    );
    assert_eq!(
        every_group(&GOOGLE),
        GOOGLE_SCOPES.iter().copied().collect()
    );
}

#[test]
fn a_graph_scope_is_held_however_microsoft_spells_it_back() {
    let granted = vec![
        "mail.readwrite".to_owned(),
        "https://graph.microsoft.com/Calendars.ReadWrite".to_owned(),
        "User.Read".to_owned(),
    ];
    assert!(MICROSOFT.holds(&granted, MICROSOFT.mail.needed));
    assert!(MICROSOFT.holds(&granted, MICROSOFT.calendar.needed));
    assert!(MICROSOFT.holds(&granted, "https://graph.microsoft.com/User.Read"));
    assert!(!MICROSOFT.holds(&granted, MICROSOFT.contacts.needed));
}

#[test]
fn a_google_scope_is_held_only_under_its_own_name() {
    let granted = vec!["https://www.googleapis.com/auth/calendar".to_owned()];
    assert!(GOOGLE.holds(&granted, GOOGLE.calendar.needed));
    assert!(!GOOGLE.holds(&granted, GOOGLE.mail.needed));
    // A scope that merely shares a prefix is another scope.
    let readonly = vec!["https://www.googleapis.com/auth/calendar.readonly".to_owned()];
    assert!(!GOOGLE.holds(&readonly, GOOGLE.calendar.needed));
}
