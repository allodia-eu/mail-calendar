//! Schema/handler agreement.
//!
//! `schemars` was added for exactly one failure: a client sends precisely what a tool's published
//! schema documents and gets a parse error, because the schema and the deserializer drifted
//! apart. Deriving both from one struct makes that impossible *by construction*, but only as
//! long as the schema a client actually reads is the one derived from the struct a handler
//! actually parses. That is what these check: a canonical example per tool, round-tripped
//! through the published `required`/`additionalProperties` **and** through serde.

use std::collections::BTreeSet;

use serde_json::{Value, json};

use crate::{
    config::McpConfig,
    schema::{
        AccountArgs, DraftArgs, ListMessagesArgs, MarkReadArgs, MessageArgs, NoArgs, SearchArgs,
        SetFlaggedArgs,
    },
    tools,
};

/// One tool's canonical, minimal-but-complete call.
fn examples() -> Vec<(&'static str, Value)> {
    vec![
        ("list_accounts", json!({})),
        ("list_folders", json!({"account": "work"})),
        (
            "list_messages",
            json!({"account": "work", "folder": "inbox", "unread_only": true, "offset": 0, "limit": 10}),
        ),
        (
            "search_messages",
            json!({"query": "invoice", "account": "work", "folder": "inbox", "offset": 0, "limit": 5}),
        ),
        ("get_message", json!({"account": "work", "key": "m1"})),
        (
            "mark_read",
            json!({"account": "work", "key": "m1", "read": true}),
        ),
        (
            "set_flagged",
            json!({"account": "work", "key": "m1", "flagged": true}),
        ),
        ("archive_message", json!({"account": "work", "key": "m1"})),
        ("move_to_trash", json!({"account": "work", "key": "m1"})),
        ("mark_as_spam", json!({"account": "work", "key": "m1"})),
        (
            "create_draft",
            json!({
                "account": "work",
                "to": ["a@b.example"],
                "cc": [],
                "bcc": [],
                "subject": "Hi",
                "body_text": "Hello",
                "reply_to": {"account": "work", "key": "m1"},
            }),
        ),
        (
            "send_message",
            json!({"to": ["a@b.example"], "subject": "Hi", "body_text": "Hello"}),
        ),
    ]
}

/// Deserializes `args` with the same type the named tool's handler uses. Kept beside the
/// examples so a new tool that forgets an arm fails this test rather than passing vacuously.
fn parses(name: &str, args: Value) -> Result<(), String> {
    fn as_<T: serde::de::DeserializeOwned>(args: Value) -> Result<(), String> {
        serde_json::from_value::<T>(args)
            .map(|_| ())
            .map_err(|err| err.to_string())
    }
    match name {
        "list_accounts" => as_::<NoArgs>(args),
        "list_folders" => as_::<AccountArgs>(args),
        "list_messages" => as_::<ListMessagesArgs>(args),
        "search_messages" => as_::<SearchArgs>(args),
        "get_message" | "archive_message" | "move_to_trash" | "mark_as_spam" => {
            as_::<MessageArgs>(args)
        }
        "mark_read" => as_::<MarkReadArgs>(args),
        "set_flagged" => as_::<SetFlaggedArgs>(args),
        "create_draft" | "send_message" => as_::<DraftArgs>(args),
        other => Err(format!("no handler type is wired for {other}")),
    }
}

fn full_listing() -> Vec<tools::Tool> {
    tools::listing(&McpConfig {
        allow_direct_send: true,
        accounts: BTreeSet::from(["work".to_owned()]),
        ..McpConfig::default()
    })
}

#[test]
fn every_tool_in_the_listing_has_a_canonical_example_that_both_validates_and_parses() {
    let examples = examples();
    for tool in full_listing() {
        let (_, args) = examples
            .iter()
            .find(|(name, _)| *name == tool.name)
            .unwrap_or_else(|| panic!("{} has no canonical example in this test", tool.name));

        // Every field the schema marks required is present in the example…
        let required: Vec<&str> = tool.input_schema["required"]
            .as_array()
            .map(|values| values.iter().filter_map(Value::as_str).collect())
            .unwrap_or_default();
        for field in &required {
            assert!(
                args.get(field).is_some(),
                "{}'s schema requires `{field}`, which the example omits",
                tool.name,
            );
        }
        // …every field the example sends is one the schema declares…
        let properties = tool.input_schema["properties"].as_object();
        for field in args.as_object().into_iter().flat_map(serde_json::Map::keys) {
            assert!(
                properties.is_some_and(|properties| properties.contains_key(field)),
                "{} sends `{field}`, which its schema does not declare",
                tool.name,
            );
        }
        // …and the very same JSON parses into the type the handler deserializes.
        parses(tool.name, args.clone())
            .unwrap_or_else(|err| panic!("{}'s example does not parse: {err}", tool.name));
    }
}

