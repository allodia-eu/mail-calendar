//! The two optional **calendar** bindings a dial reaches for, shared by every path that opens an
//! account: the boot dial, the reconnect, and the OAuth sign-in completions.
//!
//! It used to hold `connect_account` / `connect_jmap_account` too: the third of four independent
//! implementations of "open an account of family X", the one `add_account` and the JMAP
//! re-authentication used. Both now go through
//! [`AccountDial`](crate::account_registry::AccountDial) like everything else, so what is left here
//! is the part that genuinely is per-family and genuinely is optional: a calendar, whose failure
//! the dial weighs against what else the account is used for
//! (`account_registry::dial_parts`).

use std::sync::Arc;

use engine_provider::Provider;
use mailcal_account::{GraphTokenSource, JmapAccountConfig};

use crate::account_registry::ConnectFailure;

/// Binds the JMAP account's calendar provider when its session advertises calendars. A server
/// with no calendar support yields no provider rather than one that fails every pass.
pub(crate) async fn connect_jmap_calendars(
    config: &JmapAccountConfig,
    tokens: Option<&Arc<GraphTokenSource>>,
    providers: &[Box<dyn Provider>],
) -> Result<Vec<Box<dyn Provider>>, ConnectFailure> {
    if !providers
        .first()
        .is_some_and(|provider| provider.connection_info().capabilities.calendars())
    {
        return Ok(Vec::new());
    }
    mailcal_account::connect_jmap_calendar_providers(config, tokens)
        .await
        .map_err(|err| {
            log::warn!("jmap: calendar connect failed: {err}");
            ConnectFailure::from(err)
        })
}

/// Binds a Google account's calendar provider (its primary calendar). Unlike the Graph parallel
/// there is no re-consent case to report (Google requests the calendar scope at sign-in).
pub(crate) async fn connect_google_calendars(
    id: &engine_api::AccountId,
    tokens: Arc<GraphTokenSource>,
) -> Result<Vec<Box<dyn Provider>>, ConnectFailure> {
    mailcal_account::connect_google_calendar_providers(id, tokens)
        .await
        .map_err(|err| {
            log::warn!("google: calendar connect failed: {err}");
            ConnectFailure::from(err)
        })
}
