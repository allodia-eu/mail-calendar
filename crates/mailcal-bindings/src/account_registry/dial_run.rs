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

/// The mailbox's part: opened as [`part`] would, or, when the dial is of the uses alone, taken as
/// connected without opening it, so its calendar and contacts assemble as they do beside a
/// working mailbox.
async fn mail_part<T, F>(on: bool, open_mail: bool, open: F) -> Part<T>
where
    F: Future<Output = Result<Vec<T>, ConnectFailure>>,
{
    if on && !open_mail {
        Part::Bound(Vec::new())
    } else {
        part(on, open).await
    }
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
        self.open(id, display_zone, true).await
    }

    /// Opens the account's calendar and contacts again, and never its mailbox: the account is
    /// connected, and what failed beside it is all a retry has to reach. The outcome carries no
    /// mail providers; [`App::replace_uses`](mailcal_app::App::replace_uses) keeps the live ones.
    ///
    /// A JMAP account still reads its session, since its calendar and contacts are bound from it.
    ///
    /// # Errors
    ///
    /// Returns [`ConnectFailure`] only for a JMAP session that could not be read, or for an
    /// account without mail of which nothing connected.
    pub(crate) async fn run_uses(
        self,
        id: &AccountId,
        display_zone: TimeZoneId,
    ) -> Result<DialOutcome, ConnectFailure> {
        self.open(id, display_zone, false).await
    }

    async fn open(
        self,
        id: &AccountId,
        display_zone: TimeZoneId,
        open_mail: bool,
    ) -> Result<DialOutcome, ConnectFailure> {
        let capabilities = self.capabilities().clone();
        let on = |capability| capabilities.contains(capability);
        let shared = self.shared_handle().cloned();
        match self {
            Self::Imap {
                config,
                connections,
                tokens,
                shared: ref shared_dial,
                ..
            } => {
                let username = config.username().to_owned();
                let identity = shared_dial.as_ref().map_or_else(
                    || EmailAddress::new(username.clone()),
                    |shared| shared.identity.clone(),
                );
                let learned_calendar = std::sync::Mutex::new(mailcal_account::Learned::Nothing);
                let learned_contacts = std::sync::Mutex::new(mailcal_account::Learned::Nothing);
                // Calendar and contacts both talk to the same CalDAV host, so they run
                // concurrently with mail rather than making the mailbox wait for either.
                let (mail, calendar, contacts) = tokio::join!(
                    mail_part(on(Capability::Mail), open_mail, async {
                        mailcal_account::connect_mail_providers(
                            &connections,
                            &config,
                            tokens.as_ref(),
                            id,
                            shared.as_ref(),
                        )
                        .await
                        .map_err(ConnectFailure::from)
                    }),
                    part(on(Capability::Calendar) && config.caldav.is_some(), async {
                        mailcal_account::connect_caldav_learning(&config, tokens.as_ref())
                            .await
                            .map(|connected| {
                                *learned_calendar.lock().expect("learned mutex poisoned") =
                                    mailcal_account::Learned::connected(connected.principal);
                                vec![connected.providers]
                            })
                            .map_err(ConnectFailure::from)
                    }),
                    part(
                        on(Capability::Contacts),
                        boot::connect_caldav_contacts(
                            &config,
                            tokens.as_ref(),
                            on(Capability::Mail),
                            &learned_contacts,
                        ),
                    ),
                );
                let assembled = assemble(mail, calendar, contacts)?;
                let calendar_error = assembled
                    .calendar_failure
                    .as_ref()
                    .map(|failure| format!("{username}: {failure}"));
                let learned = mailcal_account::DavLearned {
                    calendar: learned_calendar
                        .into_inner()
                        .expect("learned mutex poisoned"),
                    contacts: learned_contacts
                        .into_inner()
                        .expect("learned mutex poisoned"),
                };
                Ok(outcome(
                    id,
                    on(Capability::Mail),
                    identity,
                    assembled,
                    calendar_error,
                    false,
                    learned,
                ))
            }
            Self::Microsoft {
                tokens,
                identity,
                withheld,
                affiliation_unknown,
                ..
            } => {
                // The same Graph token also syncs the calendar and contacts, concurrently. A
                // calendar the grant withholds, or a scope-denied `403` on it, sets
                // `calendar_reauth_required`. An account stored before its affiliation was
                // recorded is asked beside them, and the token sink stores the answer.
                let (mail, (calendar, calendar_reauth_required), contacts, ()) = tokio::join!(
                    mail_part(on(Capability::Mail), open_mail, async {
                        mailcal_account::connect_graph_mail_providers(
                            id,
                            Arc::clone(&tokens),
                            None,
                            shared.as_ref(),
                        )
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
                            (Part::Off, withheld.contains(Capability::Calendar))
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
                    async {
                        if affiliation_unknown {
                            tokens.detect_affiliation().await;
                        }
                    },
                );
                let assembled = assemble(mail, calendar, contacts)?;
                Ok(outcome(
                    id,
                    on(Capability::Mail),
                    identity,
                    assembled,
                    None,
                    calendar_reauth_required,
                    mailcal_account::DavLearned::default(),
                ))
            }
            Self::Google {
                tokens,
                identity,
                affiliation_unknown,
                ..
            } => {
                // Google has no "connected before calendar support" case, so there is never a
                // calendar re-consent to report; a calendar the grant withholds is not opened.
                // All three spend the same token, concurrently, and an account stored before its
                // affiliation was recorded is asked beside them.
                let (mail, calendar, contacts, ()) = tokio::join!(
                    mail_part(on(Capability::Mail), open_mail, async {
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
                    async {
                        if affiliation_unknown {
                            tokens.detect_affiliation().await;
                        }
                    },
                );
                let assembled = assemble(mail, calendar, contacts)?;
                Ok(outcome(
                    id,
                    on(Capability::Mail),
                    identity,
                    assembled,
                    None,
                    false,
                    mailcal_account::DavLearned::default(),
                ))
            }
            Self::Jmap {
                config,
                tokens,
                session,
                shared: shared_dial,
                ..
            } => {
                let identity =
                    shared_dial.map_or_else(|| config.identity(), |shared| shared.identity);
                // The session says which of mail, calendars and contacts the account has, and the
                // mail provider is what reads it. An account not used for mail still reads its
                // session that way and binds no mail from it; a session that cannot be read leaves
                // nothing on the account that could connect, so it is the account's failure either
                // way.
                let providers = mailcal_account::connect_jmap_mail_providers(
                    &config,
                    tokens.as_ref(),
                    shared.as_ref(),
                )
                .await
                .map_err(ConnectFailure::from)?;
                // A shared mailbox's session is its parent's to record, not this dial's.
                if let Some(provider) = providers.first().filter(|_| shared.is_none()) {
                    session.record(provider.connection_info().capabilities);
                }
                let has_mail = shared.is_some() || !session.lacks().contains(Capability::Mail);
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
                let uses_mail = on(Capability::Mail) && has_mail;
                let mail = if uses_mail {
                    Part::Bound(providers)
                } else {
                    Part::Off
                };
                let assembled = assemble(mail, calendar, contacts)?;
                Ok(outcome(
                    id,
                    uses_mail,
                    identity,
                    assembled,
                    None,
                    false,
                    mailcal_account::DavLearned::default(),
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
    learned: mailcal_account::DavLearned,
) -> DialOutcome {
    // A calendar withheld for a missing scope already reads as "needs permission"; it is not also
    // a failure to report.
    let rediscover = assembled
        .calendar_failure
        .iter()
        .chain(&assembled.contacts_failure)
        .any(ConnectFailure::worth_rediscovering);
    let use_problems = mailcal_app::UseProblems {
        calendar: assembled
            .calendar_failure
            .as_ref()
            .filter(|_| !calendar_reauth_required)
            .map(ConnectFailure::use_problem),
        contacts: assembled
            .contacts_failure
            .as_ref()
            .map(ConnectFailure::use_problem),
    };
    let unanswered = mailcal_app::Unanswered {
        calendar: assembled
            .calendar_failure
            .as_ref()
            .is_some_and(ConnectFailure::is_outage),
        contacts: assembled
            .contacts_failure
            .as_ref()
            .is_some_and(ConnectFailure::is_outage),
    };
    DialOutcome {
        account: Account {
            id: id.clone(),
            providers: assembled.mail.into(),
            calendar_provider: assembled.calendar.into_iter().next(),
            contact_providers: assembled.contacts,
            identity,
            dialled: true,
            uses_mail,
        },
        calendar_error,
        calendar_reauth_required,
        use_problems,
        unanswered,
        learned,
        rediscover,
    }
}

#[cfg(test)]
#[path = "dial_run_tests.rs"]
mod tests;
