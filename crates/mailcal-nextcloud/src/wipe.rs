//! Remote wipe: Nextcloud's "Wipe device", which a person chooses for one app password in their
//! list of devices. From then on the password is refused, and the server answers a question asked
//! with it: should this device delete what it holds?
//!
//! Both calls are limited to ten per address in five minutes, so a client asks once per refusal,
//! never on every retry.

use serde::Deserialize;

use crate::{NextcloudError, REQUEST_TIMEOUT, Root, Secret, read_body};

const CHECK_PATH: &str = "/index.php/core/wipe/check";
const SUCCESS_PATH: &str = "/index.php/core/wipe/success";

#[derive(Deserialize)]
struct Check {
    wipe: bool,
}

/// Whether the server asks this device to delete what `app_password` reached. Asked after the
/// password was refused, because a password marked for wipe is refused everywhere else.
///
/// # Errors
///
/// [`NextcloudError::Transport`] or [`NextcloudError::Status`] when the server gave no answer,
/// and [`NextcloudError::Malformed`] when its answer cannot be read. Neither is a wipe.
pub async fn wipe_requested(
    client: &reqwest::Client,
    root: &Root,
    app_password: &Secret,
) -> Result<bool, NextcloudError> {
    let response = client
        .post(root.endpoint(CHECK_PATH))
        .form(&[("token", app_password.expose())])
        .timeout(REQUEST_TIMEOUT)
        .send()
        .await
        .map_err(NextcloudError::Transport)?;
    match response.status().as_u16() {
        200 => {
            let check: Check = serde_json::from_slice(&read_body(response).await?)
                .map_err(|_| NextcloudError::Malformed)?;
            Ok(check.wipe)
        }
        404 => Ok(false),
        status => Err(NextcloudError::Status(status)),
    }
}

/// Tells the server this device has deleted what `app_password` reached, which also deletes the
/// password there and shows the person the wipe finished.
///
/// # Errors
///
/// [`NextcloudError::Transport`] or [`NextcloudError::Status`] when the server did not take it.
pub async fn wipe_finished(
    client: &reqwest::Client,
    root: &Root,
    app_password: &Secret,
) -> Result<(), NextcloudError> {
    let response = client
        .post(root.endpoint(SUCCESS_PATH))
        .form(&[("token", app_password.expose())])
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
#[path = "wipe_tests.rs"]
mod wipe_tests;
