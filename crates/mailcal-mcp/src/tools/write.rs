//! The action tools: five mailbox mutations, `create_draft`, and the gated `send_message`.
//!
//! Every mutation here dispatches the **same** core action the user's own swipe does, so an
//! assistant's archive happens in their list, visibly, with the same optimistic hide and the same
//! undo story. That is the deliberate asymmetry with the read side: a write goes through the
//! user's door precisely so it is not invisible.

use std::collections::HashSet;

use mailcal_app::{MailActionError, SendActionError};
use serde_json::Value;

use super::{ToolContext, ToolFailure};
use crate::{
    backend::{AgentDraft, ComposerError},
    policy::{self, Budget},
    schema::{ActionOut, DraftArgs, MarkReadArgs, MessageArgs, SetFlaggedArgs},
    tools::read::{parse, structured},
};

/// Runs an action tool, or returns `None` if `name` is not one.
pub(super) async fn call(
    ctx: &ToolContext,
    budget: &mut Budget,
    name: &str,
    args: Value,
) -> Option<Result<Value, ToolFailure>> {
    Some(match name {
        "mark_read" => mark_read(ctx, args).await,
        "set_flagged" => set_flagged(ctx, args).await,
        "archive_message" => act(ctx, args, "archived", Action::Archive).await,
        "move_to_trash" => act(ctx, args, "moved to Trash", Action::Trash).await,
        "mark_as_spam" => act(ctx, args, "reported as junk", Action::Spam).await,
        "create_draft" => create_draft(ctx, budget, args).await,
        "send_message" => send_message(ctx, args).await,
        _ => return None,
    })
}

/// Which folder move an action tool performs.
enum Action {
    Archive,
    Trash,
    Spam,
}

async fn mark_read(ctx: &ToolContext, args: Value) -> Result<Value, ToolFailure> {
    let args: MarkReadArgs = parse(args)?;
    ctx.require_exposed(&args.account)?;
    ctx.backend
        .mark_read(&args.account, &args.key, args.read)
        .await
        .map_err(action_failure)?;
    done(
        &args.account,
        Some(args.key),
        if args.read {
            "marked read"
        } else {
            "marked unread"
        },
    )
}

async fn set_flagged(ctx: &ToolContext, args: Value) -> Result<Value, ToolFailure> {
    let args: SetFlaggedArgs = parse(args)?;
    ctx.require_exposed(&args.account)?;
    ctx.backend
        .set_flagged(&args.account, &args.key, args.flagged)
        .await
        .map_err(action_failure)?;
    done(
        &args.account,
        Some(args.key),
        if args.flagged { "flagged" } else { "unflagged" },
    )
}

async fn act(
    ctx: &ToolContext,
    args: Value,
    outcome: &str,
    action: Action,
) -> Result<Value, ToolFailure> {
    let args: MessageArgs = parse(args)?;
    ctx.require_exposed(&args.account)?;
    let result = match action {
        Action::Archive => ctx.backend.archive(&args.account, &args.key).await,
        Action::Trash => ctx.backend.trash(&args.account, &args.key).await,
        Action::Spam => ctx.backend.spam(&args.account, &args.key).await,
    };
    result.map_err(action_failure)?;
    done(&args.account, Some(args.key), outcome)
}

// `async` with no inner `await` on purpose: every tool handler shares one shape so the
// dispatcher drives them uniformly, and `open_composer` is deliberately synchronous: a host
// must hop to its UI thread and return rather than block this connection.
#[allow(clippy::unused_async)]
async fn create_draft(
    ctx: &ToolContext,
    budget: &mut Budget,
    args: Value,
) -> Result<Value, ToolFailure> {
    let args: DraftArgs = parse(args)?;
    if let Some(account) = &args.account {
        ctx.require_exposed(account)?;
    }
    if let Some(reply) = &args.reply_to {
        ctx.require_exposed(&reply.account)?;
    }
    // Opening the composer raises and focuses a window: the one user-interface primitive an
    // agent controls here, so it is throttled on its own clock.
    budget.spend_composer().map_err(ToolFailure::Refused)?;
    // Deliberately NOT recipient-guarded. A draft is not a send: the user reads the recipients
    // before pressing Send, which is exactly the review the guard substitutes for when there is
    // no human in the loop. Guarding it too would refuse a legitimate first email to a new
    // contact, which is a thing people do constantly.
    let (to, cc, bcc) = (join(&args.to), join(&args.cc), join(&args.bcc));
    let account = args.account.clone();
    ctx.backend
        .open_composer(AgentDraft {
            account: args.account,
            to,
            cc,
            bcc,
            subject: args.subject,
            body_text: args.body_text,
            reply_to_account: args.reply_to.as_ref().map(|reply| reply.account.clone()),
            reply_to_key: args.reply_to.map(|reply| reply.key),
        })
        .map_err(|ComposerError::NoHostComposer| {
            ToolFailure::Refused(
                "this build has no composer to open a draft in; send the message another way"
                    .to_owned(),
            )
        })?;
    done(
        account.as_deref().unwrap_or_default(),
        None,
        "opened a draft in the user's composer; they have not sent it yet",
    )
}

