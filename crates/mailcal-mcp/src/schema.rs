//! The request and result shapes of every tool, and the JSON Schemas derived from them.
//!
//! One type per tool, deriving `Deserialize` **and** `JsonSchema` from the same fields. That is
//! the point: the schema a client reads and the deserializer that then parses what it sends are
//! generated from one definition, so a client cannot send exactly what a tool documented and get
//! a parse error. Every request is `deny_unknown_fields`, which is what emits
//! `additionalProperties: false`: so a typo'd argument is refused loudly rather than silently
//! dropped and the call answered as though it meant something else.
//!
//! `account` and `key` always travel together, mirroring `MessageRef::from_parts` in the core:
//! a provider key is unique only *within* an account, so a bare key could route an action into
//! the wrong mailbox. Making the pair unrepresentable-apart at the MCP boundary keeps that class
//! of bug out of the adapter too, and every message-carrying result echoes both back.

use schemars::{
    JsonSchema, Schema, SchemaGenerator,
    generate::SchemaSettings,
    transform::{Transform, transform_subschemas},
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

/// Derives one type's JSON Schema as a draft-2020-12 object with **no `$defs`** and no
/// array-form `type`.
///
/// Subschemas are inlined because an MCP `inputSchema` is consumed by a dozen different clients
/// and `$ref`/`$defs` support across them is uneven; a self-contained schema is understood by
/// all of them. The schemas here are shallow, so inlining costs nothing.
///
/// `SplitArrayTypes` is applied for the same reason at the same single point: every tool's
/// schema in this crate is generated here, so normalising once covers the whole surface and
/// cannot be forgotten on a type added later.
pub fn schema_for<T: JsonSchema>() -> Value {
    let settings = SchemaSettings::draft2020_12().with(|settings| {
        settings.inline_subschemas = true;
        settings.transforms.push(Box::new(SplitArrayTypes));
    });
    let schema: Schema = SchemaGenerator::new(settings).into_root_schema_for::<T>();
    schema.to_value()
}

/// Rewrites `"type": ["string", "null"]` into `"anyOf": [{"type": "string"}, {"type": "null"}]`.
///
/// Both forms are legal JSON Schema and mean the same thing, and `schemars` emits the array form
/// for every `Option<T>`. The trouble is downstream: **several MCP clients read `type` as a single
/// string**, and either reject the tool outright or quietly drop the constraint: so a tool that
/// validates cleanly in one client fails in the next, which is the worst shape an interop bug can
/// take. Normalising here means what this server publishes is the form every client reads.
///
/// Sibling keywords are deliberately left where they are rather than pushed into the non-null
/// branch. A validation keyword applies only to the instance types it is defined for, `maxLength`
/// to strings, `minimum` to numbers: so it is ignored against `null` either way, and moving them
/// would rewrite far more of each schema for no change in meaning.
///
/// A schema that already carries `anyOf` is left alone: overwriting it would silently discard a
/// constraint. Nothing in this crate's surface generates that combination today, and the test in
/// `tests_schema` asserts the *result* is free of array-form `type` rather than trusting this
/// transform to have covered every case.
#[derive(Debug, Clone, Copy)]
struct SplitArrayTypes;

impl Transform for SplitArrayTypes {
    fn transform(&mut self, schema: &mut Schema) {
        transform_subschemas(self, schema);
        let Some(object) = schema.as_object_mut() else {
            return;
        };
        if object.contains_key("anyOf") {
            return;
        }
        let Some(types) = object.get("type").and_then(Value::as_array) else {
            return;
        };
        let branches: Vec<Value> = types.iter().map(|one| json!({ "type": one })).collect();
        if branches.len() < 2 {
            return;
        }
        object.remove("type");
        object.insert("anyOf".to_owned(), Value::Array(branches));
    }
}

/// `list_accounts` takes nothing.
#[derive(Debug, Clone, Default, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct NoArgs {}

/// `list_folders`.
#[derive(Debug, Clone, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct AccountArgs {
    /// The account id, from `list_accounts`.
    pub account: String,
}

/// `list_messages`.
#[derive(Debug, Clone, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ListMessagesArgs {
    /// The account id, from `list_accounts`.
    pub account: String,
    /// The folder key, from `list_folders`. Omit to list the account's whole mailbox.
    #[serde(default)]
    pub folder: Option<String>,
    /// Only unread messages.
    #[serde(default)]
    pub unread_only: bool,
    /// How many messages to skip, for paging.
    #[serde(default)]
    pub offset: usize,
    /// How many messages to return (1-50, default 20).
    #[serde(default)]
    pub limit: Option<usize>,
}

/// `search_messages`.
#[derive(Debug, Clone, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SearchArgs {
    /// What to search for.
    pub query: String,
    /// Narrow to one account. Omit to search every exposed account.
    #[serde(default)]
    pub account: Option<String>,
    /// Narrow to one folder. Requires `account`.
    #[serde(default)]
    pub folder: Option<String>,
    /// How many hits to skip, for paging.
    #[serde(default)]
    pub offset: usize,
    /// How many hits to return (1-50, default 20).
    #[serde(default)]
    pub limit: Option<usize>,
}

