//! The **reading and composing** preferences: how the message list is grouped, how a reply quotes
//! the original, and what a swipe across a row does.
//!
//! Split out of the parent module, which was at the 500-line limit. Its sibling `display` holds the
//! ones about how the app *looks*.

use serde::{Deserialize, Serialize};

#[allow(unused_imports)] // for the intra-doc links below
use super::Preferences;

/// How a reply or forward quotes the original message, as a persisted app-level default.
/// The composer renders the chosen style; the user may override it per message when
/// [`Preferences::quote_style_per_message`] is on. The two styles are named for what they
/// *are*, not for the mail client that popularized each.
///
/// The `gmail` / `outlook` aliases are the tokens this setting was written under before the
/// rename; they are read so an existing preferences file keeps the user's choice, and are
/// never written back (a save re-serializes under the current name).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum QuoteStyle {
    /// Indent the original in a left-bordered blockquote under "On … wrote:".
    #[default]
    #[serde(alias = "gmail")]
    Indented,
    /// Divide the original off with a rule and a labelled header block, at full width.
    #[serde(alias = "outlook")]
    LineAndHeader,
}

/// How the mailbox message list is grouped, persisted as an app-level default. Formerly a
/// runtime-only toggle, the chosen grouping now survives a restart and is edited in the Settings
/// screen. Threaded groups a mailbox into conversations (newest activity first); Flat lists
/// individual messages (newest first). Defaults to Threaded: the product default.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MessageGrouping {
    /// A flat list of individual messages, newest first.
    Flat,
    /// Conversations grouped by thread, newest activity first.
    #[default]
    Threaded,
}

/// What a swipe across a message row does, as a persisted per-direction default. Delete moves
/// the message to Trash (recoverable), Archive to the account's Archive folder, and Star toggles
/// the flag in place. Both directions default to Delete: the behaviour before the setting existed.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SwipeAction {
    /// Move the message to the account's Trash folder.
    #[default]
    Delete,
    /// Move the message to the account's Archive folder.
    Archive,
    /// Flag (star) the message, leaving it in the list.
    Star,
}
