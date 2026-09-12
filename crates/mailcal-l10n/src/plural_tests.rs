//! Plural-form codegen: a `<key>_one` beside `<key>` becomes one accessor that chooses by
//! count, so no client repeats the choice and none of them can disagree about it.
//!
//! The cases below are mostly about what is *not* captured. This catalog already held pairs
//! written the older way (`invitation_attendees_one`, `"1 attendee"`, chosen between at the call
//! site), and folding those into a chooser would delete accessors four clients call.

use std::collections::BTreeMap;

use super::{
    brand::Brand,
    emit_kotlin, emit_rust, emit_swift, emit_winui,
    model::{Catalog, Raw},
    validate,
};

/// A catalog fixture: the required picker labels, plus whatever `extra` the case is about.
fn catalog_with(extra: &[(&str, &str)]) -> Catalog {
    Catalog::from_raw(&raw_with(extra))
}

fn raw_with(extra: &[(&str, &str)]) -> Raw {
    let mut pairs: Vec<(&str, &str)> = vec![
        ("settings_language_en", "English"),
        ("settings_language_nl", "Nederlands"),
    ];
    pairs.extend_from_slice(extra);
    let map: BTreeMap<String, String> = pairs
        .iter()
        .map(|(k, v)| ((*k).to_string(), (*v).to_string()))
        .collect();
    let mut maps = BTreeMap::new();
    maps.insert("en".to_string(), map.clone());
    maps.insert("nl".to_string(), map);
    Raw {
        base: "en".to_string(),
        locales: vec!["en".to_string(), "nl".to_string()],
        maps,
    }
}

fn brand() -> Brand {
    Brand {
        name: "Fixture Mail".to_string(),
        id: "org.fixture.client".to_string(),
    }
}

/// The pair this whole mechanism exists for.
const PLURAL_PAIR: [(&str, &str); 2] = [("rows", "{count} rows"), ("rows_one", "{count} row")];

/// The older shape beside it: the numeral is spelled into the sentence, so the two are separate
/// messages rather than two forms of one.
const NUMERAL_PAIR: [(&str, &str); 2] = [("guests", "{count} guests"), ("guests_one", "1 guest")];

#[test]
fn a_singular_partner_is_reached_through_its_plurals_accessor_and_has_none_of_its_own() {
    let catalog = catalog_with(&PLURAL_PAIR);

    assert!(
        catalog.has_singular("rows"),
        "`rows` has a singular partner"
    );
    assert!(
        catalog.is_singular("rows_one"),
        "`rows_one` is that partner"
    );
    let names: Vec<&str> = catalog.accessors().map(|m| m.key.as_str()).collect();
    assert!(names.contains(&"rows"), "the plural keeps its accessor");
    assert!(
        !names.contains(&"rows_one"),
        "the singular gets none: a caller passes a count, never a grammatical form",
    );
}

/// **The regression this rule is shaped around.** `invitation_attendees_one` and its four
/// siblings predate plural forms: they spell the numeral in, and four clients call them
/// directly. Capturing them would delete those accessors and fail every client's build.
#[test]
fn a_pair_that_spells_the_numeral_in_is_two_messages_and_keeps_both_accessors() {
    let catalog = catalog_with(&NUMERAL_PAIR);

    assert!(
        !catalog.has_singular("guests"),
        "a partner without {{count}} has nothing to choose on, so `guests` is not a chooser",
    );
    let names: Vec<&str> = catalog.accessors().map(|m| m.key.as_str()).collect();
    assert!(names.contains(&"guests"), "both keep their own accessor");
    assert!(names.contains(&"guests_one"), "including the `_one` one");
}

#[test]
fn a_key_ending_in_one_with_no_plural_beside_it_is_an_ordinary_message() {
    // `setup_allodia_have_one` ("Already have one? Sign in") is a sentence, not a count.
    let catalog = catalog_with(&[("have_one", "Already have one?")]);

    assert!(!catalog.is_singular("have_one"));
    let names: Vec<&str> = catalog.accessors().map(|m| m.key.as_str()).collect();
    assert!(names.contains(&"have_one"));
}