/// `get_message`, and every per-message action.
#[derive(Debug, Clone, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct MessageArgs {
    /// The account the message belongs to.
    pub account: String,
    /// The message's key, from a list or search result.
    pub key: String,
}

/// `mark_read`.
#[derive(Debug, Clone, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct MarkReadArgs {
    /// The account the message belongs to.
    pub account: String,
    /// The message's key.
    pub key: String,
    /// `true` to mark it read, `false` to mark it unread.
    pub read: bool,
}

/// `set_flagged`.
#[derive(Debug, Clone, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SetFlaggedArgs {
    /// The account the message belongs to.
    pub account: String,
    /// The message's key.
    pub key: String,
    /// `true` to flag it, `false` to clear the flag.
    pub flagged: bool,
}

/// Which message a draft replies to.
#[derive(Debug, Clone, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ReplyTo {
    /// The account the original belongs to.
    pub account: String,
    /// The original's key.
    pub key: String,
}

/// `create_draft` and `send_message`.
#[derive(Debug, Clone, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DraftArgs {
    /// The account to send from. Omit to use the app's own default.
    #[serde(default)]
    pub account: Option<String>,
    /// The `To` recipients.
    pub to: Vec<String>,
    /// The `Cc` recipients.
    #[serde(default)]
    pub cc: Vec<String>,
    /// The `Bcc` recipients.
    #[serde(default)]
    pub bcc: Vec<String>,
    /// The subject.
    pub subject: String,
    /// The body, as plain text.
    pub body_text: String,
    /// The message this replies to, so the draft threads. `create_draft` only.
    #[serde(default)]
    pub reply_to: Option<ReplyTo>,
}

/// One account in a `list_accounts` result.
#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct AccountOut {
    /// The account's stable id; what every other tool's `account` argument takes.
    pub account: String,
    /// The account's own email address.
    pub address: String,
}

/// The `list_accounts` result.
#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct AccountsOut {
    /// The accounts the user has exposed to assistants. Empty means they have exposed none,
    /// which is the default: not that they have no mail.
    pub accounts: Vec<AccountOut>,
}

/// One folder in a `list_folders` result.
#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct FolderOut {
    /// The folder key; what a list or search `folder` argument takes.
    pub key: String,
    /// The folder's display name.
    pub name: String,
    /// Its special role (`inbox`, `sent`, `archive`, `junk`, `trash`, …), or `null` for an
    /// ordinary folder.
    pub role: Option<String>,
}

/// The `list_folders` result.
#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct FoldersOut {
    /// The account these folders belong to.
    pub account: String,
    /// The folders, special ones first in canonical order.
    pub folders: Vec<FolderOut>,
}

/// One message summary. Deliberately **no body**; see `tools::read`.
#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct MessageSummary {
    /// The owning account.
    pub account: String,
    /// The message's key, for `get_message` and every action.
    pub key: String,
    /// The subject.
    pub subject: String,
    /// The sender.
    pub from: String,
    /// When it arrived, as an RFC 3339 UTC instant.
    pub date: String,
    /// Whether it is unread.
    pub unread: bool,
    /// Whether it is flagged.
    pub flagged: bool,
    /// Whether it has an attachment.
    pub has_attachment: bool,
}

/// A page of message summaries.
#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct MessagesOut {
    /// The summaries, newest first.
    pub messages: Vec<MessageSummary>,
    /// How many messages match in total, before paging.
    pub total: usize,
    /// Where this page starts.
    pub offset: usize,
    /// `true` when older mail exists that raising `offset` cannot reach: the listing is cut
    /// from a bounded newest-first window, not a cursor over the whole mailbox.
    pub older_mail_unreachable: bool,
    /// How many months of mail this device holds for the accounts read, or absent when it
    /// holds all of it. Mail older than this was never downloaded, so no query here can find
    /// it: an empty answer means "not in the last N months", not "no such message".
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sync_depth_months: Option<u16>,
}

/// One message in full, body included.
#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct MessageOut {
    /// The owning account.
    pub account: String,
    /// The message's key.
    pub key: String,
    /// The subject.
    pub subject: String,
    /// The sender.
    pub from: String,
    /// The `To` recipients, comma-joined.
    pub to: String,
    /// The `Cc` recipients, comma-joined.
    pub cc: String,
    /// When it arrived, as an RFC 3339 UTC instant.
    pub date: String,
    /// Whether it is unread. Reading it through this tool does not change it.
    pub unread: bool,
    /// Whether it is flagged.
    pub flagged: bool,
    /// The names of its attachments. Names only: no bytes cross this boundary.
    pub attachments: Vec<String>,
    /// The body as plain text, wrapped in an `<untrusted-message-content>` fence. See
    /// `policy::fence`.
    pub body: String,
}

/// What every write tool returns.
#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct ActionOut {
    /// The account acted on.
    pub account: String,
    /// The message acted on, when the action named one.
    pub key: Option<String>,
    /// What happened, in one short phrase.
    pub outcome: String,
}
