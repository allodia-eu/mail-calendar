//! The five read tools.
//!
//! # Why a listing carries no bodies
//!
//! `list_messages` and `search_messages` return subject, sender, date and flags. Only
//! `get_message` returns a body, and only for one message at a time.
//!
//! That is the single most effective bound on prompt injection available here. A message body is
//! attacker-authored text entering a model's context; a search that returned bodies would let
//! one call (*"find everything about invoices"*) drop fifty hostile bodies in at once, and it
//! only takes one of them landing to steer what happens next. Forcing a deliberate, per-message
//! `get_message` keeps the blast radius at one message the assistant had a reason to open.

use mailcal_app::{MessageDetail, MessagePage};
use mailcal_viewmodel::{FolderRole, FolderRow, SearchHorizon};
use serde_json::Value;

use super::{ToolContext, ToolFailure};
use crate::{
    config::McpConfig,
    policy,
    schema::{
        AccountArgs, AccountOut, AccountsOut, FolderOut, FoldersOut, ListMessagesArgs, MessageArgs,
        MessageOut, MessageSummary, MessagesOut, SearchArgs,
    },
};

/// Runs a read tool, or returns `None` if `name` is not one.
pub(super) async fn call(
    ctx: &ToolContext,
    name: &str,
    args: Value,
) -> Option<Result<Value, ToolFailure>> {
    Some(match name {
        "list_accounts" => list_accounts(ctx).await,
        "list_folders" => list_folders(ctx, args).await,
        "list_messages" => list_messages(ctx, args).await,
        "search_messages" => search_messages(ctx, args).await,
        "get_message" => get_message(ctx, args).await,
        _ => return None,
    })
}

async fn list_accounts(ctx: &ToolContext) -> Result<Value, ToolFailure> {
    // Filtered to the allow list, so an assistant is never even told which other mailboxes are
    // configured, that set is itself a disclosure the user did not agree to.
    let config = ctx.config();
    let accounts = ctx
        .backend
        .accounts()
        .await
        .into_iter()
        .filter(|row| policy::account_is_exposed(&config, &row.id))
        .map(|row| AccountOut {
            account: row.id,
            address: row.email,
        })
        .collect();
    structured(&AccountsOut { accounts })
}

async fn list_folders(ctx: &ToolContext, args: Value) -> Result<Value, ToolFailure> {
    let args: AccountArgs = parse(args)?;
    ctx.require_exposed(&args.account)?;
    let folders = ctx
        .backend
        .folders(&args.account)
        .await
        .into_iter()
        .map(folder_out)
        .collect();
    structured(&FoldersOut {
        account: args.account,
        folders,
    })
}

async fn list_messages(ctx: &ToolContext, args: Value) -> Result<Value, ToolFailure> {
    let args: ListMessagesArgs = parse(args)?;
    ctx.require_exposed(&args.account)?;
    let page = ctx
        .backend
        .folder_page(
            &args.account,
            args.folder.as_deref(),
            args.unread_only,
            args.offset,
            McpConfig::page_size(args.limit),
        )
        .await;
    structured(&messages_out(page))
}

async fn search_messages(ctx: &ToolContext, args: Value) -> Result<Value, ToolFailure> {
    let args: SearchArgs = parse(args)?;
    if let Some(account) = &args.account {
        ctx.require_exposed(account)?;
    }
    if args.account.is_none() && args.folder.is_some() {
        return Err(ToolFailure::BadArgs(
            "`folder` narrows within one account, so it requires `account` too".to_owned(),
        ));
    }
    // The query's LENGTH, never the query: what someone searches their own mail for is content
    // (`docs/logging.md`), and a support log must stay safe to attach to a ticket.
    log::debug!(
        "mcp: search over a {}-char query, scoped to {}",
        args.query.chars().count(),
        if args.account.is_some() {
            "one account"
        } else {
            "every exposed account"
        },
    );
    let page = ctx
        .backend
        .search(
            &args.query,
            args.account.as_deref(),
            args.folder.as_deref(),
            args.offset,
            McpConfig::page_size(args.limit),
        )
        .await;
    // An unscoped search runs across every configured account, including ones the user did not
    // expose: so the hits are filtered here rather than trusting the scope to have done it.
    let config = ctx.config();
    let mut out = messages_out(page);
    out.messages
        .retain(|row| policy::account_is_exposed(&config, &row.account));
    structured(&out)
}

async fn get_message(ctx: &ToolContext, args: Value) -> Result<Value, ToolFailure> {
    let args: MessageArgs = parse(args)?;
    ctx.require_exposed(&args.account)?;
    let detail = ctx
        .backend
        .message(&args.account, &args.key)
        .await
        .ok_or_else(|| ToolFailure::Refused("no such message in that account".to_owned()))?;
    if detail.load_error {
        return Err(ToolFailure::Refused(
            "the message body could not be fetched: the account may be offline".to_owned(),
        ));
    }
    structured(&message_out(detail))
}

/// Maps a core folder row onto the wire shape, with the role as a lowercase word a model can
/// reason about rather than a numeric discriminant.
fn folder_out(row: FolderRow) -> FolderOut {
    FolderOut {
        key: row.key,
        name: row.name,
        role: row.role.map(|role| {
            match role {
                FolderRole::Inbox => "inbox",
                FolderRole::Drafts => "drafts",
                FolderRole::Sent => "sent",
                FolderRole::Archive => "archive",
                FolderRole::Junk => "junk",
                FolderRole::Trash => "trash",
                FolderRole::Other => "other",
            }
            .to_owned()
        }),
    }
}

fn messages_out(page: MessagePage) -> MessagesOut {
    MessagesOut {
        messages: page
            .rows
            .into_iter()
            .map(|row| MessageSummary {
                account: row.account,
                key: row.key,
                subject: row.subject,
                from: row.from,
                date: row.date,
                unread: row.unread,
                flagged: row.flagged,
                has_attachment: row.has_attachment,
            })
            .collect(),
        total: page.total,
        offset: page.offset,
        older_mail_unreachable: page.windowed,
        sync_depth_months: match page.horizon {
            Some(SearchHorizon::Months(months)) => Some(months),
            Some(SearchHorizon::AllTime) | None => None,
        },
    }
}

fn message_out(detail: MessageDetail) -> MessageOut {
    MessageOut {
        account: detail.account,
        key: detail.key,
        subject: detail.subject,
        from: detail.from,
        to: detail.to,
        cc: detail.cc,
        date: detail.date,
        unread: detail.unread,
        flagged: detail.flagged,
        attachments: detail.attachment_names,
        // The one place a body crosses this boundary, and the one place the fence goes on.
        body: policy::fence(&detail.body_text),
    }
}

/// Deserializes a tool's arguments against the very type its published schema was derived from.
/// Shared with the write tools, so both halves of the surface parse identically.
pub(super) fn parse<T: serde::de::DeserializeOwned>(args: Value) -> Result<T, ToolFailure> {
    serde_json::from_value(args).map_err(|err| ToolFailure::BadArgs(err.to_string()))
}

/// Serializes a result type into the `structuredContent` value.
pub(super) fn structured<T: serde::Serialize>(value: &T) -> Result<Value, ToolFailure> {
    serde_json::to_value(value).map_err(|err| ToolFailure::Internal(err.to_string()))
}
