//! `From` conversions for the surfaces that describe an account's **standing**, rather than its
//! mail: sync progress (the download bar, the background hint, the paused notice) and
//! connectivity (the offline banner, the per-account prompts, the connection facts a details
//! view shows).
//!
//! A sibling of [`crate::convert`], split off to keep each file under the 500-line limit. No FFI
//! macros live here either, so the generated bindings are unaffected.

use engine_provider::{
    ConnectionInfo as AppConnectionInfo, HttpVersion as AppHttpVersion, TlsVersion as AppTlsVersion,
};
use mailcal_viewmodel::{
    AccountSyncProgress as AppAccountSyncProgress, ConnectivitySnapshot as AppConnectivity,
    SyncProgressSnapshot as AppSyncProgress, ThrottledAccount as AppThrottledAccount,
};

use crate::{
    AccountSyncProgress, ConnectionInfo, ConnectivitySnapshot, HttpVersion, SyncProgressSnapshot,
    ThrottledAccount, TlsVersion,
};

impl From<AppSyncProgress> for SyncProgressSnapshot {
    fn from(snapshot: AppSyncProgress) -> Self {
        Self {
            active: snapshot.active,
            fetched: snapshot.fetched,
            total: snapshot.total,
            accounts: snapshot
                .accounts
                .into_iter()
                .map(AccountSyncProgress::from)
                .collect(),
            throttled: snapshot
                .throttled
                .into_iter()
                .map(ThrottledAccount::from)
                .collect(),
        }
    }
}

impl From<AppThrottledAccount> for ThrottledAccount {
    fn from(account: AppThrottledAccount) -> Self {
        Self {
            account_id: account.account_id,
            resumes_in_minutes: account.resumes_in_minutes,
        }
    }
}

impl From<AppAccountSyncProgress> for AccountSyncProgress {
    fn from(account: AppAccountSyncProgress) -> Self {
        Self {
            account_id: account.account_id,
            folders_done: account.folders_done,
            folders_total: account.folders_total,
            warming_bodies: account.warming_bodies,
            bodies_done: account.bodies_done,
        }
    }
}

impl From<AppConnectivity> for ConnectivitySnapshot {
    fn from(snapshot: AppConnectivity) -> Self {
        Self {
            offline: snapshot.offline,
            unreachable_accounts: snapshot.unreachable_accounts,
            calendar_reauth_accounts: snapshot.calendar_reauth_accounts,
            mail_reauth_accounts: snapshot.mail_reauth_accounts,
            signin_expired_accounts: snapshot.signin_expired_accounts,
        }
    }
}

impl From<AppTlsVersion> for TlsVersion {
    fn from(version: AppTlsVersion) -> Self {
        match version {
            AppTlsVersion::Tls1_2 => Self::Tls1_2,
            AppTlsVersion::Tls1_3 => Self::Tls1_3,
        }
    }
}

impl From<AppHttpVersion> for HttpVersion {
    fn from(version: AppHttpVersion) -> Self {
        match version {
            AppHttpVersion::Http1_1 => Self::Http1_1,
            AppHttpVersion::Http2 => Self::Http2,
        }
    }
}

impl From<AppConnectionInfo> for ConnectionInfo {
    fn from(info: AppConnectionInfo) -> Self {
        Self {
            tls_version: info.tls_version.map(TlsVersion::from),
            http_version: info.http_version.map(HttpVersion::from),
        }
    }
}
