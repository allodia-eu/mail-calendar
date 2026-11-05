//! The two OCS calls an app password makes about itself: who it signs in as, and withdrawing it.

use serde::Deserialize;

use crate::{AppCredentials, NextcloudError, REQUEST_TIMEOUT, Root, read_body};

/// The signed-in user, as the Login Flow documentation tells a client to learn it.
const USER_PATH: &str = "/ocs/v2.php/cloud/user?format=json";

/// The app password the request authenticates with.
const APP_PASSWORD_PATH: &str = "/ocs/v2.php/core/apppassword";

/// Who an app password signs in as.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct User {
    /// The user's id: stable, and the same whichever name they signed in with.
    pub id: String,
    /// The address on their Nextcloud profile, when it has one.
    pub email: Option<String>,
}

#[derive(Deserialize)]
struct Envelope {
    ocs: Ocs,
}

#[derive(Deserialize)]
struct Ocs {
    data: UserData,
}

#[derive(Deserialize)]
struct UserData {
    id: String,
    #[serde(default)]
    email: Option<String>,
}

/// Who `credentials` sign in as at `root`.
///
/// # Errors
///
/// [`NextcloudError::Transport`] or [`NextcloudError::Status`] when the server does not answer,
/// and [`NextcloudError::Malformed`] when it answers something that names no user.
pub async fn current_user(
    client: &reqwest::Client,
    root: &Root,
    credentials: &AppCredentials,
) -> Result<User, NextcloudError> {
    let response = client
        .get(root.endpoint(USER_PATH))
        .basic_auth(
            &credentials.login_name,
            Some(credentials.app_password.expose()),
        )
        .header("OCS-APIRequest", "true")
        .header(reqwest::header::ACCEPT, "application/json")
        .timeout(REQUEST_TIMEOUT)
        .send()
        .await
        .map_err(NextcloudError::Transport)?;
    if !response.status().is_success() {
        return Err(NextcloudError::Status(response.status().as_u16()));
    }
    let envelope: Envelope = serde_json::from_slice(&read_body(response).await?)
        .map_err(|_| NextcloudError::Malformed)?;
    let data = envelope.ocs.data;
    if data.id.is_empty() {
        return Err(NextcloudError::Malformed);
    }
    Ok(User {
        id: data.id,
        email: data.email.filter(|email| !email.trim().is_empty()),
    })
}

/// Withdraws the app password in `credentials` at `root`, so it leaves the server's list of
/// devices with the account rather than lingering there for a year.
///
/// # Errors
///
/// [`NextcloudError::Transport`] or [`NextcloudError::Status`] when the server did not withdraw
/// it. The caller goes on either way: a credential it no longer holds is no use to it.
pub async fn revoke(
    client: &reqwest::Client,
    root: &Root,
    credentials: &AppCredentials,
) -> Result<(), NextcloudError> {
    let response = client
        .delete(root.endpoint(APP_PASSWORD_PATH))
        .basic_auth(
            &credentials.login_name,
            Some(credentials.app_password.expose()),
        )
        .header("OCS-APIRequest", "true")
        .timeout(REQUEST_TIMEOUT)
        .send()
        .await
        .map_err(NextcloudError::Transport)?;
    if response.status().is_success() {
        Ok(())
    } else {
        Err(NextcloudError::Status(response.status().as_u16()))
    }
}

#[cfg(test)]
#[path = "ocs_tests.rs"]
mod ocs_tests;
