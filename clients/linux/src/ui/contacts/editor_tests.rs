//! What the editor turns a form into, and what it refuses.

use super::*;

fn books() -> Vec<BookChoice> {
    vec![
        BookChoice {
            account: "personal".into(),
            book: "personal-book".into(),
            label: "me@example.test".into(),
            is_default: false,
        },
        BookChoice {
            account: "work".into(),
            book: "work-book".into(),
            label: "me@work.test".into(),
            is_default: true,
        },
    ]
}

fn form() -> ContactForm {
    ContactForm {
        given_name: "Ada".into(),
        surname: "Lovelace".into(),
        emails: vec!["ada@example.test".into()],
        ..ContactForm::default()
    }
}

/// The picker opens on the account's own default book, not on whichever came first: a user
/// with a personal book and a shared one saves to the personal one unless they say otherwise.
#[test]
fn a_create_opens_on_the_default_book_and_files_the_chosen_one() {
    let editor = ContactEditor::create(books());
    assert_eq!(editor.selected, 1);
    let Ok(Intent::CreateContact {
        account,
        address_book,
        edit,
    }) = editor.intent(&ContactForm {
        book_index: 0,
        ..form()
    })
    else {
        panic!("a valid form did not produce a create");
    };
    assert_eq!(account.as_deref(), Some("personal"));
    assert_eq!(address_book.as_deref(), Some("personal-book"));
    assert_eq!(edit.given_name, "Ada");
    assert_eq!(edit.emails, vec!["ada@example.test".to_owned()]);
}

/// An edit names the card it was opened on, never the person: a person is several accounts'
/// cards, and saving without naming one files the work details in the personal book.
#[test]
fn an_edit_carries_the_card_it_was_opened_on() {
    let editor = ContactEditor::edit(
        EditTarget {
            person: "7".into(),
            account: "work".into(),
            card: "c-work".into(),
        },
        ContactEdit {
            given_name: "Ada".into(),
            surname: "Lovelace".into(),
            organization: String::new(),
            title: String::new(),
            emails: vec!["ada@example.test".into()],
            phones: Vec::new(),
        },
    );
    let Ok(Intent::UpdateContact {
        person,
        account,
        card,
        ..
    }) = editor.intent(&form())
    else {
        panic!("a valid form did not produce an update");
    };
    assert_eq!(person, "7");
    assert_eq!(account, "work");
    assert_eq!(card, "c-work");
}

/// A company contact has no person's name, and a card with none of the three is a blank row
/// nobody can find again.
#[test]
fn an_organization_alone_is_enough_and_nothing_at_all_is_not() {
    let editor = ContactEditor::create(books());
    assert!(
        editor
            .intent(&ContactForm {
                organization: "Analytical Engines".into(),
                ..ContactForm::default()
            })
            .is_ok()
    );
    assert_eq!(
        editor.intent(&ContactForm::default()).err(),
        Some(FormError::Empty)
    );
}

/// The two refusals are different sentences on screen, so they are different values here.
#[test]
fn a_malformed_address_is_its_own_refusal() {
    let editor = ContactEditor::create(books());
    for malformed in [
        "ada",
        "@example.test",
        "ada@",
        "ada@@example.test",
        "ada@.test",
    ] {
        assert_eq!(
            editor
                .intent(&ContactForm {
                    emails: vec![malformed.into()],
                    ..form()
                })
                .err(),
            Some(FormError::Email),
            "{malformed} was accepted"
        );
    }
}

/// A row the user emptied is a row they removed, so it must not fail validation as a blank
/// address, and must not reach the core as one either.
#[test]
fn blank_rows_are_dropped_rather_than_refused() {
    let editor = ContactEditor::create(books());
    let Ok(Intent::CreateContact { edit, .. }) = editor.intent(&ContactForm {
        emails: vec!["  ".into(), " ada@example.test ".into()],
        phones: vec![String::new()],
        ..form()
    }) else {
        panic!("a form with an emptied row was refused");
    };
    assert_eq!(edit.emails, vec!["ada@example.test".to_owned()]);
    assert!(edit.phones.is_empty());
}