#[test]
fn swift_picks_the_key_by_count_and_takes_the_singular_at_zero_in_french_only() {
    let files = emit_swift::render(&catalog_with(&PLURAL_PAIR), &brand());
    let source = &files[0].1;

    assert!(
        source.contains(r#"resolve(plural("rows", count)"#),
        "the accessor resolves whichever key the count calls for",
    );
    assert!(
        source.contains(r#"current() == "fr" ? count <= 1 : count == 1"#),
        "French counts zero as singular; every other shipped locale takes it at one alone",
    );
}

#[test]
fn rust_branches_between_the_two_templates_inside_one_accessor() {
    let files = emit_rust::render(&catalog_with(&PLURAL_PAIR), &brand());
    let source = &files[0].1;

    assert!(source.contains("pub(crate) fn rows(count: i64) -> String {"));
    assert!(
        source.contains("let mut message = if is_one(count) {"),
        "the Linux target inlines templates, so the choice is a branch, not a key lookup",
    );
    assert!(
        !source.contains("fn rows_one("),
        "the singular gets no accessor"
    );
}

/// The emitted `is_one` is compiled by the Linux crate, which denies warnings: a catalog with
/// no plural pair must not emit a helper nothing calls.
#[test]
fn rust_emits_no_plural_helper_for_a_catalog_that_has_no_pair() {
    let files = emit_rust::render(&catalog_with(&NUMERAL_PAIR), &brand());

    assert!(!files[0].1.contains("fn is_one("));
}

#[test]
fn kotlin_picks_between_two_resource_ids_because_android_resolves_by_id() {
    let files = emit_kotlin::render(&catalog_with(&PLURAL_PAIR), "org.fixture");
    let source = &files
        .iter()
        .find(|(path, _)| path.ends_with("L10n.kt"))
        .expect("the Kotlin accessor")
        .1;

    assert!(source.contains("if (isOne(ctx, count)) R.string.rows_one else R.string.rows"));
    assert!(
        source.contains(r#"ctx.resources.configuration.locales[0].language == "fr""#),
        "Android carries the active language on the context, not on a resolved locale",
    );
}

#[test]
fn winui_picks_between_two_resource_names() {
    let files = emit_winui::render(&catalog_with(&PLURAL_PAIR), "Fixture", &brand());
    let source = &files
        .iter()
        .find(|(path, _)| path.ends_with("L10n.cs"))
        .expect("the C# accessor")
        .1;

    assert!(source.contains(r#"Get(IsOne(count) ? "rows_one" : "rows")"#));
}

#[test]
fn a_singular_whose_placeholders_differ_from_its_plural_fails_the_build() {
    // Both branches format with the arguments the one accessor declares, so a mismatch prints a
    // `{name}` literally in exactly one locale at exactly one count: the shape of bug that
    // reaches a user rather than a test.
    let raw = raw_with(&[
        ("rows", "{count} rows in {folder}"),
        ("rows_one", "{count} row"),
    ]);

    let error = validate::check(&raw).expect_err("a mismatched pair must not generate");
    assert!(
        error.contains("rows_one") && error.contains("differ from its plural"),
        "the error names the pair: {error}",
    );
}

#[test]
fn a_well_formed_pair_validates() {
    assert!(validate::check(&raw_with(&PLURAL_PAIR)).is_ok());
    assert!(validate::check(&raw_with(&NUMERAL_PAIR)).is_ok());
}

/// The shipped catalog, as the generators read it.
fn shipped() -> Catalog {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(std::path::Path::parent)
        .expect("the crate sits two levels under the repository root")
        .to_path_buf();
    let raw = super::parse::load(&root).expect("the shipped catalog loads");
    Catalog::from_raw(&raw)
}

/// The template one locale carries for one key.
fn template<'a>(catalog: &'a Catalog, key: &str, locale: &str) -> &'a str {
    catalog
        .message(key)
        .unwrap_or_else(|| panic!("the catalog carries \"{key}\""))
        .values
        .get(locale)
        .unwrap_or_else(|| panic!("\"{key}\" is translated into {locale}"))
        .as_str()
}

/// The folder pane's unread badge states a number of messages, so it is a count like any other
/// (`docs/folder-pane.md`). The cases above prove the mechanism; this one proves this key is
/// wired into it, which is the half a mechanism test cannot see.
#[test]
fn the_unread_badge_is_a_plural_form() {
    let catalog = shipped();

    assert!(
        catalog.has_singular("a11y_unread_count"),
        "the unread badge reaches one whenever a folder holds a single unread message",
    );
    assert!(
        catalog
            .accessors()
            .all(|m| m.key != "a11y_unread_count_one"),
        "and its singular is reached through that accessor, never named by a client",
    );
}

/// **The regression.** A partner that exists but reads identically to its plural is a pair that
/// changes nothing, and it fails exactly where the bug was: French inflects the participle, so
/// "1 non lus" was what a screen reader announced for a folder holding one message.
///
/// French alone, deliberately. The other six shipped languages word this badge without number
/// agreement (English "unread", Dutch "ongelezen", German "ungelesen", Spanish "sin leer",
/// Italian "da leggere", Portuguese "por ler"), so their two forms are the same sentence and
/// asserting a difference there would be asserting bad grammar.
#[test]
fn french_inflects_the_unread_badge_and_the_other_languages_do_not() {
    let catalog = shipped();

    assert_ne!(
        template(&catalog, "a11y_unread_count", "fr"),
        template(&catalog, "a11y_unread_count_one", "fr"),
        "French agrees the participle with the count, so the singular drops the s",
    );
    for locale in ["en", "nl", "de", "es", "it", "pt"] {
        assert_eq!(
            template(&catalog, "a11y_unread_count", locale),
            template(&catalog, "a11y_unread_count_one", locale),
            "{locale} words this badge without number agreement",
        );
    }
}
