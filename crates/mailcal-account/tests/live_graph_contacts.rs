//! Gated live check that a Microsoft account's contacts bind, sync and take an edit through the
//! **product** path: `connect_graph_contact_providers`, and the form builders a client's editor
//! submits through.
//!
//! The engine's own live suite proves its Graph adapter against hand-built cards. What it cannot
//! prove is that the cards *this* app builds are ones Graph accepts and hands back unchanged,
//! which is what decides whether a user's save survives a round trip. So the write here goes in
//! through `build_contact_draft` and `build_contact_patch`, and comes back out through
//! `ContactEdit::from_card`, the same way an editor is seeded.
//!
//! It **skips** unless `MS_REFRESH_TOKEN`, `MS_CLIENT_ID` and `MS_TEST_ADDRESS` are set, so the
//! offline `cargo test` stays green. It **writes**: one contact is created in the default
//! contacts folder, edited and deleted, so run it against a test account.
//! ```sh
//! T=<engine checkout>/tools/graph-oauth/.local/tokens.json   # or tokens-m365.json
//! MS_TEST_ADDRESS=<the account's own address> \
//! MS_CLIENT_ID=$(python3 -c "import json;print(json.load(open('$T'))['client_id'])") \
//! MS_REFRESH_TOKEN=$(python3 -c "import json;print(json.load(open('$T'))['refresh_token'])") \
//!   cargo test -p mailcal-account --test live_graph_contacts -- --nocapture
//! ```
//! `MS_SCOPES` overrides the scopes the refresh asks for; the default is what the tool's
//! registration grants a personal account. Add `User.ReadBasic.All` for a work account, or the
//! directory source reports itself unavailable.

use std::time::{SystemTime, UNIX_EPOCH};

use engine_core::{
    ids::AccountId,
    sync::{SyncScope, SyncUpdate},
};
use engine_provider::{ContactSourceSync, ContactsProvider};
use mailcal_account::{
    ContactEdit, CredentialOrigin, GraphTokenSource, MicrosoftConfig, Secret, build_contact_draft,
    build_contact_patch, connect_graph_contact_providers,
};

/// A Microsoft config built from the environment, or `None` to skip the gated test.
fn config() -> Option<MicrosoftConfig> {
    let var = |name| std::env::var(name).ok().filter(|value| !value.is_empty());
    let scopes = var("MS_SCOPES")
        .unwrap_or_else(|| "offline_access User.Read Contacts.ReadWrite".to_owned())
        .split_whitespace()
        .map(str::to_owned)
        .collect();
    Some(MicrosoftConfig {
        email: var("MS_TEST_ADDRESS")?,
        client_id: var("MS_CLIENT_ID")?,
        tenant: "common".to_owned(),
        redirect_uri: "http://localhost".to_owned(),
        scopes,
        refresh_token: Secret::new(var("MS_REFRESH_TOKEN")?),
        granted_scopes: None,
        shape: mailcal_account::AccountShape::default(),
    })
}

fn count<T: engine_core::sync::SyncObject>(update: &SyncUpdate<T>) -> usize {
    match update {
        SyncUpdate::Snapshot { objects, .. } => objects.len(),
        SyncUpdate::Delta { changed, .. } => changed.len(),
    }
}

fn unique() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock after the epoch")
        .as_nanos()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn live_a_microsoft_account_binds_syncs_and_round_trips_an_edit() {
    let Some(config) = config() else {
        eprintln!(
            "skipping live_a_microsoft_account_binds_syncs_and_round_trips_an_edit: \
             MS_REFRESH_TOKEN / MS_CLIENT_ID / MS_TEST_ADDRESS unset"
        );
        return;
    };
    let account: AccountId = config.account_id().expect("account id");
    let tokens = GraphTokenSource::new(&config, account.clone(), None, CredentialOrigin::Stored)
        .expect("token source");

    let providers = connect_graph_contact_providers(&account, tokens, true)
        .await
        .expect("contacts connect");
    let first = providers.first().expect("at least the default folder");
    let destination = first
        .contact_destination()
        .expect("the default folder is bound first, and accepts a create");
    assert_eq!(
        providers
            .last()
            .expect("a directory")
            .contact_scope(&account),
        SyncScope::GraphDirectoryUsers {
            account: account.clone(),
        },
    );
    eprintln!("bound {} contact source(s)", providers.len());

    let ContactSourceSync::Available { sync, .. } = first
        .sync_address_books(&account, None)
        .await
        .expect("folder listing")
    else {
        panic!("personal contact folders are never optional");
    };
    eprintln!("address books: {}", count(&sync.update));
    for (index, provider) in providers.iter().enumerate() {
        match provider.sync_contacts(&account, None).await {
            Ok(ContactSourceSync::Available { sync, .. }) => {
                eprintln!("source[{index}]: {} card(s)", count(&sync.update));
            }
            Ok(ContactSourceSync::Unavailable(unavailable)) => {
                assert!(
                    provider.contact_destination().is_none(),
                    "a personal folder reported itself unavailable: {}",
                    unavailable.reason,
                );
                eprintln!("source[{index}]: unavailable: {}", unavailable.reason);
            }
            Err(error) => panic!("source[{index}] failed: {error}"),
        }
    }

    let stamp = unique();
    let created = ContactEdit {
        given_name: "Livetest".to_owned(),
        surname: format!("Graph{stamp}"),
        organization: "Allodia Test".to_owned(),
        title: "Tester".to_owned(),
        emails: vec![format!("live-{stamp}@example.com")],
        phones: vec!["+31 20 123 4567".to_owned()],
    };
    let draft = build_contact_draft(destination.address_book, &format!("live-{stamp}"), &created)
        .expect("a valid draft");
    let receipt = first
        .create_contact(&account, &draft)
        .await
        .expect("create");

    // Everything after the create is collected rather than asserted in place, so a failed
    // assertion still reaches the delete below and leaves no card behind.
    let outcome = async {
        let stored = first.fetch_contact(&account, &receipt.contact).await?;
        let after_create = ContactEdit::from_card(&stored);
        let edited = ContactEdit {
            surname: format!("Graph{stamp}b"),
            title: "Lead tester".to_owned(),
            emails: vec![
                format!("live-{stamp}@example.com"),
                format!("live-{stamp}-2@example.com"),
            ],
            ..created.clone()
        };
        let patch = build_contact_patch(&stored, &edited).expect("a valid patch");
        first.patch_contact(&account, &stored, &patch).await?;
        let patched = first.fetch_contact(&account, &receipt.contact).await?;
        Ok::<_, engine_provider::ProviderError>((after_create, edited, patched))
    }
    .await;
    let stored = first.fetch_contact(&account, &receipt.contact).await;
    if let Ok(card) = &stored {
        first.delete_contact(&account, card).await.expect("delete");
    }

    let (after_create, edited, patched) = outcome.expect("fetch and patch");
    assert_eq!(
        after_create, created,
        "a created card reads back as it was typed"
    );
    assert_eq!(
        ContactEdit::from_card(&patched),
        edited,
        "an edited card reads back as it was saved",
    );
}
