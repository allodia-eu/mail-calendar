//! Persisted app preferences: the user's chosen display timezone, account sync settings, and
//! reading/composing defaults.
//!
//! These are host **app preferences**, not synced PIM data, so they live in a small
//! TOML file in the app data dir (written next to the account store), not in the engine
//! store. The product-core owns reading/writing them; the engine only acts on the
//! values (the chosen zone for calendar resolution; per-account [`SyncDepth`] cutoffs used to
//! build per-sync windows).

use std::collections::{BTreeMap, BTreeSet};

use mailcal_jurisdiction::Mode;
use serde::{Deserialize, Serialize};

use crate::signatures::AccountSignatureAssignment;

// Which writing style each account drafts in, and the person's own AI endpoint.
mod ai;
// The reading/composing choices: message grouping, quote style, swipe actions.
mod behavior;
mod display;
// Which accounts have their sidebar folder tree shut, and its accessors.
mod file;
mod folder_pane;
// The per-account "may we email the organiser ourselves?" choice, and its accessors.
mod reply_fallback;
// Which signature each account uses, and the accessors that keep no assignment dangling.
mod signature;
// The name an account sends under, its sanitiser, and its accessors.
mod sender_name;
mod sync;

pub use ai::{AiEndpoint, AiPreferences};
pub use behavior::{MessageGrouping, QuoteStyle, SwipeAction};
pub use display::{
    Appearance, CalendarLayout, CalendarPrefs, DEFAULT_VISIBLE_HOURS, DefaultCalendar,
    MAX_VISIBLE_HOURS, MIN_VISIBLE_HOURS, TimeFormat, WeekStart, clamp_visible_hours,
};
pub use file::{load_preferences, preferences_path, save_preferences};
pub use reply_fallback::ReplyFallback;
pub use sender_name::{MAX_SENDER_NAME_CHARS, sanitize_sender_name};
pub use sync::{
    AccountSyncSettings, DEFAULT_POLL_INTERVAL, EffectiveSync, MAX_PUSH_FOLDERS,
    MESSAGE_SIZE_LIMITS_MB, MessageSizeLimit, POLL_INTERVALS, SYNC_DEPTHS, SyncDepth, SyncStrategy,
    cap_push_folders, effective, snap_poll_interval,
};

