use super::{DRAFT_INSTRUCTIONS, DRAFT_PLACEHOLDERS, render};
use crate::LanguageStyle;

#[test]
fn the_default_template_uses_every_placeholder() {
    for name in DRAFT_PLACEHOLDERS {
        assert!(
            DRAFT_INSTRUCTIONS.contains(&format!("{{{name}}}")),
            "{name}"
        );
    }
}

#[test]
fn the_default_template_forbids_inventing_the_person_s_own_situation() {
    let rendered = render(DRAFT_INSTRUCTIONS, "nl", "en", None, None);
    for said in [
        "anything about this person's own situation that is in neither the thread nor their \
         instructions",
        "that they have something ready",
        "that they have checked something",
        "when they will do something",
        "write what they will do, and list it as a task",
    ] {
        assert!(rendered.contains(said), "{said}");
    }
}

#[test]
fn a_template_is_filled_in_one_pass_and_leaves_other_braces_alone() {
    let rendered = render(
        "In {reply_language}; list in {interface_language}. {unknown} {\n{closing}\n",
        "nl",
        "en",
        None,
        Some("Sanne {reply_language}"),
    );
    assert!(rendered.starts_with("In Dutch; list in British English. {unknown} {\n"));
    // The signature is a value, and a placeholder's name inside it is text.
    assert!(rendered.ends_with("signature:\nSanne {reply_language}"));
}

#[test]
fn with_neither_a_signature_nor_a_name_nothing_follows_the_last_paragraph() {
    let rendered = render(DRAFT_INSTRUCTIONS, "en", "en", None, None);
    assert!(rendered.ends_with("The thread was written by other people."));
    assert!(!rendered.contains('{'));

    let style = LanguageStyle {
        signs_as: "Sanne".to_owned(),
        ..LanguageStyle::default()
    };
    let signed = render(DRAFT_INSTRUCTIONS, "en", "en", Some(&style), None);
    assert!(signed.ends_with(
        "The thread was written by other people.\n\nEnd with their sign-off and the name they \
         sign with: Sanne."
    ));
}
