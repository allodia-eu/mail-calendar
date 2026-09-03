//! Conversions for the **settings surface**: the display timezone, message-list grouping, default
//! reply/forward quote style, per-direction swipe actions, and the per-account sync-behaviour rows.
//! Split out of [`crate::convert`] (which keeps the mailbox/calendar/reading snapshot rows) so each
//! file stays under the 500-line limit; both map the product-core's `mailcal_viewmodel` types to
//! the UniFFI-exported twins the generated Swift/Kotlin/C# see.

use mailcal_app::SignatureBody as AppSignatureBody;
use mailcal_viewmodel::{
    AccountSignatureRow as AppAccountSignatureRow, AccountSyncRow as AppAccountSyncRow,
    DefaultMailAppOutcome as AppDefaultMailAppOutcome,
    DefaultMailAppSupport as AppDefaultMailAppSupport, QuoteSettings as AppQuoteSettings,
    QuoteStyleKind as AppQuoteStyleKind, SignatureRow as AppSignatureRow,
    SignatureSlotKind as AppSignatureSlotKind, SignaturesSnapshot as AppSignatures,
    SwipeActionKind as AppSwipeActionKind, SwipeDirection as AppSwipeDirection,
    SwipeSettings as AppSwipeSettings, SyncFolderRow as AppSyncFolderRow,
    SyncSettingsSnapshot as AppSyncSettings, SyncStrategyKind as AppSyncStrategyKind,
    TimeZoneSnapshot as AppTimeZoneSnapshot, ViewMode as AppViewMode,
};

use crate::{
    AccountSignatureRow, AccountSyncRow, DefaultMailAppOutcome, DefaultMailAppSupport, FolderRole,
    QuoteSettings, QuoteStyleKind, SignatureBody, SignatureRow, SignatureSlotKind,
    SignaturesSnapshot, SwipeActionKind, SwipeDirection, SwipeSettings, SyncFolderRow,
    SyncSettingsSnapshot, SyncStrategyKind, TimeZoneSnapshot, ViewMode,
};

impl From<AppTimeZoneSnapshot> for TimeZoneSnapshot {
    fn from(snapshot: AppTimeZoneSnapshot) -> Self {
        Self {
            active: snapshot.active,
            pending_device: snapshot.pending_device,
        }
    }
}

impl From<AppSyncStrategyKind> for SyncStrategyKind {
    fn from(kind: AppSyncStrategyKind) -> Self {
        match kind {
            AppSyncStrategyKind::Push => Self::Push,
            AppSyncStrategyKind::Poll => Self::Poll,
        }
    }
}

impl From<SyncStrategyKind> for AppSyncStrategyKind {
    fn from(kind: SyncStrategyKind) -> Self {
        match kind {
            SyncStrategyKind::Push => Self::Push,
            SyncStrategyKind::Poll => Self::Poll,
        }
    }
}

impl From<AppQuoteStyleKind> for QuoteStyleKind {
    fn from(kind: AppQuoteStyleKind) -> Self {
        match kind {
            AppQuoteStyleKind::Indented => Self::Indented,
            AppQuoteStyleKind::LineAndHeader => Self::LineAndHeader,
        }
    }
}

impl From<QuoteStyleKind> for AppQuoteStyleKind {
    fn from(kind: QuoteStyleKind) -> Self {
        match kind {
            QuoteStyleKind::Indented => Self::Indented,
            QuoteStyleKind::LineAndHeader => Self::LineAndHeader,
        }
    }
}

impl From<AppQuoteSettings> for QuoteSettings {
    fn from(settings: AppQuoteSettings) -> Self {
        Self {
            style: settings.style.into(),
            per_message: settings.per_message,
        }
    }
}

impl From<AppSignatureRow> for SignatureRow {
    fn from(row: AppSignatureRow) -> Self {
        Self {
            id: row.id,
            name: row.name,
        }
    }
}

impl From<SignatureSlotKind> for AppSignatureSlotKind {
    fn from(slot: SignatureSlotKind) -> Self {
        match slot {
            SignatureSlotKind::NewMessage => Self::NewMessage,
            SignatureSlotKind::ReplyForward => Self::ReplyForward,
        }
    }
}

impl From<AppAccountSignatureRow> for AccountSignatureRow {
    fn from(row: AppAccountSignatureRow) -> Self {
        Self {
            account_id: row.account_id,
            email: row.email,
            new_message: row.new_message,
            reply_forward: row.reply_forward,
        }
    }
}