#[test]
fn every_input_schema_refuses_unknown_fields() {
    // `additionalProperties: false` is what makes a typo'd argument a loud refusal rather than a
    // silently dropped one, and a silently dropped argument means the tool answers a question
    // nobody asked (`unread_only` misspelled would return read mail as if it were unread).
    for tool in full_listing() {
        assert_eq!(
            tool.input_schema["additionalProperties"],
            Value::Bool(false),
            "{} allows unknown fields",
            tool.name,
        );
    }
}

#[test]
fn every_schema_is_draft_2020_12_as_mcp_requires() {
    for tool in full_listing() {
        let schema = tool.input_schema["$schema"].as_str().unwrap_or_default();
        assert!(
            schema.contains("2020-12"),
            "{} publishes {schema}, not draft 2020-12",
            tool.name,
        );
    }
}

#[test]
fn no_published_schema_states_its_type_as_an_array() {
    // `schemars` renders every `Option<T>` as `"type": ["string", "null"]`. That is legal JSON
    // Schema and means what it says, but several MCP clients read `type` as a single string and
    // either reject the tool or drop the constraint, so a tool that validates in one client fails
    // in the next. `SplitArrayTypes` normalises it to `anyOf` at generation.
    //
    // Asserted over the whole published surface; walking every subschema of every input AND
    // output schema; rather than by testing the transform in isolation. The transform skips a
    // schema that already carries `anyOf`, so only the result is evidence: a type added later
    // that lands in that skip lights this up instead of shipping a schema half the clients drop.
    fn walk(node: &Value, path: &str, found: &mut Vec<String>) {
        match node {
            Value::Object(fields) => {
                if fields.get("type").is_some_and(Value::is_array) {
                    found.push(path.to_owned());
                }
                for (key, value) in fields {
                    walk(value, &format!("{path}.{key}"), found);
                }
            }
            Value::Array(items) => {
                for (index, item) in items.iter().enumerate() {
                    walk(item, &format!("{path}[{index}]"), found);
                }
            }
            _ => {}
        }
    }

    let mut found = Vec::new();
    for tool in full_listing() {
        walk(
            &tool.input_schema,
            &format!("{}.inputSchema", tool.name),
            &mut found,
        );
        walk(
            &tool.output_schema,
            &format!("{}.outputSchema", tool.name),
            &mut found,
        );
    }
    assert!(
        found.is_empty(),
        "array-form `type` still published at: {found:#?}"
    );
}

#[test]
fn an_optional_field_still_admits_both_its_value_and_null() {
    // The contract the rewrite must preserve, so it cannot pass by emitting something merely
    // well-formed. `anyOf` has to carry BOTH branches; dropping the null one would turn an
    // optional argument into one a client may not explicitly clear.
    let schema = crate::schema::schema_for::<crate::schema::ListMessagesArgs>();
    let folder = &schema["properties"]["folder"];
    let branches: BTreeSet<&str> = folder["anyOf"]
        .as_array()
        .expect("an anyOf, not an array-form type")
        .iter()
        .map(|branch| branch["type"].as_str().unwrap())
        .collect();
    assert_eq!(branches, BTreeSet::from(["string", "null"]));
    assert!(
        folder.get("type").is_none(),
        "the array-form `type` must be gone, not merely joined by an anyOf: {folder}",
    );
}

#[test]
fn a_tool_that_omits_a_required_argument_is_rejected_by_the_deserializer_too() {
    // The negative case, so the agreement test above cannot pass vacuously: if the deserializer
    // were lenient, "the example parses" would prove nothing.
    assert!(parses("list_folders", json!({})).is_err());
    assert!(parses("mark_read", json!({"account": "work", "key": "m1"})).is_err());
}