/// App-level preferences persisted across launches.
///
/// `Default` is written out rather than derived: `calendar_visible_hours` is a `u8` and would
/// derive to `0`.
///
/// More than three booleans, deliberately: this is a flat bag of independent user choices
/// serialized to one TOML file, and grouping them into sub-structs to satisfy a lint would
/// change the on-disk shape for every existing installation.
#[allow(clippy::struct_excessive_bools)]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Preferences {
    /// The user's chosen display timezone (an IANA id like `Europe/Amsterdam`), or
    /// `None` on first boot before one has been adopted from the device.
    pub display_timezone: Option<String>,
    /// The account a **new** message composes from when the unified all-inboxes view is showing
    /// and no single mailbox scopes the choice (an account id). `None`: the default, and the
    /// state after the chosen account is removed; falls back to the first configured account.
    /// Ignored while one account's mailbox is selected: that account sends.
    #[serde(default)]
    pub default_send_account: Option<String>,
    /// The persisted message-list grouping (flat vs threaded). Defaults to Threaded when the
    /// field is absent (an older preferences file, or first boot).
    #[serde(default)]
    pub message_grouping: MessageGrouping,
    /// The default style for quoting an original on reply/forward. Defaults to
    /// [`QuoteStyle::Indented`] when the field is absent (an older preferences file, or first
    /// boot).
    #[serde(default)]
    pub quote_style: QuoteStyle,
    /// Whether the composer offers a per-message quote-style override. Off by default: a reply
    /// or forward silently uses [`Preferences::quote_style`] and shows no picker. Turning it on
    /// is an opt-in for users who want to vary the style message by message; the override is
    /// never persisted; it applies to the one composer window and the app default stands.
    #[serde(default)]
    pub quote_style_per_message: bool,
    /// What swiping a message row **leftwards** (toward the start edge) does. Defaults to Delete.
    #[serde(default)]
    pub swipe_left: SwipeAction,
    /// What swiping a message row **rightwards** (toward the end edge) does. Defaults to Delete.
    #[serde(default)]
    pub swipe_right: SwipeAction,
    /// Which day the calendar's week begins on. Defaults to Monday when the field is absent (an
    /// older preferences file, or first boot).
    #[serde(default)]
    pub week_start: WeekStart,
    /// Whether times render on a 24-hour clock, across mail and calendar. Defaults to 24-hour.
    #[serde(default)]
    pub time_format: TimeFormat,
    /// Whether the app paints itself light, dark, or however the host is set. Defaults to
    /// following the host.
    #[serde(default)]
    pub appearance: Appearance,
    /// How many hours of the day the calendar grid shows at once by default: the horizon a pinch
    /// zooms in and out of, persisted so it is where the user left it. Always within
    /// [`MIN_VISIBLE_HOURS`]..=[`MAX_VISIBLE_HOURS`]; read it through
    /// [`Preferences::visible_hours`], which clamps a hand-edited or corrupt value rather than
    /// trusting it.
    #[serde(default = "display::default_visible_hours")]
    pub calendar_visible_hours: u8,
    /// The shape the calendar opens in: the last one the user chose. Persisted so a pinch to the
    /// day view, or a switch to the month, survives the app being closed.
    #[serde(default)]
    pub calendar_layout: CalendarLayout,
    /// What the user has decided about each calendar (shown/hidden, colour override), keyed by
    /// **account id, then calendar id**.
    ///
    /// Nested rather than flat because a calendar id is only unique *within* its account: two
    /// accounts can each have a calendar called `work`, and a flat map would let hiding one hide
    /// the other. Read it through [`Preferences::calendar`], which returns the default for a
    /// calendar nobody has touched.
    #[serde(default)]
    pub calendars: BTreeMap<String, BTreeMap<String, CalendarPrefs>>,
    /// Which calendar a new event is filed on unless the user picks another in the editor.
    ///
    /// `None` (the state before anyone chose) means "whichever writable calendar comes first",
    /// which is also what a stored choice falls back to once it stops existing or stops being
    /// writable. Resolved in one place, [`crate::preferences::DefaultCalendar`] says why.
    #[serde(default)]
    pub default_calendar: Option<DefaultCalendar>,
    /// Per-account synchronisation behaviour (push vs. poll), keyed by account id. An
    /// account absent here has not been customised and uses the [`effective`] default.
    /// A [`BTreeMap`] so the serialized TOML order is stable across writes.
    #[serde(default)]
    pub accounts: BTreeMap<String, AccountSyncSettings>,
    /// Per-account new-mail notification high-water-marks (RFC3339 `…Z`), keyed by account
    /// id: the newest inbound-Inbox instant a background sync has already reported (or
    /// seeded on first run), so a later pass raises a notification only for strictly-newer
    /// mail. The product core writes these; a [`BTreeMap`] keeps the serialized order stable.
    #[serde(default)]
    pub notify_marks: BTreeMap<String, String>,
    /// Whether the user opted in to sending privacy-preserving product-usage statistics.
    ///
    /// `None` (the state on first boot) means they have **not been asked yet**. `Some(false)`
    /// means they were asked and declined, so the app does not ask again. `Some(true)` means
    /// they opted in. Analytics is off unless this is `Some(true)`: consent is the gate, and
    /// its absence is a refusal, never an assumption. See `docs/analytics.md`.
    #[serde(default)]
    pub analytics_consent: Option<bool>,
    /// Whether the one-time offer to become the OS's default mail app has been put, and what
    /// came of it.
    ///
    /// `None` (the state on first boot) means it has **not been offered yet**, and is the only
    /// value that lets it be offered. `Some(true)` means the user took it, `Some(false)` that
    /// they turned it down or closed it. The distinction changes nothing about whether we ask
    /// again, we do not; it is kept because "you have already made us the default" and "you
    /// said no" are different things to say in Settings. See `docs/os-integration.md`.
    #[serde(default)]
    pub default_mail_app_offer: Option<bool>,
    /// The opaque install id analytics events carry, minted **at the moment of consent** and
    /// cleared on withdrawal: so a user who never consents has nothing written for analytics
    /// at all. It is pure CSPRNG output: not derived from the device, the accounts, the
    /// addresses, or anything else, so it identifies nothing but itself.
    #[serde(default)]
    pub analytics_install_id: Option<String>,
    /// Which version of the consent notice was agreed to. A material change to what we send
    /// bumps the notice version, which re-asks rather than silently widening a stale consent.
    #[serde(default)]
    pub analytics_notice_version: Option<u32>,
    /// When consent was given (RFC3339 `…Z`). GDPR Art. 7(1) requires a controller to be able
    /// to **demonstrate** that consent was given, so the decision is timestamped.
    #[serde(default)]
    pub analytics_consented_at: Option<String>,
    /// Which signature each account uses, keyed by account id; one choice for new messages and
    /// one for replies/forwards. The signatures themselves live in their own store
    /// ([`crate::Signatures`]); this is only the small per-account pointer, so it belongs with
    /// the other per-account preferences. An account absent here has no signature in either
    /// slot. Read it through [`Preferences::account_signature`], which returns the empty
    /// assignment for an account nobody has configured.
    #[serde(default)]
    pub signature_assignments: BTreeMap<String, AccountSignatureAssignment>,
    /// Whether each account may send an invitation reply as email itself when its calendar
    /// server reports it could not deliver one, keyed by account id. An account absent here is
    /// [`ReplyFallback::Ask`] (the default) so an upgrade asks rather than assuming either
    /// way. Read it through [`Preferences::reply_fallback`].
    #[serde(default)]
    pub invitation_reply_fallback: BTreeMap<String, ReplyFallback>,
    /// Extra addresses that are also *this account's own*, keyed by account id: the account's
    /// aliases (`docs/invitations.md` §"Identity is a set").
    ///
    /// An account has one primary identity, but mail arrives at more than one address: an
    /// invitation sent to `info@example.com` on an account whose primary is `alice@example.com`
    /// is still an invitation to **me**, and Outlook treats it as one. Matching an iTIP
    /// `ATTENDEE` against a single identity silently answers "you are not invited to this",
    /// which hides the RSVP the user is waiting for.
    ///
    /// This is the **persisted** half of the address set, and the only half the *calendar grid*
    /// can use: a grid has no message to read delivery headers from. The reading view's card
    /// additionally accepts the addresses a message was actually delivered to, which makes the
    /// common alias case work with no configuration at all.
    ///
    /// Read it through [`Preferences::account_aliases`], which returns an empty slice for an
    /// account nobody has configured. Works on every provider, including plain IMAP/CalDAV.
    #[serde(default)]
    pub account_aliases: BTreeMap<String, Vec<String>>,
    /// The display name each account's outgoing mail is sent under, keyed by account id.
    ///
    /// An account absent here has no name and its mail goes out as a bare address. Read and
    /// written through [`Preferences::sender_name_of`] and
    /// [`Preferences::set_account_sender_name`], which sanitise: this is spliced into a
    /// `From` header, so it may not be taken from the map raw. A [`BTreeMap`] so the
    /// serialized TOML order is stable across writes.
    #[serde(default)]
    pub account_sender_names: BTreeMap<String, String>,
    /// Whether the local MCP server is on, so an AI assistant on this machine can read and act
    /// on mail. Off unless the user turns it on.
    ///
    /// A plain `bool`, deliberately **not** analytics' `Option<bool>` tri-state. Analytics needs
    /// the tri-state because *unasked* is what raises the first-run consent screen, and because
    /// GDPR Art. 7(1) requires a demonstrable, timestamped decision. Neither applies here:
    /// nothing leaves the device, no identifier is written, and there is no prompt: the user
    /// goes looking for the setting. See `docs/mcp.md`.
    #[serde(default)]
    pub mcp_enabled: bool,
    /// Which version of the MCP notice the user agreed to.
    ///
    /// Kept even though there is no first-run prompt, because the *re-ask on widening* mechanic
    /// is exactly right for "the tool set gained the ability to send mail": a material widening
    /// bumps the version, which re-asks rather than silently inheriting a consent given to a
    /// smaller surface. Mirrors `analytics_notice_version`.
    #[serde(default)]
    pub mcp_notice_version: Option<u32>,
    /// The account ids an MCP client may see and act on.
    ///
    /// **Empty by default, and empty exposes nothing.** Turning the server on and granting
    /// access to a mailbox are two separate decisions, and one toggle doing both silently would
    /// be the wrong default in the one place a wrong default costs the most. A [`BTreeSet`] so
    /// the serialized TOML order is stable across writes.
    #[serde(default)]
    pub mcp_accounts: BTreeSet<String>,
    /// Whether an assistant may send mail directly, with no human review. Off by default; with
    /// it off the `send_message` tool does not exist at all and an assistant can only open a
    /// draft for the user to send themselves.
    #[serde(default)]
    pub mcp_allow_direct_send: bool,
    /// Whether a direct send is restricted to people the user already corresponds with;
    /// someone at one of their own account domains, or an address in the Sent-mail recipient
    /// index. **On** by default. This is the control that blocks exfiltration to an address an
    /// injected instruction chose.
    #[serde(default = "default_true")]
    pub mcp_require_known_recipient: bool,
    /// How far an external dispatch may reach. EU-native unless this file says otherwise; no
    /// client offers to change it yet (`docs/ai.md`, "The gate").
    #[serde(default)]
    pub jurisdiction_mode: Mode,
    /// What the AI features remember: each account's writing style and the person's own
    /// endpoint. Boxed, because four of the core's settings states each hold a whole copy of
    /// this struct, and every future that holds the app by value carries all four.
    #[serde(default)]
    pub ai: Box<AiPreferences>,
    /// The accounts whose folder tree is **shut** in the sidebar, by account id.
    ///
    /// The collapsed ones, not the expanded ones, so an account nobody has touched, and
    /// every account on the first launch after this shipped; opens showing its folders.
    /// Read it through [`Preferences::account_expanded`], never off the field: the
    /// inversion is exactly the kind a direct reader gets backwards. A [`BTreeSet`] so the
    /// serialized TOML order is stable across writes.
    #[serde(default)]
    pub collapsed_accounts: BTreeSet<String>,
    /// Whether the sidebar's **All Accounts** group is shut.
    ///
    /// The shut state, not the open one, for the reason `collapsed_accounts` above stores
    /// the collapsed accounts: the group a user has never touched stands open, and
    /// `bool::default()` is `false`. Read it through [`Preferences::unified_expanded`].
    #[serde(default)]
    pub unified_collapsed: bool,
    /// The folders whose own sub-folders are **shut**, by account id then folder key.
    ///
    /// Keyed on both halves because a folder key is unique only within its account: every
    /// provider calls its inbox `inbox`, and the pane holds every account's tree at once
    /// (`docs/folder-pane.md`, rule 14). Nested rather than one set of joined strings because
    /// a folder key is a provider's own string and may hold whatever separator the join
    /// picked. The collapsed ones for the reason `collapsed_accounts` stores them: a folder
    /// nobody has touched shows what is inside it. Read it through
    /// [`Preferences::folder_expanded`].
    #[serde(default)]
    pub collapsed_folders: BTreeMap<String, BTreeSet<String>>,
}

