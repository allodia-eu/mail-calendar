//! Running a dial: opening each capability the account is used for, and assembling the account.
//!
//! Split from `dial.rs`, which holds what a dial *is*; this is what one *does*. Each family opens
//! its capabilities as a [`Part`] each, concurrently where they do not depend on each other, and
//! [`assemble`] decides which failure is the account's.

use std::sync::Arc;

use engine_api::{AccountId, EmailAddress, TimeZoneId};
use mailcal_account::Capability;
use mailcal_app::Account;

use super::{
    dial::{AccountDial, ConnectFailure, DialOutcome},
    dial_parts::{Assembled, Part, assemble},
};
use crate::boot;

/// Opens a capability when the account is used for it, and answers [`Part::Off`] otherwise
/// without running anything.
async fn part<T, F>(on: bool, open: F) -> Part<T>
where
    F: Future<Output = Result<Vec<T>, ConnectFailure>>,
{
    if on { open.await.into() } else { Part::Off }
}

impl AccountDial {
    /// Opens the account's providers for each capability it is used for.
    ///
    /// `display_zone` is the `Prefer: outlook.timezone` a Microsoft account binds its Graph
    /// calendar with. Which failure is the account's is [`assemble`]'s: mail's, when the account
    /// is used for mail; otherwise the first, when nothing it is used for connected. A calendar
    /// failure beside a working account is reported in the [`DialOutcome`].
    ///
    /// # Errors
    ///
    /// Returns [`ConnectFailure`] when the account did not connect, carrying whether the stored
    /// sign-in is dead rather than the server unreachable.
    pub(crate) async fn run(
        self,
        id: &AccountId,
        display_zone: TimeZoneId,
    ) -> Result<DialOutcome, ConnectFailure> {
        let capabilities = self.capabilities().clone();
        let on = |capability| capabilities.contains(capability);
        match self {
            Self::Imap {
                config,
                connections,
                tokens,
                ..
            } => {
                let username = config.username().to_owned();
                // Calendar and contacts both talk to the same CalDAV host, so they run
                // concurrently with mail rather than making the mailbox wait for either.
                let (mail, calendar, contacts) = tokio::join!(
                    part(on(Capability::Mail), async {
                        mailcal_account::connect_mail_providers(
                            &connections,
                            &config,
                            tokens.as_ref(),
                            id,
                        )
                        .await
                        .map_err(ConnectFailure::from)
                    }),
                    part(on(Capability::Calendar) && config.caldav.is_some(), async {
                        mailcal_account::connect_caldav(&config, tokens.as_ref())
                            .await
                            .map(|provider| vec![provider])
                            .map_err(ConnectFailure::from)
                    }),
                    part(
                        on(Capability::Contacts),
                        boot::connect_caldav_contacts(&config, tokens.as_ref()),
                    ),
                );
                let assembled = assemble(mail, calendar, contacts)?;
                let calendar_error = assembled
                    .calendar_failure
                    .as_ref()
                    .map(|failure| format!("{username}: {failure}"));
                Ok(outcome(
                    id,
                    on(Capability::Mail),
                    EmailAddress::new(username),
                    assembled,
                    calendar_error,
                    false,
                ))
            }
            Self::Microsoft {
                tokens, identity, ..
            } => {
                // The same Graph token also syncs the calendar and contacts, concurrently. A
                // scope-denied `403` on the calendar sets `calendar_reauth_required`.
                let (mail, (calendar, calendar_reauth_required), contacts) = tokio::join!(
                    part(on(Capability::Mail), async {
                        mailcal_account::connect_graph_mail_providers(id, Arc::clone(&tokens), None)
                            .await
                            .map_err(ConnectFailure::from)
                    }),
                    async {
                        if on(Capability::Calendar) {
                            let (calendar, reauth) = boot::connect_graph_calendars(
                                id,
                                Arc::clone(&tokens),
                                display_zone,
                            )
                            .await;
                            (Part::from(calendar), reauth)
                        } else {
                            (Part::Off, false)
                        }
                    },
                    part(
                        on(Capability::Contacts),
                        boot::connect_graph_contacts(
                            id,
                            Arc::clone(&tokens),
                            on(Capability::Colleagues),
                        ),
                    ),
                );
                let assembled = assemble(mail, calendar, contacts)?;
                Ok(outcome(
                    id,
                    on(Capability::Mail),
                    identity,
                    assembled,
                    None,
                    calendar_reauth_required,
                ))
            }
            Self::Google {
                tokens, identity, ..
            } => {
                // Google requests every scope at sign-in, so there is no "connected before
                // calendar support" case and never a calendar re-consent to report. All three
                // spend the same token, concurrently.
                let (mail, calendar, contacts) = tokio::join!(
                    part(on(Capability::Mail), async {
                        mailcal_account::connect_google_mail_providers(Arc::clone(&tokens), None)
                            .await
                            .map_err(ConnectFailure::from)
                    }),
                    part(
                        on(Capability::Calendar),
                        boot::connect_google_calendars(id, Arc::clone(&tokens)),
                    ),
                    part(
                        on(Capability::Contacts),
                        boot::connect_google_contacts(
                            Arc::clone(&tokens),
                            on(Capability::Colleagues),
                        ),
                    ),
                );
                let assembled = assemble(mail, calendar, contacts)?;
                Ok(outcome(
                    id,
                    on(Capability::Mail),
                    identity,
                    assembled,
                    None,
                    false,
                ))
            }
            Self::Jmap { config, tokens, .. } => {
                let identity = config.identity();
                // The session says which of calendars and contacts the account has, and the mail
                // provider is what reads it. An account not used for mail still reads its session
                // that way and binds no mail from it; a session that cannot be read leaves nothing
                // on the account that could connect, so it is the account's failure either way.
                let providers =
                    mailcal_account::connect_jmap_mail_providers(&config, tokens.as_ref())
                        .await
                        .map_err(ConnectFailure::from)?;
                let calendar = part(
                    on(Capability::Calendar),
                    boot::connect_jmap_calendars(&config, tokens.as_ref(), &providers),
                )
                .await;
                let contacts = part(
                    on(Capability::Contacts),
                    boot::connect_jmap_contacts(&config, tokens.as_ref(), &providers),
                )
                .await;
                let mail = if on(Capability::Mail) {
                    Part::Bound(providers)
                } else {
                    Part::Off
                };
                let assembled = assemble(mail, calendar, contacts)?;
                Ok(outcome(
                    id,
                    on(Capability::Mail),
                    identity,
                    assembled,
                    None,
                    false,
                ))
            }
        }
    }
}

/// The dial's result, from what [`assemble`] kept.
fn outcome(
    id: &AccountId,
    uses_mail: bool,
    identity: EmailAddress,
    assembled: Assembled<
        Box<dyn engine_api::Provider>,
        Box<dyn engine_api::Provider>,
        Box<dyn engine_api::ContactsProvider>,
    >,
    calendar_error: Option<String>,
    calendar_reauth_required: bool,
) -> DialOutcome {
    DialOutcome {
        account: Account {
            id: id.clone(),
            providers: assembled.mail,
            calendar_providers: assembled.calendar,
            contact_providers: assembled.contacts,
            identity,
            dialled: true,
            uses_mail,
        },
        calendar_error,
        calendar_reauth_required,
    }
}

#[cfg(test)]
#[path = "dial_run_tests.rs"]
mod tests;
