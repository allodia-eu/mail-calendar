//! The services an account makes video meetings through (`docs/conferencing.md` rule 14): Teams
//! through a Microsoft account's Graph, Meet through a Google account, Talk through a Nextcloud
//! account's own server.

use std::sync::Arc;

use async_trait::async_trait;
use engine_api::{Conference, ConferenceService, MeetingRequest, MeetingService};
use engine_provider::{CredentialSource, ProviderResult};
use provider_caldav::TalkMeetings;
use provider_google::{GoogleClient, MeetMeetings};
use provider_graph::{GraphClient, MailboxPrincipal, TeamsMeetings};

use crate::{AccountConfig, AccountError, GraphTokenSource, tls::tls_with};

/// What connects a new client of type `C` for the account.
type Connect<C> = Box<dyn Fn() -> ProviderResult<C> + Send + Sync>;

/// Teams meetings made as the signed-in user of a Microsoft account. Each call is made on a client
/// of its own, which asks the account's token source for the token: a meeting may be made hours
/// after the last one, and a connection kept that long may have died while the machine slept.
struct AccountTeams {
    connect: Connect<GraphClient>,
}

impl AccountTeams {
    fn teams(&self) -> ProviderResult<TeamsMeetings> {
        Ok(TeamsMeetings::new((self.connect)()?))
    }
}

#[async_trait]
impl MeetingService for AccountTeams {
    fn service(&self) -> ConferenceService {
        ConferenceService::Teams
    }

    async fn create_meeting(&self, request: &MeetingRequest) -> ProviderResult<Conference> {
        self.teams()?.create_meeting(request).await
    }

    async fn delete_meeting(&self, meeting: &Conference) -> ProviderResult<()> {
        self.teams()?.delete_meeting(meeting).await
    }
}

/// The Teams meetings a Microsoft account makes, through its token source.
///
/// # Errors
///
/// [`AccountError::Tls`] when the trust policy cannot be built.
pub fn teams_meetings(
    tokens: Arc<GraphTokenSource>,
) -> Result<Arc<dyn MeetingService>, AccountError> {
    Ok(Arc::new(AccountTeams {
        connect: Box::new(crate::graph::connector(
            tokens,
            MailboxPrincipal::Me,
            tls_with(&[])?,
            GraphClient::connect_with_source,
        )),
    }))
}

/// Meet meetings made as the signed-in user of a Google account, each on a client of its own, as
/// [`AccountTeams`] does.
struct AccountMeet {
    connect: Connect<GoogleClient>,
}

impl AccountMeet {
    fn meet(&self) -> ProviderResult<MeetMeetings> {
        Ok(MeetMeetings::new((self.connect)()?))
    }
}

#[async_trait]
impl MeetingService for AccountMeet {
    fn service(&self) -> ConferenceService {
        ConferenceService::GoogleMeet
    }

    async fn create_meeting(&self, request: &MeetingRequest) -> ProviderResult<Conference> {
        self.meet()?.create_meeting(request).await
    }

    async fn delete_meeting(&self, meeting: &Conference) -> ProviderResult<()> {
        self.meet()?.delete_meeting(meeting).await
    }
}

/// The Meet meetings a Google account makes, through its token source.
///
/// # Errors
///
/// [`AccountError::Tls`] when the trust policy cannot be built.
pub fn meet_meetings(
    tokens: Arc<GraphTokenSource>,
) -> Result<Arc<dyn MeetingService>, AccountError> {
    Ok(Arc::new(AccountMeet {
        connect: Box::new(crate::google::connector(
            tokens,
            tls_with(&[])?,
            GoogleClient::connect_with_source,
        )),
    }))
}

/// What a Nextcloud account's server said about Talk when the account connected.
pub enum TalkAtConnect {
    /// The account makes Talk meetings through this service.
    Offered(Arc<dyn MeetingService>),
    /// The account is no Nextcloud sign-in's, the server has no Talk, or Talk lets the account
    /// start no conversation.
    NotOffered,
    /// The server could not be asked; the next connect asks again.
    Unknown,
}

impl core::fmt::Debug for TalkAtConnect {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(match self {
            Self::Offered(_) => "Offered",
            Self::NotOffered => "NotOffered",
            Self::Unknown => "Unknown",
        })
    }
}

/// Where Talk is asked: the server a Nextcloud sign-in made the account at, and the source it
/// signs in with there.
type TalkSignIn<'a> = (&'a str, Arc<dyn CredentialSource>);

/// The server a Nextcloud sign-in made the account at, and the source Talk signs in with there:
/// the app password of the DAV endpoint that stored it, under that login's subject
/// (`crate::config_dav`). `None` for an account no Nextcloud sign-in made.
fn talk_source(config: &AccountConfig) -> Option<Result<TalkSignIn<'_>, AccountError>> {
    let (login, endpoint, dav_use) = config.nextcloud_endpoint()?;
    endpoint.password?;
    Some(
        crate::config_dav::dav_credential_source(config, endpoint, dav_use, None)
            .map(|source| (login.server.as_str(), source)),
    )
}

/// Asks `config`'s Nextcloud server whether the account can make Talk meetings there, signing in
/// with the account's own app password under the subject of the DAV login that holds it. Nothing
/// is sent for an account no Nextcloud sign-in made.
pub async fn talk_at_connect(config: &AccountConfig) -> TalkAtConnect {
    let Some(source) = talk_source(config) else {
        return TalkAtConnect::NotOffered;
    };
    let talk = source
        .and_then(|(server, source)| Ok((server, source, crate::tls::account_tls(config)?)))
        .map_err(|err| err.to_string())
        .and_then(|(server, source, tls)| {
            TalkMeetings::from_credential_source(
                server,
                source,
                &tls,
                &crate::throttle::ungated_retry(),
                &crate::user_agent(),
            )
            .map_err(|err| err.to_string())
        });
    let talk = match talk {
        Ok(talk) => talk,
        Err(err) => {
            log::warn!("talk: the server cannot be asked: {err}");
            return TalkAtConnect::Unknown;
        }
    };
    match talk.offered().await {
        Ok(true) => TalkAtConnect::Offered(Arc::new(talk)),
        Ok(false) => TalkAtConnect::NotOffered,
        Err(err) => {
            log::warn!("talk: the server did not say whether it has Talk: {err}");
            TalkAtConnect::Unknown
        }
    }
}

#[cfg(test)]
#[path = "account_meetings_tests.rs"]
mod tests;
