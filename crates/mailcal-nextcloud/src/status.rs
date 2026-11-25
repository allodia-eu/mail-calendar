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

/// What a server's `status.php` says it is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    /// An installed Nextcloud server that is not in maintenance, and so can run a sign-in now.
    Nextcloud,
    /// An installed Nextcloud server in maintenance, which runs no sign-in until it is over.
    Maintenance,
    /// The document says something else: another product, or a server not installed yet.
    Other,
    /// No status document could be read: no answer, a refusal, or a body that is none.
    Unknown,
}

/// What a `status.php` body says the server is.
#[must_use]
pub fn status_of_body(body: &[u8]) -> Status {
    let Ok(Value::Object(status)) = serde_json::from_slice::<Value>(body) else {
        return Status::Unknown;
    };
    let Some(product) = status.get("productname").and_then(Value::as_str) else {
        return Status::Unknown;
    };
    if status.get("installed") != Some(&Value::Bool(true))
        || !product.eq_ignore_ascii_case("Nextcloud")
    {
        Status::Other
    } else if status.get("maintenance") == Some(&Value::Bool(true)) {
        Status::Maintenance
    } else {
        Status::Nextcloud
    }
}

/// Whether a `status.php` body describes an installed Nextcloud server that is not in
/// maintenance, and so can run a sign-in now.
#[must_use]
pub fn is_nextcloud_status(body: &[u8]) -> bool {
    status_of_body(body) == Status::Nextcloud
}

/// Whether `root` answers as Nextcloud. Never an error: a server that does not answer, or answers
/// anything else, is not one this sign-in can be offered for.
pub async fn is_nextcloud(client: &reqwest::Client, root: &Root) -> bool {
    status_of(client, root).await == Status::Nextcloud
}

/// What `root`'s `status.php` says it is. Never an error: what cannot be read is
/// [`Status::Unknown`].
pub async fn status_of(client: &reqwest::Client, root: &Root) -> Status {
    let response = client
        .get(root.endpoint(STATUS_PATH))
        .timeout(STATUS_TIMEOUT)
        .send()
        .await;
    let Ok(response) = response else {
        log::debug!("nextcloud: the status document did not answer");
        return Status::Unknown;
    };
    if !response.status().is_success() {
        log::debug!(
            "nextcloud: the status document answered {}",
            response.status().as_u16()
        );
        return Status::Unknown;
    }
    crate::read_body(response)
        .await
        .map_or(Status::Unknown, |body| status_of_body(&body))
}

#[cfg(test)]
#[path = "status_tests.rs"]
mod status_tests;