/// The `serde` default for a flag that is on unless the user turns it off.
const fn default_true() -> bool {
    true
}

impl Default for Preferences {
    fn default() -> Self {
        Self {
            display_timezone: None,
            default_send_account: None,
            message_grouping: MessageGrouping::default(),
            quote_style: QuoteStyle::default(),
            quote_style_per_message: false,
            swipe_left: SwipeAction::default(),
            swipe_right: SwipeAction::default(),
            week_start: WeekStart::default(),
            time_format: TimeFormat::default(),
            appearance: Appearance::default(),
            calendar_visible_hours: DEFAULT_VISIBLE_HOURS,
            calendar_layout: CalendarLayout::default(),
            calendars: BTreeMap::new(),
            default_calendar: None,
            accounts: BTreeMap::new(),
            notify_marks: BTreeMap::new(),
            analytics_consent: None,
            default_mail_app_offer: None,
            analytics_install_id: None,
            analytics_notice_version: None,
            analytics_consented_at: None,
            signature_assignments: BTreeMap::new(),
            invitation_reply_fallback: BTreeMap::new(),
            account_aliases: BTreeMap::new(),
            account_sender_names: BTreeMap::new(),
            mcp_enabled: false,
            mcp_notice_version: None,
            mcp_accounts: BTreeSet::new(),
            mcp_allow_direct_send: false,
            mcp_require_known_recipient: true,
            jurisdiction_mode: Mode::default(),
            ai: Box::default(),
            collapsed_accounts: BTreeSet::new(),
            unified_collapsed: false,
            collapsed_folders: BTreeMap::new(),
        }
    }
}