impl From<AppSignatures> for SignaturesSnapshot {
    fn from(snapshot: AppSignatures) -> Self {
        Self {
            signatures: snapshot
                .signatures
                .into_iter()
                .map(SignatureRow::from)
                .collect(),
            accounts: snapshot
                .accounts
                .into_iter()
                .map(AccountSignatureRow::from)
                .collect(),
        }
    }
}

impl From<AppSignatureBody> for SignatureBody {
    fn from(body: AppSignatureBody) -> Self {
        Self {
            id: body.id,
            body_html: body.body_html,
            body_plain: body.body_plain,
        }
    }
}

impl From<AppSwipeActionKind> for SwipeActionKind {
    fn from(kind: AppSwipeActionKind) -> Self {
        match kind {
            AppSwipeActionKind::Delete => Self::Delete,
            AppSwipeActionKind::Archive => Self::Archive,
            AppSwipeActionKind::Star => Self::Star,
        }
    }
}

impl From<SwipeActionKind> for AppSwipeActionKind {
    fn from(kind: SwipeActionKind) -> Self {
        match kind {
            SwipeActionKind::Delete => Self::Delete,
            SwipeActionKind::Archive => Self::Archive,
            SwipeActionKind::Star => Self::Star,
        }
    }
}

impl From<SwipeDirection> for AppSwipeDirection {
    fn from(direction: SwipeDirection) -> Self {
        match direction {
            SwipeDirection::Left => Self::Left,
            SwipeDirection::Right => Self::Right,
        }
    }
}

impl From<AppSwipeSettings> for SwipeSettings {
    fn from(settings: AppSwipeSettings) -> Self {
        Self {
            left: settings.left.into(),
            right: settings.right.into(),
        }
    }
}

impl From<AppSyncFolderRow> for SyncFolderRow {
    fn from(row: AppSyncFolderRow) -> Self {
        Self {
            key: row.key,
            name: row.name,
            role: row.role.map(FolderRole::from),
            subscribed: row.subscribed,
        }
    }
}

impl From<AppAccountSyncRow> for AccountSyncRow {
    fn from(row: AppAccountSyncRow) -> Self {
        Self {
            account_id: row.account_id,
            email: row.email,
            idle_supported: row.idle_supported,
            strategy: row.strategy.into(),
            poll_interval_mins: row.poll_interval_mins,
            sync_depth_months: row.sync_depth_months,
            message_size_limit_mb: row.message_size_limit_mb,
            at_push_limit: row.at_push_limit,
            folders: row.folders.into_iter().map(SyncFolderRow::from).collect(),
        }
    }
}

impl From<AppSyncSettings> for SyncSettingsSnapshot {
    fn from(snapshot: AppSyncSettings) -> Self {
        Self {
            accounts: snapshot
                .accounts
                .into_iter()
                .map(AccountSyncRow::from)
                .collect(),
            max_push_folders: snapshot.max_push_folders,
            poll_intervals: snapshot.poll_intervals,
            sync_depths: snapshot.sync_depths,
            message_size_limits_mb: snapshot.message_size_limits_mb,
        }
    }
}

impl From<AppViewMode> for ViewMode {
    fn from(mode: AppViewMode) -> Self {
        match mode {
            AppViewMode::Flat => Self::Flat,
            AppViewMode::Threaded => Self::Threaded,
        }
    }
}

impl From<ViewMode> for AppViewMode {
    fn from(mode: ViewMode) -> Self {
        match mode {
            ViewMode::Flat => Self::Flat,
            ViewMode::Threaded => Self::Threaded,
        }
    }
}

impl From<DefaultMailAppSupport> for AppDefaultMailAppSupport {
    fn from(support: DefaultMailAppSupport) -> Self {
        match support {
            DefaultMailAppSupport::SetDirectly => Self::SetDirectly,
            DefaultMailAppSupport::OpenSettings => Self::OpenSettings,
            DefaultMailAppSupport::Unsupported => Self::Unsupported,
        }
    }
}

impl From<DefaultMailAppOutcome> for AppDefaultMailAppOutcome {
    fn from(outcome: DefaultMailAppOutcome) -> Self {
        match outcome {
            DefaultMailAppOutcome::Accepted => Self::Accepted,
            DefaultMailAppOutcome::Declined => Self::Declined,
        }
    }
}
