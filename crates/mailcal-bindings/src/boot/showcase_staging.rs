//! The questions and states the in-memory boots stage on demand for the UI suites and the help
//! screenshots, each behind a `MAILCAL_*` variable only a development build reads
//! (`docs/debugging.md`): this module is compiled into no other build.

use std::sync::Arc;

use engine_api::{AccountId, Provider, TimeZoneId};
use mailcal_app::{App, Intent as AppIntent, ai::AiHost};
use time::OffsetDateTime;

use crate::{
    MailcalApp,
    ai_host::BindingsAiHost,
    core_build::build_kind,
    runtime::FfiRuntime,
    showcase_data::AccountSeed,
};

#[path = "showcase_ai.rs"]
mod ai;

/// Whether the launch set `name` to anything.
fn requested(name: &str) -> bool {
    std::env::var_os(name).is_some_and(|value| !value.is_empty())
}

/// What the `MAILCAL_SHOWCASE_*` variables ask of the seeded showcase, whose first account is `id`
/// at `address`.
pub(crate) fn stage_on_request<P: Provider>(
    app: &App<P>,
    runtime: &FfiRuntime,
    id: &str,
    address: &str,
    device: &TimeZoneId,
) {
    if requested("MAILCAL_SHOWCASE_SERVER_CHANGE") {
        pause_for_a_server_change(app, id);
    }
    if requested("MAILCAL_SHOWCASE_REAUTH") {
        ask_for_reconsent(app, id);
    }
    if requested("MAILCAL_SHOWCASE_QUESTIONS") {
        ask_the_questions(app, runtime, id, address, device);
    }
}

/// Whether the launch set `MAILCAL_OFFER_AI`.
pub(crate) fn ai_requested() -> bool {
    requested("MAILCAL_OFFER_AI")
}

/// The showcase's first account as seeded, with the sent mail a learning flow reads when
/// `offer_ai`, so its review is `Ready`. Without it the seed is returned untouched: the store
/// captures come from this boot.
pub(crate) fn primary_seed(seed: AccountSeed, now: OffsetDateTime, offer_ai: bool) -> AccountSeed {
    if offer_ai {
        ai::with_sent_mail(seed, now)
    } else {
        seed
    }
}

/// `MAILCAL_OFFER_AI`: the writing assistant offered on an in-memory boot, with two learned
/// styles and an own endpoint staged for `account` (its id and address) when the boot has one.
pub(crate) fn offer_ai_on_request(mailcal: &MailcalApp, account: Option<(&str, &str)>) {
    if ai_requested() {
        offer_ai(mailcal);
        if let Some((id, address)) = account {
            ai::stage(mailcal.app.as_ref(), id, address);
        }
    }
}

/// Installs the assistant's host, as a stored boot does, so the own endpoint's form and Settings
/// → Writing style can be driven (`docs/ai.md` rule 5). Without it an in-memory boot offers the
/// feature to nobody, which keeps it out of the store captures. The host reaches no network,
/// since these boots reach nobody: a draft asked of the staged endpoint's model gets a canned
/// reply and every other request answers as no answer (`ai::transport`). The keystore refuses
/// a key, since these boots keep none.
pub(crate) fn offer_ai(mailcal: &MailcalApp) {
    install_offline_ai(mailcal, build_kind());
}

/// [`offer_ai`]'s host as a production build installs it, whose offer follows the entitlement,
/// for the tests that grant and withdraw it.
#[cfg(test)]
pub(crate) fn offer_ai_in_production(mailcal: &MailcalApp) {
    install_offline_ai(mailcal, mailcal_app::ai::BuildKind::Production);
}

fn install_offline_ai(mailcal: &MailcalApp, build: mailcal_app::ai::BuildKind) {
    let host = BindingsAiHost::offline(
        mailcal.runtime.handle().clone(),
        Arc::clone(&mailcal.credential_store),
        ai::transport,
    );
    mailcal
        .app
        .install_ai_host(Arc::new(host) as Arc<dyn AiHost>, build);
}

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

#[cfg(test)]
#[path = "showcase_staging_tests.rs"]
mod tests;
