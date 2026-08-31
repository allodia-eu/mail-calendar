//! What we report: the closed [`Event`] enum and the closed label enums its fields are drawn
//! from.
//!
//! The privacy rule this module enforces (`docs/analytics.md`) is **structural, not a
//! convention**: no call site can pass a free-form string into an event. Every property value is
//! the label of a closed enum; [`Protocol`], [`Feature`], [`DurationBucket`], or one of the
//! persisted settings enums. So a subject line, an address, a folder name, a search query, or a
//! hostname cannot reach the payload *even by accident*: there is no variant that would carry
//! one.
//!
//! That is also why setup and sync failures carry **no error class**. The only place the useful
//! distinction lives (a rejected password vs a TLS failure vs a dead DNS lookup) is inside an
//! error *string* that routinely embeds the user's host and username, and no amount of
//! keyword-matching on that string is a safe source of truth. We count the failures and log the
//! gap rather than invent a class we cannot honestly derive.
//!
//! Widening what we send therefore means adding a variant here, which forces a matching key into
//! [`PROPERTY_KEYS`](super::payload::PROPERTY_KEYS) **and** into the relay's ingest whitelist;
//! two deliberate edits in two repositories. That is the intended cost.

use std::collections::BTreeMap;

use mailcal_account::{MessageGrouping, QuoteStyle, SwipeAction};

/// An account's provider family. Mirrors the binding layer's `ConnectedAccount::account_type`,
/// which already exists precisely because it names only the protocol; never an endpoint, a host,
/// or an identity.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Protocol {
    /// IMAP/SMTP (+ CalDAV).
    Imap,
    /// JMAP (RFC 8620/8621).
    Jmap,
    /// Microsoft Graph.
    Graph,
    /// Google (Gmail + Google Calendar) native APIs.
    Google,
}

impl Protocol {
    /// The wire label.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Imap => "imap",
            Self::Jmap => "jmap",
            Self::Graph => "graph",
            Self::Google => "google",
        }
    }
}

/// A product surface whose adoption we count. A closed enum, so "which features are used" can
/// never turn into "what did the user search for".
///
/// Every variant is something the **core** can observe from an inbound `Intent`. Two surfaces we
/// would like to count are deliberately absent because only the client knows they happened: a
/// swipe arrives at the core as a plain Delete/Archive/Flag, and opening a received attachment
/// never reaches the core at all. Rather than widen the FFI so clients can report them, we take
/// the proxy we already have (the settings snapshot says which swipe actions were *configured*)
/// and log the gap; `docs/analytics.md` → Known gaps.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Feature {
    /// The search field was used. The **query is never sent**; we look only at whether it is
    /// blank.
    Search,
    /// The calendar was viewed.
    Calendar,
    /// A calendar event was created.
    EventCreate,
    /// A new message was composed.
    ComposerNew,
    /// A reply was composed.
    ComposerReply,
    /// A forward was composed.
    ComposerForward,
    /// A file was attached in the composer. We look only at whether the attachment list is
    /// non-empty; never at a filename, a size, or a type.
    AttachmentAdd,
    /// A background sync pass ran.
    BackgroundSync,
}

impl Feature {
    /// The wire label.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Search => "search",
            Self::Calendar => "calendar",
            Self::EventCreate => "event_create",
            Self::ComposerNew => "composer_new",
            Self::ComposerReply => "composer_reply",
            Self::ComposerForward => "composer_forward",
            Self::AttachmentAdd => "attachment_add",
            Self::BackgroundSync => "background_sync",
        }
    }
}

/// How long an operation took, as a bucket. A raw millisecond count is a surprisingly good
/// fingerprint across many events; a bucket answers "is sync slow?" just as well.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DurationBucket {
    /// Under one second.
    Fast,
    /// One to five seconds.
    Normal,
    /// Five to thirty seconds.
    Slow,
    /// Over thirty seconds.
    VerySlow,
}

impl DurationBucket {
    /// Buckets a raw duration. The **only** way to name a bucket; callers cannot pick one, so a
    /// raw timing can never be smuggled through as a label.
    #[must_use]
    pub const fn of_millis(millis: u64) -> Self {
        match millis {
            0..1_000 => Self::Fast,
            1_000..5_000 => Self::Normal,
            5_000..30_000 => Self::Slow,
            _ => Self::VerySlow,
        }
    }

    /// The wire label.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Fast => "lt_1s",
            Self::Normal => "1_5s",
            Self::Slow => "5_30s",
            Self::VerySlow => "gt_30s",
        }
    }
}

