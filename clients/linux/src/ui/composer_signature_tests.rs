//! The composer's signature rules: the slot a kind seeds from, the resolution precedence, the
//! seed payload and the picker's menu.

use gtk::{gio, glib, prelude::*};
use mailcal_bindings::{SignatureBody, SignatureSlotKind};

use super::{
    ComposeKind, SignatureChoice, resolve, signature_menu, signature_seed, signature_slot,
};

fn body(id: &str) -> SignatureBody {
    SignatureBody {
        id: id.to_owned(),
        body_html: format!("<p>{id}</p>"),
        body_plain: id.to_owned(),
    }
}

#[test]
fn a_reply_a_reply_all_and_a_forward_share_one_slot() {
    assert!(matches!(
        signature_slot(ComposeKind::New),
        SignatureSlotKind::NewMessage
    ));
    for kind in [
        ComposeKind::Reply,
        ComposeKind::ReplyAll,
        ComposeKind::Forward,
    ] {
        assert!(
            matches!(signature_slot(kind), SignatureSlotKind::ReplyForward),
            "{kind:?} continues an existing message, so it seeds from the reply slot"
        );
    }
}

#[test]
fn no_choice_follows_the_account_and_its_slot() {
    let resolved = resolve(
        None,
        Some("work"),
        ComposeKind::Reply,
        |account, slot| {
            assert_eq!(account, "work");
            assert!(matches!(slot, SignatureSlotKind::ReplyForward));
            Some(body("reply-signature"))
        },
        |_| panic!("no explicit choice means no lookup by id"),
    );

    assert_eq!(
        resolved.expect("the account assigns one").id,
        "reply-signature"
    );
}

#[test]
fn an_explicit_choice_wins_over_the_account_in_both_directions() {
    let account = |_: &str, _: SignatureSlotKind| Some(body("account-signature"));

    // "None" is a choice, not the absence of one: it must not fall back to the account.
    assert!(
        resolve(
            Some(&SignatureChoice::NoSignature),
            Some("work"),
            ComposeKind::New,
            account,
            |_| panic!("None resolves to nothing without a lookup"),
        )
        .is_none()
    );
    let named = resolve(
        Some(&SignatureChoice::Named("picked".to_owned())),
        Some("work"),
        ComposeKind::New,
        account,
        |id| Some(body(id)),
    );
    assert_eq!(named.expect("the picked signature").id, "picked");
}

#[test]
fn a_composer_with_no_sender_yet_resolves_to_nothing() {
    assert!(
        resolve(
            None,
            None,
            ComposeKind::New,
            |_, _| panic!("there is no account to ask about"),
            |_| panic!("and no id to look up"),
        )
        .is_none()
    );
}

#[test]
fn the_seed_carries_the_two_keys_the_editor_and_the_core_share() {
    let seed = signature_seed(Some(&body("work")));
    let parsed: serde_json::Value = serde_json::from_str(&seed).expect("valid JSON");

    // The Rust field names verbatim: the editor reads `body_html`/`body_plain` and emits the
    // same two back inside the Signature block. A renamed key is read as "no signature".
    assert_eq!(parsed["body_html"], "<p>work</p>");
    assert_eq!(parsed["body_plain"], "work");
    // `null`, not `{}`: the seam reads it as "remove the region".
    assert_eq!(signature_seed(None), "null");
}

#[test]
fn every_menu_entry_is_one_choice_from_a_set_none_included() {
    let library = [
        ("id-work".to_owned(), "Work".to_owned()),
        ("id-home".to_owned(), "Home & away".to_owned()),
    ];

    let menu = signature_menu(&library);

    assert_eq!(menu.n_items(), 3, "the library, plus None");
    let entries = (0..menu.n_items())
        .map(|index| {
            let label = menu
                .item_attribute_value(index, gio::MENU_ATTRIBUTE_LABEL, None)
                .and_then(|value| value.get::<String>())
                .expect("every entry is labelled");
            let action = menu
                .item_attribute_value(index, gio::MENU_ATTRIBUTE_ACTION, None)
                .and_then(|value| value.get::<String>())
                .expect("every entry targets the radio action");
            let target = menu
                .item_attribute_value(index, gio::MENU_ATTRIBUTE_TARGET, None)
                .and_then(|value| value.get::<String>())
                .expect("every entry carries the id it selects");
            (label, action, target)
        })
        .collect::<Vec<_>>();

    // One action for all three, so GTK draws exactly one mark and moves it: and so "None" is
    // a choice the user can make rather than the absence of one.
    assert!(
        entries
            .iter()
            .all(|(_, action, _)| action == "signature.choose")
    );
    assert_eq!(entries[0].0, "None");
    assert_eq!(entries[0].2, "", "None selects no signature");
    assert_eq!(
        entries[1],
        (
            "Work".to_owned(),
            "signature.choose".to_owned(),
            "id-work".to_owned()
        )
    );
    // A menu label is plain text, so a name with an ampersand reaches the menu intact.
    assert_eq!(entries[2].0, "Home & away");
    assert_eq!(entries[2].2, "id-home");
}

#[test]
fn the_action_carries_the_id_the_handler_reads_back() {
    // The target is a value rather than part of a detailed action name: a signature id is
    // opaque CSPRNG output, and `"signature.choose::<id>"` would have to survive that parser.
    let item = super::menu_item("Work", "id-work");
    let target = item
        .attribute_value(gio::MENU_ATTRIBUTE_TARGET, Some(glib::VariantTy::STRING))
        .expect("a string target");

    assert_eq!(target.str(), Some("id-work"));
}
