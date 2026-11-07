//! The `notice` lookup on every target, and the rule that keeps a notice's words to strings.

use std::collections::BTreeMap;

use crate::{
    brand::Brand,
    emit_kotlin, emit_rust, emit_swift, emit_winui,
    model::{Catalog, Raw},
    validate,
};

fn raw(extra: &[(&str, &str)]) -> Raw {
    let en: BTreeMap<String, String> = [
        (
            "notice_signin_expired",
            "The sign-in for {address} no longer works.",
        ),
        ("notice_offline", "You’re offline."),
        ("greeting", "Hi {name}"),
        ("settings_language_en", "English"),
    ]
    .iter()
    .chain(extra)
    .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
    .collect();
    Raw {
        base: "en".to_owned(),
        locales: vec!["en".to_owned()],
        maps: BTreeMap::from([("en".to_owned(), en)]),
    }
}

fn brand() -> Brand {
    Brand {
        name: "Fixture Mail".to_owned(),
        id: "org.fixture.client".to_owned(),
        mcp_server_name: "fixture-mail".to_owned(),
        mcp_relay_name: "fixture-mcp".to_owned(),
        data_dir_name: "fixture".to_owned(),
        windows_data_dir: "Fixture/Mail".to_owned(),
    }
}

fn file(files: &[(String, String)], name: &str) -> String {
    let Some((_, text)) = files.iter().find(|(path, _)| path.ends_with(name)) else {
        panic!("no {name}");
    };
    text.clone()
}

#[test]
fn every_target_resolves_each_notice_key_and_only_those() {
    let catalog = Catalog::from_raw(&raw(&[]));

    let swift = file(&emit_swift::render(&catalog, &brand()), "L10n.swift");
    assert!(swift.contains("public static func notice(_ key: String, _ args: [String: String])"));
    assert!(swift.contains(
        "case \"notice_signin_expired\": return notice_signin_expired(address: args[\"address\"] ?? \"\")"
    ));
    assert!(swift.contains("case \"notice_offline\": return notice_offline()"));
    assert!(!swift.contains("case \"greeting\""));

    let kotlin = file(
        &emit_kotlin::render(&catalog, "eu.allodia.mailcal"),
        "L10n.kt",
    );
    assert!(kotlin.contains("fun notice(ctx: Context, key: String, args: Map<String, String>)"));
    assert!(kotlin.contains(
        "\"notice_signin_expired\" -> notice_signin_expired(ctx, args[\"address\"] ?: \"\")"
    ));
    assert!(kotlin.contains("\"notice_offline\" -> notice_offline(ctx)"));
    assert!(kotlin.contains("else -> key"));

    let cs = file(
        &emit_winui::render(&catalog, "Allodia.Mailcal", &brand()),
        "L10n.cs",
    );
    assert!(cs.contains("public static string Notice("));
    assert!(cs.contains(
        "\"notice_signin_expired\" => NoticeSigninExpired(NoticeArg(args, \"address\")),"
    ));
    assert!(cs.contains("\"notice_offline\" => NoticeOffline(),"));
    assert!(cs.contains("_ => key,"));

    let rs = file(&emit_rust::render(&catalog, &brand()), "l10n.rs");
    assert!(rs.contains("pub(crate) fn notice(key: &str, args: &std::collections::HashMap<String, String>) -> String"));
    assert!(rs.contains("\"notice_signin_expired\" => notice_signin_expired(arg(\"address\")),"));
    assert!(rs.contains("\"notice_offline\" => notice_offline().to_owned(),"));
    assert!(rs.contains("_ => key.to_owned(),"));
}

#[test]
fn a_notice_key_with_a_number_or_a_plural_form_fails() {
    assert!(validate::check(&raw(&[])).is_ok());

    let counted = validate::check(&raw(&[(
        "notice_skipped",
        "{count} accounts were left out.",
    )]))
    .expect_err("a number cannot arrive as a string argument");
    assert!(counted.contains("notice_skipped"), "{counted}");

    let plural = validate::check(&raw(&[
        ("notice_more", "{name} more"),
        ("notice_more_one", "{name} more"),
    ]))
    .expect_err("a plural form cannot be chosen by the lookup");
    assert!(plural.contains("notice_more_one"), "{plural}");
}
