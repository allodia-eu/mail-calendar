//! The app the invitation suites run: one account over [`InvitationFake`], its mail half and
//! its calendar half sharing their recording state.

use std::sync::{Arc, Mutex};

use engine_api::{AccountId, EmailAddress, Engine, TimeZoneId};

use super::{InvitationFake, RecordingObserver};
use crate::{Account, App, Surface, Telemetry, TimeZoneInit};

/// An app over one account whose identity is `me@`; **not** the address the invitation is
/// sent to. The alias reaches the matcher through the message's own `To:` header, which is
/// what makes an aliased invitation work with no configuration at all.
pub(crate) fn invitation_app(
    provider: InvitationFake,
    surfaces: &Arc<Mutex<Vec<Surface>>>,
) -> App<InvitationFake> {
    invitation_app_with_prefs(provider, surfaces, None)
}

/// The same, with a preferences file: so a test can set the account's standing answer to the
/// reply-fallback question, and prove a remembered choice is honoured without a prompt.
///
/// Without a path the app has nowhere to read a choice from and every account answers `Ask`,
/// which is right for the default but makes `Always`/`Never` untestable.
pub(crate) fn invitation_app_with_prefs(
    provider: InvitationFake,
    surfaces: &Arc<Mutex<Vec<Surface>>>,
    prefs_path: Option<std::path::PathBuf>,
) -> App<InvitationFake> {
    let account = Account {
        id: AccountId::try_from("acct-a").unwrap(),
        // The mail half and the calendar half are two `Provider`s over **shared** recording
        // state, because the answer crosses them: the calendar stores it and the mail sends
        // it, and a test that could not see both would only ever prove half an answer.
        providers: vec![InvitationFake {
            caps: provider.caps,
            rsvps: Arc::clone(&provider.rsvps),
            controls: provider.controls,
            materialized: provider.materialized,
            stored_sequence: provider.stored_sequence,
            source: provider.source.clone(),
            puts: Arc::clone(&provider.puts),
            sends: Arc::clone(&provider.sends),
            created: Arc::clone(&provider.created),
            has_calendar: provider.has_calendar,
            reply_delivery: provider.reply_delivery.clone(),
            calendar_users: provider.calendar_users.clone(),
            writes: provider.writes,
            days_ahead: provider.days_ahead,
        }]
        .into(),
        calendar_provider: provider.has_calendar.then_some(provider),
        meetings: None,
        contact_providers: Vec::new(),
        identity: EmailAddress::new("me@test.local"),
        dialled: true,
        uses_mail: true,
    };
    App::new(
        Engine::open_in_memory().unwrap(),
        vec![account],
        TimeZoneInit {
            device_zone: TimeZoneId::utc(),
            prefs_path,
        },
        None,
        Arc::new(RecordingObserver {
            surfaces: Arc::clone(surfaces),
        }),
        Telemetry::off(),
    )
}
