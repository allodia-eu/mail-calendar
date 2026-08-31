//! The two optional **calendar** bindings a dial reaches for, shared by every path that opens an
//! account: the boot dial, the reconnect, and the OAuth sign-in completions.
//!
//! It used to hold `connect_account` / `connect_jmap_account` too: the third of four independent
//! implementations of "open an account of family X", the one `add_account` and the JMAP
//! re-authentication used. Both now go through
//! [`AccountDial`](crate::account_registry::AccountDial) like everything else, so what is left here
//! is the part that genuinely is per-family and genuinely is optional: a calendar whose failure
//! must never take the mailbox down with it.

use std::sync::Arc;

use engine_provider::Provider;
use mailcal_account::{GraphTokenSource, JmapAccountConfig};

/// Binds the JMAP account's calendar provider when its session advertises calendars. A
/// calendar-connect failure is non-fatal: mail comes up with an empty agenda rather than failing
/// the whole account. A server with no calendar support yields no provider rather than one that
/// fails every pass.
pub(crate) async fn connect_jmap_calendars(
    config: &JmapAccountConfig,
    tokens: Option<&Arc<GraphTokenSource>>,
    providers: &[Box<dyn Provider>],
) -> Vec<Box<dyn Provider>> {
    if providers
        .first()
        .is_some_and(|provider| provider.connection_info().capabilities.calendars())
    {
        match mailcal_account::connect_jmap_calendar_providers(config, tokens).await {
            Ok(providers) => providers,
            Err(err) => {
                log::warn!("jmap: calendar connect failed, mail only: {err}");
                Vec::new()
            }
        }
    } else {
        Vec::new()
    }
}

/// Binds a Google account's calendar provider (its primary calendar). A calendar-connect failure is
/// non-fatal: mail comes up with an empty agenda. Unlike the Graph parallel there is no re-consent
/// case to report (Google requests the calendar scope at sign-in) so this returns just the
/// (possibly empty) providers.
pub(crate) async fn connect_google_calendars(
    id: &engine_api::AccountId,
    tokens: Arc<GraphTokenSource>,
) -> Vec<Box<dyn Provider>> {
    match mailcal_account::connect_google_calendar_providers(id, tokens).await {
        Ok(providers) => providers,
        Err(err) => {
            log::warn!("google: calendar connect failed, mail only: {err}");
            Vec::new()
        }
    }
}