impl Preferences {
    /// What the user has decided about one calendar: the default (visible, server colour) for a
    /// calendar nobody has touched.
    ///
    /// Keyed on account **and** calendar: a calendar id is unique only within its account.
    #[must_use]
    pub fn calendar(&self, account: &str, calendar: &str) -> CalendarPrefs {
        self.calendars
            .get(account)
            .and_then(|by_calendar| by_calendar.get(calendar))
            .cloned()
            .unwrap_or_default()
    }

    /// Records a decision about one calendar, creating the account's entry if it is the first.
    pub fn set_calendar(&mut self, account: &str, calendar: &str, prefs: CalendarPrefs) {
        self.calendars
            .entry(account.to_owned())
            .or_default()
            .insert(calendar.to_owned(), prefs);
    }

    /// Drops every calendar decision for an account; used when the account is removed, so a later
    /// re-add starts from the defaults (server colour, visible) rather than inheriting a stale
    /// colour override or a hidden calendar. Returns whether anything was stored for it.
    pub fn remove_account_calendars(&mut self, account: &str) -> bool {
        self.calendars.remove(account).is_some()
    }

    /// The calendar horizon **in effect**: [`Self::calendar_visible_hours`], clamped.
    ///
    /// Read through this rather than off the field: the preferences file is plain TOML a user can
    /// hand-edit, and `calendar_visible_hours = 0` would divide the grid by nothing.
    #[must_use]
    pub fn visible_hours(&self) -> u8 {
        clamp_visible_hours(self.calendar_visible_hours)
    }

