//! The questions and states the showcase stages on demand for the UI suites and the help
//! screenshots, each behind a `MAILCAL_SHOWCASE_*` variable a development build reads
//! (`docs/debugging.md`).

use engine_api::{AccountId, Provider, TimeZoneId};
use mailcal_app::{App, Intent as AppIntent};

use crate::runtime::FfiRuntime;

/// The account `id`'s calendar waiting on a server found on another site, for
/// `MAILCAL_SHOWCASE_SERVER_CHANGE`, which puts the question in front of the UI suites
/// (`docs/account-autodetect.md` rule 14). Allowing it is refused, because a showcase account has
/// no servers to change.
pub(crate) fn pause_for_a_server_change<P>(app: &App<P>, id: &str) {
    let id = AccountId::try_from(id).expect("valid account id");
    app.note_server_change(
        &id,
        mailcal_app::ServerChange {
            previous_site: "example.com".to_owned(),
            proposed_site: "example.net".to_owned(),
            calendar_url: Some("https://dav.example.net/".to_owned()),
            contacts_url: None,
        },
    );
}

/// The account `id` asked to sign in again for mail and for its calendar, for
/// `MAILCAL_SHOWCASE_REAUTH`: the two notices only a narrowed Microsoft grant raises otherwise
/// (`docs/provider-oauth.md` rule 11), staged for the UI suites and the help screenshots. Signing
/// in again opens the account's page, as for any account without a browser sign-in.
pub(crate) fn ask_for_reconsent<P: Provider>(app: &App<P>, id: &str) {
    let id = AccountId::try_from(id).expect("valid account id");
    app.note_calendar_reauth_required(&id);
    app.note_mail_reauth_required(&id);
}

/// `MAILCAL_SHOWCASE_QUESTIONS`: every question the core can ask as a notice (`docs/notices.md`
/// rule 8), staged for the UI suites and the help screenshots. The calendar is moved to a zone
/// other than the device's, so the device's own report at launch keeps the time zone question
/// standing rather than settling it.
pub(crate) fn ask_the_questions<P: Provider>(
    app: &App<P>,
    runtime: &FfiRuntime,
    id: &str,
    address: &str,
    device: &TimeZoneId,
) {
    let id = AccountId::try_from(id).expect("valid account id");
    app.stage_showcase_questions(&id, address);
    let elsewhere = if device.as_str() == "Asia/Tokyo" {
        "Europe/Lisbon"
    } else {
        "Asia/Tokyo"
    };
    runtime.block_on(|| async {
        app.dispatch(AppIntent::SetTimeZone(elsewhere.to_owned()))
            .await;
        app.dispatch(AppIntent::ReportDeviceTimeZone(device.as_str().to_owned()))
            .await;
    });
}
