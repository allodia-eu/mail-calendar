//! Login Flow v2: a login page for the person, and a poll for the app.

use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use crate::{NextcloudError, REQUEST_TIMEOUT, Root, Secret, read_body};

/// How long a flow can be finished in. The server keeps a flow for 1200 seconds from when it
/// started, polled or not.
pub const FLOW_LIFETIME: Duration = Duration::from_mins(20);

/// How often a client asks whether the person has finished.
pub const POLL_INTERVAL: Duration = Duration::from_secs(2);

/// Where a flow starts, under the server's [`Root`]. With `index.php`, which answers whether or
/// not the server rewrites its URLs.
const BEGIN_PATH: &str = "/index.php/login/v2";

/// A flow the server has started and the person has not finished: what the app needs to ask for
/// its result. It carries the poll token, so it is held in memory only.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PendingFlow {
    /// The server the flow runs at, and the only one its app password will be sent to.
    pub root: Root,
    endpoint: String,
    token: Secret,
    started_at: u64,
}

impl PendingFlow {
    /// Whether the server has let the flow go by `now`.
    #[must_use]
    pub fn expired_at(&self, now: SystemTime) -> bool {
        seconds(now) >= self.started_at.saturating_add(FLOW_LIFETIME.as_secs())
    }
}

/// A flow just started: the page to open in the person's browser, and what to poll with.
#[derive(Debug)]
pub struct FlowStart {
    /// The login page, on the server's own host.
    pub login_url: String,
    /// What [`poll`] asks with.
    pub pending: PendingFlow,
}

/// What a finished flow yields: the login name to present, verbatim, and the app password.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AppCredentials {
    /// The name the person signed in with in the browser: their user id, their address, or
    /// under single sign-on an opaque id. The server accepts the app password with this name and
    /// no other spelling.
    pub login_name: String,
    /// The app password.
    pub app_password: Secret,
}

/// What one poll found.
#[derive(Debug, PartialEq, Eq)]
pub enum Poll {
    /// The person has not finished yet, or the server could not be asked this time.
    Waiting,
    /// The flow is over without a result; a new one has to start.
    Expired,
    /// The person granted access. The server answers this once and forgets it, so this is the
    /// only copy.
    Granted(AppCredentials),
}

#[derive(Deserialize)]
struct Started {
    poll: StartedPoll,
    login: String,
}

#[derive(Deserialize)]
struct StartedPoll {
    token: String,
    endpoint: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Granted {
    login_name: String,
    app_password: String,
}

/// Starts a flow at `root`. `client_name` is sent as the request's `User-Agent`, because the
/// server names the app password after it: the person reads it on the page that asks them to
/// grant access, and again in their list of devices.
///
/// # Errors
///
/// [`NextcloudError::Transport`] or [`NextcloudError::Status`] when the server does not start
/// one, [`NextcloudError::Malformed`] when its answer is not a flow, and
/// [`NextcloudError::ForeignUrl`] when it names a page or a poll endpoint on another server.
pub async fn begin(
    client: &reqwest::Client,
    root: &Root,
    client_name: &str,
) -> Result<FlowStart, NextcloudError> {
    let response = client
        .post(root.endpoint(BEGIN_PATH))
        .header(reqwest::header::USER_AGENT, client_name)
        .timeout(REQUEST_TIMEOUT)
        .send()
        .await
        .map_err(NextcloudError::Transport)?;
    if !response.status().is_success() {
        return Err(NextcloudError::Status(response.status().as_u16()));
    }
    let started: Started = serde_json::from_slice(&read_body(response).await?)
        .map_err(|_| NextcloudError::Malformed)?;
    if started.poll.token.is_empty() {
        return Err(NextcloudError::Malformed);
    }
    let login_url = root
        .owns(&started.login)
        .ok_or(NextcloudError::ForeignUrl)?;
    let endpoint = root
        .owns(&started.poll.endpoint)
        .ok_or(NextcloudError::ForeignUrl)?;
    Ok(FlowStart {
        login_url,
        pending: PendingFlow {
            root: root.clone(),
            endpoint,
            token: Secret::new(started.poll.token),
            started_at: seconds(SystemTime::now()),
        },
    })
}

/// Asks once whether the person has finished the flow `pending` started, as of `now`.
///
/// A server that cannot be asked this time is [`Poll::Waiting`], since the next poll may reach
/// it; the flow's lifetime bounds the waiting.
///
/// # Errors
///
/// [`NextcloudError::Malformed`] when the server granted access in a document that cannot be
/// read. The server has then forgotten the result, so the flow cannot be finished.
pub async fn poll(
    client: &reqwest::Client,
    pending: &PendingFlow,
    now: SystemTime,
) -> Result<Poll, NextcloudError> {
    if pending.expired_at(now) {
        return Ok(Poll::Expired);
    }
    let sent = client
        .post(&pending.endpoint)
        .form(&[("token", pending.token.expose())])
        .timeout(REQUEST_TIMEOUT)
        .send()
        .await;
    let Ok(response) = sent else {
        log::debug!("nextcloud: a poll did not reach the server");
        return Ok(Poll::Waiting);
    };
    match response.status().as_u16() {
        200 => {
            let granted: Granted = serde_json::from_slice(&read_body(response).await?)
                .map_err(|_| NextcloudError::Malformed)?;
            if granted.login_name.is_empty() || granted.app_password.is_empty() {
                return Err(NextcloudError::Malformed);
            }
            Ok(Poll::Granted(AppCredentials {
                login_name: granted.login_name,
                app_password: Secret::new(granted.app_password),
            }))
        }
        404 => Ok(Poll::Waiting),
        status => {
            log::debug!("nextcloud: a poll answered {status}");
            Ok(Poll::Waiting)
        }
    }
}

/// `time` in whole seconds since the Unix epoch.
fn seconds(time: SystemTime) -> u64 {
    time.duration_since(UNIX_EPOCH)
        .map_or(0, |since| since.as_secs())
}

#[cfg(test)]
#[path = "flow_tests.rs"]
mod flow_tests;
