//! The `notice` lookup every target gets: a notice's words arrive as a catalog key and its string
//! arguments (`docs/notices.md` rule 11), and this turns them into text through the typed
//! accessors, so no client keeps a switch of its own.
//!
//! Generated as a switch over the `notice_` keys, the way the endonym lookup is, because Android
//! resolves a string by resource id rather than by name. An unknown key yields the key itself,
//! and a missing argument the empty string: a newer core never crashes an older client.

use std::fmt::Write;

use crate::model::{Catalog, Message, NOTICE_PREFIX, pascal_case};

fn notices(catalog: &Catalog) -> impl Iterator<Item = &Message> {
    catalog
        .accessors()
        .filter(|message| message.key.starts_with(NOTICE_PREFIX))
}

/// `L10n.notice(_:_:)` for the Swift accessor `enum`.
pub(crate) fn swift(catalog: &Catalog) -> String {
    let mut out = String::from(
        "\n    /// A notice's words, from the catalog key and arguments the core sent\n\
         \x20   /// (`docs/notices.md`). An unknown key yields the key itself.\n\
         \x20   public static func notice(_ key: String, _ args: [String: String]) -> String {\n\
         \x20       switch key {\n",
    );
    for message in notices(catalog) {
        let args = message
            .placeholders
            .iter()
            .map(|name| format!("{name}: args[\"{name}\"] ?? \"\""))
            .collect::<Vec<_>>()
            .join(", ");
        let _ = writeln!(
            out,
            "        case \"{key}\": return {key}({args})",
            key = message.key
        );
    }
    out.push_str("        default: return key\n        }\n    }\n");
    out
}

/// `L10n.notice(ctx, key, args)` for the Kotlin accessor object.
pub(crate) fn kotlin(catalog: &Catalog) -> String {
    let mut out = String::from(
        "\n    // A notice's words, from the catalog key and arguments the core sent\n\
         \x20   // (docs/notices.md). An unknown key yields the key itself.\n\
         \x20   fun notice(ctx: Context, key: String, args: Map<String, String>): String = when (key) {\n",
    );
    for message in notices(catalog) {
        let args = std::iter::once("ctx".to_owned())
            .chain(
                message
                    .placeholders
                    .iter()
                    .map(|name| format!("args[\"{name}\"] ?: \"\"")),
            )
            .collect::<Vec<_>>()
            .join(", ");
        let _ = writeln!(out, "        \"{key}\" -> {key}({args})", key = message.key);
    }
    out.push_str("        else -> key\n    }\n");
    out
}

/// `L10n.Notice(key, args)` for the C# accessor class.
pub(crate) fn csharp(catalog: &Catalog) -> String {
    let mut out = String::from(
        "\n    /// <summary>\n\
         \x20   /// A notice's words, from the catalog key and arguments the core sent\n\
         \x20   /// (docs/notices.md). An unknown key yields the key itself.\n\
         \x20   /// </summary>\n\
         \x20   public static string Notice(\n\
         \x20       string key, System.Collections.Generic.IReadOnlyDictionary<string, string> args\n\
         \x20   ) => key switch\n\
         \x20   {\n",
    );
    for message in notices(catalog) {
        let args = message
            .placeholders
            .iter()
            .map(|name| format!("NoticeArg(args, \"{name}\")"))
            .collect::<Vec<_>>()
            .join(", ");
        let _ = writeln!(
            out,
            "        \"{key}\" => {name}({args}),",
            key = message.key,
            name = pascal_case(&message.key)
        );
    }
    out.push_str(
        "        _ => key,\n\
         \x20   };\n\n\
         \x20   private static string NoticeArg(\n\
         \x20       System.Collections.Generic.IReadOnlyDictionary<string, string> args, string name\n\
         \x20   ) => args.TryGetValue(name, out var value) ? value : \"\";\n",
    );
    out
}

/// `notice(key, args)` for the Rust module.
pub(crate) fn rust(catalog: &Catalog) -> String {
    let mut out = String::from(
        "/// A notice's words, from the catalog key and arguments the core sent\n\
         /// (`docs/notices.md`). An unknown key yields the key itself.\n\
         pub(crate) fn notice(key: &str, args: &std::collections::HashMap<String, String>) -> String {\n\
         \x20   let arg = |name: &str| args.get(name).map_or(\"\", String::as_str);\n\
         \x20   match key {\n",
    );
    for message in notices(catalog) {
        let args = message
            .placeholders
            .iter()
            .map(|name| format!("arg(\"{name}\")"))
            .collect::<Vec<_>>()
            .join(", ");
        // A message with no placeholder is a `&'static str`, one with any a `String`.
        let owned = if message.placeholders.is_empty() {
            ".to_owned()"
        } else {
            ""
        };
        let _ = writeln!(
            out,
            "        {key:?} => {key}({args}){owned},",
            key = message.key
        );
    }
    out.push_str("        _ => key.to_owned(),\n    }\n}\n\n");
    out
}

#[cfg(test)]
#[path = "notice_lookup_tests.rs"]
mod tests;