/// Everything the core can report. Adding a variant is the *only* way to widen what we send.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event {
    /// The app was launched. The retention/cohort signal: the one thing that genuinely needs the
    /// stable install id, and therefore the reason the consent gate exists at all.
    AppOpened,
    /// Account setup was started for a protocol.
    SetupStarted {
        /// Which protocol the user picked.
        protocol: Protocol,
    },
    /// Account setup succeeded.
    SetupCompleted {
        /// Which protocol connected.
        protocol: Protocol,
    },
    /// Account setup failed; paired with [`Self::SetupStarted`] this is the funnel: *how many*
    /// people never get an account connected, and on which protocol.
    ///
    /// Deliberately **unclassified**, for the same reason as [`Self::SyncFailed`]. `MailcalError`
    /// distinguishes only config / connect / engine; the distinction that would actually be
    /// useful (a rejected password vs a TLS failure vs a dead DNS lookup) lives inside a
    /// connect error's *string*, and that string routinely embeds the user's host and username.
    /// Keyword-matching it to derive a class would be brittle and would put us one engine message
    /// change away from a leak. So we count, and log the gap (`docs/analytics.md` → Known gaps);
    /// re-exporting the engine's `FailureClass` through `engine-api` is what unlocks the *why*.
    SetupFailed {
        /// Which protocol was attempted.
        protocol: Protocol,
    },
    /// A product surface was used.
    FeatureUsed {
        /// Which surface.
        feature: Feature,
    },
    /// The user's current settings, so we learn what the defaults *should* be.
    SettingsSnapshot {
        /// Flat or threaded message list.
        grouping: MessageGrouping,
        /// Gmail- or Outlook-style reply quoting.
        quote_style: QuoteStyle,
        /// What a leftward swipe does.
        swipe_left: SwipeAction,
        /// What a rightward swipe does.
        swipe_right: SwipeAction,
    },
    /// A sync pass reached the server.
    SyncCompleted {
        /// The account's protocol.
        protocol: Protocol,
        /// How long it took, bucketed.
        duration: DurationBucket,
    },
    /// A sync pass did not reach the server.
    ///
    /// Deliberately **unclassified**. The engine classifies provider failures with a
    /// `FailureClass` (auth / rate-limited / permanent / …), but that type lives in `engine-core`
    /// and the product core consumes the engine only through the `engine-api` facade (AGENTS.md).
    /// `ApiError` (all the facade exposes) cannot tell a revoked credential from a dead DNS
    /// lookup. Faking a class out of an error *string* would risk putting the user's host or
    /// username on the wire, which is precisely what this module exists to prevent. So we count
    /// failures per protocol and log the gap (`docs/analytics.md` → Known gaps); re-exporting
    /// `FailureClass` from `engine-api` is the follow-up that unlocks a real class.
    SyncFailed {
        /// The account's protocol.
        protocol: Protocol,
    },
}

impl Event {
    /// The event's wire name.
    #[must_use]
    pub const fn name(&self) -> &'static str {
        match self {
            Self::AppOpened => "app_opened",
            Self::SetupStarted { .. } => "setup_started",
            Self::SetupCompleted { .. } => "setup_completed",
            Self::SetupFailed { .. } => "setup_failed",
            Self::FeatureUsed { .. } => "feature_used",
            Self::SettingsSnapshot { .. } => "settings_snapshot",
            Self::SyncCompleted { .. } => "sync_completed",
            Self::SyncFailed { .. } => "sync_failed",
        }
    }

    /// The event's own properties. Every value is an enum label; see the module docs.
    pub(super) fn properties(&self) -> BTreeMap<&'static str, String> {
        let mut props = BTreeMap::new();
        let mut put = |key: &'static str, value: &'static str| {
            props.insert(key, value.to_owned());
        };
        match *self {
            Self::AppOpened => {}
            Self::SetupStarted { protocol }
            | Self::SetupCompleted { protocol }
            | Self::SetupFailed { protocol }
            | Self::SyncFailed { protocol } => put("protocol", protocol.as_str()),
            Self::FeatureUsed { feature } => put("feature", feature.as_str()),
            Self::SettingsSnapshot {
                grouping,
                quote_style,
                swipe_left,
                swipe_right,
            } => {
                put("grouping", grouping_label(grouping));
                put("quote_style", quote_label(quote_style));
                put("swipe_left", swipe_label(swipe_left));
                put("swipe_right", swipe_label(swipe_right));
            }
            Self::SyncCompleted { protocol, duration } => {
                put("protocol", protocol.as_str());
                put("duration", duration.as_str());
            }
        }
        props
    }
}

/// The wire label for a persisted message grouping.
const fn grouping_label(grouping: MessageGrouping) -> &'static str {
    match grouping {
        MessageGrouping::Flat => "flat",
        MessageGrouping::Threaded => "threaded",
    }
}

/// The wire label for a persisted quote style. Named for what the style *is* (not for the mail
/// client it resembles), matching the enum after the rename: the legacy `gmail` / `outlook`
/// tokens are read-only serde aliases and are not what we report.
const fn quote_label(style: QuoteStyle) -> &'static str {
    match style {
        QuoteStyle::Indented => "indented",
        QuoteStyle::LineAndHeader => "line_and_header",
    }
}

/// The wire label for a persisted swipe action.
const fn swipe_label(action: SwipeAction) -> &'static str {
    match action {
        SwipeAction::Delete => "delete",
        SwipeAction::Archive => "archive",
        SwipeAction::Star => "star",
    }
}
