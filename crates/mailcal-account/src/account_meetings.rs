//! The services an account makes video meetings through (`docs/conferencing.md` rule 14): Teams
//! through a Microsoft account's Graph, Talk through a Nextcloud account's own server.

use std::sync::Arc;

use async_trait::async_trait;
use engine_api::{Conference, ConferenceService, MeetingRequest, MeetingService};
use engine_provider::{ProviderError, ProviderResult};
use engine_tls::TlsClientConfig;
use provider_caldav::{Credentials, TalkMeetings};
use provider_graph::{GraphClient, MailboxPrincipal, TeamsMeetings};

use crate::{AccountConfig, AccountError, GraphTokenSource, tls::tls_with};

/// Teams meetings made as the signed-in user of a Microsoft account. Each call takes a current
/// access token from the account's token source, since a Graph client holds the one it was built
/// with and a meeting may be made hours after the account connected.
struct AccountTeams {
    tokens: Arc<GraphTokenSource>,
    tls: TlsClientConfig,
}

impl AccountTeams {
    async fn teams(&self) -> ProviderResult<TeamsMeetings> {
        let token = self.tokens.access_token().await.map_err(|err| match err {
            AccountError::SigninRejected(detail) => ProviderError::authentication(detail),
            other => ProviderError::retryable(other.to_string()),
        })?;
        let client = GraphClient::for_mailbox(
            token,
            MailboxPrincipal::Me,
            &self.tls,
            &self.tokens.retry(),
            &crate::user_agent(),
        )
        .map_err(ProviderError::from)?;
        Ok(TeamsMeetings::new(client))
    }
}

#[async_trait]
impl MeetingService for AccountTeams {
    fn service(&self) -> ConferenceService {
        ConferenceService::Teams
    }

    async fn create_meeting(&self, request: &MeetingRequest) -> ProviderResult<Conference> {
        self.teams().await?.create_meeting(request).await
    }

    async fn delete_meeting(&self, meeting: &Conference) -> ProviderResult<()> {
        self.teams().await?.delete_meeting(meeting).await
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
        tokens,
        tls: tls_with(&[])?,
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

/// Asks `config`'s Nextcloud server whether the account can make Talk meetings there, signing in
/// with the account's own app password. Nothing is sent for an account no Nextcloud sign-in made.
pub async fn talk_at_connect(config: &AccountConfig) -> TalkAtConnect {
    let Some((login, name, password)) = config.nextcloud_app_password() else {
        return TalkAtConnect::NotOffered;
    };
    let talk = crate::tls::account_tls(config)
        .map_err(|err| err.to_string())
        .and_then(|tls| {
            TalkMeetings::new(
                &login.server,
                Credentials::Basic {
                    username: name.to_owned(),
                    password: password.expose().to_owned(),
                },
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