    /// The extra addresses that also belong to `account`; its aliases. Empty for an account
    /// nobody has configured. See the [`account_aliases`](Preferences::account_aliases) field
    /// for why one identity is not enough.
    #[must_use]
    pub fn aliases_of(&self, account: &str) -> &[String] {
        self.account_aliases.get(account).map_or(&[], Vec::as_slice)
    }

    /// Replaces `account`'s alias list.
    ///
    /// Blank entries are dropped and duplicates removed case-insensitively, so a user who types
    /// a trailing comma or repeats an address does not end up with an entry that can never
    /// match. An account left with no aliases has its entry dropped rather than persisted as an
    /// empty list, so the file does not accumulate a row per account the user merely opened.
    pub fn set_account_aliases(&mut self, account: &str, aliases: Vec<String>) {
        let mut kept: Vec<String> = Vec::new();
        for alias in aliases {
            let trimmed = alias.trim();
            if trimmed.is_empty() {
                continue;
            }
            if !kept.iter().any(|seen| seen.eq_ignore_ascii_case(trimmed)) {
                kept.push(trimmed.to_owned());
            }
        }
        if kept.is_empty() {
            self.account_aliases.remove(account);
        } else {
            self.account_aliases.insert(account.to_owned(), kept);
        }
    }

    /// Drops every alias recorded for an account; used when the account is removed, so a later
    /// re-add starts with the addresses it actually has rather than inheriting a set the user
    /// thought removal had cleared. That set decides which `ATTENDEE` line is "me", so a stale
    /// entry does not merely linger: it can make somebody else's invitation look like ours.
    /// Returns whether anything was stored for it.
    pub fn remove_account_aliases(&mut self, account: &str) -> bool {
        self.account_aliases.remove(account).is_some()
    }

    /// The sync depth **in effect** for `account_id`: its own [`AccountSyncSettings::sync_depth`]
    /// if set, else the product default ([`SyncDepth::default`], currently 3 months).
    #[must_use]
    pub fn effective_sync_depth(&self, account_id: &str) -> SyncDepth {
        self.accounts
            .get(account_id)
            .and_then(|account| account.sync_depth)
            .unwrap_or_default()
    }
}

#[cfg(test)]
mod tests;
#[cfg(test)]
mod tests_accounts;
