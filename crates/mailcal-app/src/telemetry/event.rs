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

use mailcal_account::{MessageGrouping, Preferences, QuoteStyle, SwipeAction};

/// Declares a closed label enum: its variants, each variant's wire label, and `ALL`. `ALL` is
/// written out from the same list as the variants, so a variant cannot exist without being in it,
/// which is what lets the payload preview show every label by iterating `ALL`.
macro_rules! labels {
    (
        $(#[$meta:meta])*
        $vis:vis enum $name:ident {
            $( $(#[$vmeta:meta])* $variant:ident => $label:literal, )+
        }
    ) => {
        $(#[$meta])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        $vis enum $name {
            $( $(#[$vmeta])* $variant, )+
        }

        impl $name {
            /// Every variant, in declaration order.
            $vis const ALL: &'static [Self] = &[$(Self::$variant),+];

            /// The wire label.
            #[must_use]
            $vis const fn as_str(self) -> &'static str {
                match self {
                    $(Self::$variant => $label,)+
                }
            }
        }
    };
}

labels! {
    /// An account's provider family. Mirrors the binding layer's `ConnectedAccount::account_type`,
    /// which already exists precisely because it names only the protocol; never an endpoint, a
    /// host, or an identity.
    pub enum Protocol {
        /// IMAP/SMTP (+ CalDAV).
        Imap => "imap",
        /// JMAP (RFC 8620/8621).
        Jmap => "jmap",
        /// Microsoft Graph.
        Graph => "graph",
        /// Google (Gmail + Google Calendar) native APIs.
        Google => "google",
    }
}

labels! {
    /// A product surface whose adoption we count. A closed enum, so "which features are used" can
    /// never turn into "what did the user search for".
    ///
    /// Every variant is something the **core** can observe from an inbound `Intent`. Two surfaces
    /// we would like to count are deliberately absent because only the client knows they
    /// happened: a swipe arrives at the core as a plain Delete/Archive/Flag, and opening a
    /// received attachment never reaches the core at all. Rather than widen the FFI so clients
    /// can report them, we take the proxy we already have (the settings snapshot says which swipe
    /// actions were *configured*) and log the gap; `docs/analytics.md` → Known gaps.
    pub enum Feature {
        /// The search field was used. The **query is never sent**; we look only at whether it is
        /// blank.
        Search => "search",
        /// The calendar was viewed.
        Calendar => "calendar",
        /// A calendar event was created.
        EventCreate => "event_create",
        /// A new message was composed.
        ComposerNew => "composer_new",
        /// A reply was composed.
        ComposerReply => "composer_reply",
        /// A forward was composed.
        ComposerForward => "composer_forward",
        /// A file was attached in the composer. We look only at whether the attachment list is
        /// non-empty; never at a filename, a size, or a type.
        AttachmentAdd => "attachment_add",
        /// A background sync pass ran.
        BackgroundSync => "background_sync",
    }
}

labels! {
    /// How long an operation took, as a bucket. A raw millisecond count is a surprisingly good
    /// fingerprint across many events; a bucket answers "is sync slow?" just as well.
    pub enum DurationBucket {
        /// Under one second.
        Fast => "lt_1s",
        /// One to five seconds.
        Normal => "1_5s",
        /// Five to thirty seconds.
        Slow => "5_30s",
        /// Over thirty seconds.
        VerySlow => "gt_30s",
    }
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
}

labels! {
    /// The kind of an [`Event`], whose label is the event's wire name.
    pub(crate) enum EventKind {
        /// [`Event::AppOpened`].
        AppOpened => "app_opened",
        /// [`Event::SetupStarted`].
        SetupStarted => "setup_started",
        /// [`Event::SetupCompleted`].
        SetupCompleted => "setup_completed",
        /// [`Event::SetupFailed`].
        SetupFailed => "setup_failed",
        /// [`Event::FeatureUsed`].
        FeatureUsed => "feature_used",
        /// [`Event::SettingsSnapshot`].
        SettingsSnapshot => "settings_snapshot",
        /// [`Event::SyncCompleted`].
        SyncCompleted => "sync_completed",
        /// [`Event::SyncFailed`].
        SyncFailed => "sync_failed",
    }
}

impl EventKind {
    /// The events of this kind the payload preview shows. An event whose field names what was
    /// used (a protocol, a feature) appears once per label, so every label the app can send is on
    /// the screen; a bucket shows one value, and the settings are this device's own (`settings`).
    pub(crate) fn previewed(self, settings: &Event) -> Vec<Event> {
        let per_protocol =
            |event: fn(Protocol) -> Event| Protocol::ALL.iter().map(move |&each| event(each));
        match self {
            Self::AppOpened => vec![Event::AppOpened],
            Self::SetupStarted => {
                per_protocol(|protocol| Event::SetupStarted { protocol }).collect()
            }
            Self::SetupCompleted => {
                per_protocol(|protocol| Event::SetupCompleted { protocol }).collect()
            }
            Self::SetupFailed => per_protocol(|protocol| Event::SetupFailed { protocol }).collect(),
            Self::FeatureUsed => Feature::ALL
                .iter()
                .map(|&feature| Event::FeatureUsed { feature })
                .collect(),
            Self::SettingsSnapshot => vec![settings.clone()],
            Self::SyncCompleted => per_protocol(|protocol| Event::SyncCompleted {
                protocol,
                duration: DurationBucket::Fast,
            })
            .collect(),
            Self::SyncFailed => per_protocol(|protocol| Event::SyncFailed { protocol }).collect(),
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
    /// The settings snapshot the stored preferences make: the one a launch reports and the
    /// payload preview shows.
    pub(crate) fn settings_snapshot(prefs: &Preferences) -> Self {
        Self::SettingsSnapshot {
            grouping: prefs.message_grouping,
            quote_style: prefs.quote_style,
            swipe_left: prefs.swipe_left,
            swipe_right: prefs.swipe_right,
        }
    }

    /// The event's kind.
    pub(crate) const fn kind(&self) -> EventKind {
        match self {
            Self::AppOpened => EventKind::AppOpened,
            Self::SetupStarted { .. } => EventKind::SetupStarted,
            Self::SetupCompleted { .. } => EventKind::SetupCompleted,
            Self::SetupFailed { .. } => EventKind::SetupFailed,
            Self::FeatureUsed { .. } => EventKind::FeatureUsed,
            Self::SettingsSnapshot { .. } => EventKind::SettingsSnapshot,
            Self::SyncCompleted { .. } => EventKind::SyncCompleted,
            Self::SyncFailed { .. } => EventKind::SyncFailed,
        }
    }

    /// The event's wire name.
    #[must_use]
    pub const fn name(&self) -> &'static str {
        self.kind().as_str()
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
