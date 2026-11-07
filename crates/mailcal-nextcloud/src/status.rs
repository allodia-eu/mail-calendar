//! Whether a server is Nextcloud, asked the way Nextcloud's own clients ask it: `status.php`.
//!
//! Its `productname` defaults to "Nextcloud" and the Theming settings cannot change it, unlike
//! the instance name a `WWW-Authenticate` realm carries, which any administrator can. ownCloud and
//! ownCloud Infinite Scale answer the same document with their own names, and neither offers this
//! sign-in.

use std::time::Duration;

use serde_json::Value;

use crate::Root;

/// How long the question may take. It decides only whether a button is drawn, and a server that
/// cannot answer this quickly is offered the password, which works everywhere.
pub const STATUS_TIMEOUT: Duration = Duration::from_secs(3);

/// The path of the document, under the server's [`Root`].
pub const STATUS_PATH: &str = "/status.php";

/// Whether a `status.php` body describes an installed Nextcloud server that is not in
/// maintenance, and so can run a sign-in now.
#[must_use]
pub fn is_nextcloud_status(body: &[u8]) -> bool {
    let Ok(Value::Object(status)) = serde_json::from_slice::<Value>(body) else {
        return false;
    };
    status.get("installed") == Some(&Value::Bool(true))
        && status.get("maintenance") != Some(&Value::Bool(true))
        && status
            .get("productname")
            .and_then(Value::as_str)
            .is_some_and(|name| name.eq_ignore_ascii_case("Nextcloud"))
}

/// Whether `root` answers as Nextcloud. Never an error: a server that does not answer, or answers
/// anything else, is not one this sign-in can be offered for.
pub async fn is_nextcloud(client: &reqwest::Client, root: &Root) -> bool {
    let response = client
        .get(root.endpoint(STATUS_PATH))
        .timeout(STATUS_TIMEOUT)
        .send()
        .await;
    let Ok(response) = response else {
        log::debug!("nextcloud: the status document did not answer");
        return false;
    };
    if !response.status().is_success() {
        log::debug!(
            "nextcloud: the status document answered {}",
            response.status().as_u16()
        );
        return false;
    }
    crate::read_body(response)
        .await
        .is_ok_and(|body| is_nextcloud_status(&body))
}

#[cfg(test)]
#[path = "status_tests.rs"]
mod status_tests;