async fn send_message(ctx: &ToolContext, args: Value) -> Result<Value, ToolFailure> {
    // Reachable only when the tool was listed, which only happens with direct send on. Checked
    // again here anyway: a client that remembers a tool name across a settings change must not
    // be able to call it after the user turned it off.
    let config = ctx.config();
    if !config.allow_direct_send {
        return Err(ToolFailure::Refused(
            "direct sending is off; use create_draft, or turn it on in Settings → Advanced"
                .to_owned(),
        ));
    }
    let args: DraftArgs = parse(args)?;
    if let Some(account) = &args.account {
        ctx.require_exposed(account)?;
    }
    if config.require_known_recipient {
        ctx.check_recipients(&args).await?;
    }
    ctx.backend
        .send_plain(
            args.account.as_deref(),
            &args.to,
            &args.cc,
            &args.bcc,
            args.subject,
            args.body_text,
        )
        .await
        .map_err(send_failure)?;
    done(args.account.as_deref().unwrap_or_default(), None, "sent")
}

impl ToolContext {
    /// Refuses the send unless every recipient is someone the user already corresponds with.
    ///
    /// This is the control that actually blocks *"forward my mailbox to attacker@evil.tld"*: an
    /// injected instruction can compose any message it likes, but it cannot make the address it
    /// wants appear in the user's own Sent-mail history. Pure and deterministic: the lookups
    /// happen here, the decision in `policy::recipients_are_known`, which is unit-tested.
    async fn check_recipients(&self, args: &DraftArgs) -> Result<(), ToolFailure> {
        let recipients: Vec<String> = args
            .to
            .iter()
            .chain(&args.cc)
            .chain(&args.bcc)
            .cloned()
            .collect();
        let mut known: HashSet<String> = HashSet::new();
        for recipient in &recipients {
            for found in self.backend.known_recipients(recipient).await {
                known.insert(found.to_ascii_lowercase());
            }
        }
        let own: Vec<String> = self
            .backend
            .accounts()
            .await
            .into_iter()
            .map(|row| row.email)
            .collect();
        policy::recipients_are_known(&recipients, &own, &known).map_err(ToolFailure::Refused)
    }
}

/// Joins recipient addresses into the comma-separated field shape the composer takes.
fn join(addresses: &[String]) -> String {
    addresses
        .iter()
        .map(|address| address.trim())
        .filter(|address| !address.is_empty())
        .collect::<Vec<_>>()
        .join(", ")
}

/// The successful result every action returns.
fn done(account: &str, key: Option<String>, outcome: &str) -> Result<Value, ToolFailure> {
    structured(&ActionOut {
        account: account.to_owned(),
        key,
        outcome: outcome.to_owned(),
    })
}

/// Renders a core action failure as a refusal the model can relay.
///
/// Every variant maps to a distinct sentence, which is the whole point of plumbing the result
/// through: without it, a Gmail account (whose refreshing wrapper does not forward
/// `edit_mail`) would silently apply nothing and an assistant would report success.
fn action_failure(error: MailActionError) -> ToolFailure {
    ToolFailure::Refused(
        match error {
            MailActionError::UnknownAccount => "no such account",
            MailActionError::UnknownMessage => "no such message in that account",
            MailActionError::NoProvider => {
                "that account cannot be changed right now; it is offline, still connecting, or \
                 its provider does not support mail actions"
            }
            MailActionError::NoTargetFolder => "that account has no folder to move the message to",
            MailActionError::Rejected => "the mail server refused the change",
        }
        .to_owned(),
    )
}

/// Renders a core send failure as a refusal the model can relay.
fn send_failure(error: SendActionError) -> ToolFailure {
    ToolFailure::Refused(
        match error {
            SendActionError::UnknownAccount => "no account to send from",
            SendActionError::NoRecipients => "the message had no recipients",
            SendActionError::DraftFailed => "the message could not be assembled",
            SendActionError::Rejected => "the mail server refused the message",
        }
        .to_owned(),
    )
}
