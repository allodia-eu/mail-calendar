//! The unauthenticated request that opens discovery, and the redirects it may follow.
//!
//! The discovery client follows no redirect, because a metadata document must come from the URL
//! that named it. This request fetches no document: it asks the resource for its challenge, and
//! a JMAP server is free to answer the well-known session path with a redirect to the session
//! itself. Fastmail does: `/.well-known/jmap` answers `302` to `/jmap/session`, and only that
//! answers the `401` naming its metadata, which lives behind the session's own path, so the
//! well-known fallback for the unredirected URL finds nothing. A hop is followed only within the
//! resource's own HTTPS origin: the server we asked is still the one answering.

/// How many redirects the probe follows before taking the answer it has.
const MAX_HOPS: usize = 3;

/// Sends the unauthenticated request to `resource_url`, following redirects within its origin.
/// The answer's own [`reqwest::Response::url`] is the resource that answered.
pub(crate) async fn send(
    http: &reqwest::Client,
    resource_url: &url::Url,
) -> Result<reqwest::Response, reqwest::Error> {
    let mut response = http.get(resource_url.clone()).send().await?;
    for _ in 0..MAX_HOPS {
        let location = response
            .headers()
            .get(reqwest::header::LOCATION)
            .and_then(|value| value.to_str().ok());
        let Some(next) = same_origin_hop(response.url(), response.status(), location) else {
            break;
        };
        response = http.get(next).send().await?;
    }
    Ok(response)
}

/// Where a redirect from `from` leads, when the probe may follow it: a redirect status whose
/// `Location` stays on `from`'s HTTPS origin.
fn same_origin_hop(
    from: &url::Url,
    status: reqwest::StatusCode,
    location: Option<&str>,
) -> Option<url::Url> {
    if !status.is_redirection() {
        return None;
    }
    let next = from.join(location?).ok()?;
    (next.scheme() == "https" && next.origin() == from.origin()).then_some(next)
}

#[cfg(test)]
mod tests {
    use reqwest::StatusCode;

    use super::same_origin_hop;

    fn session() -> url::Url {
        url::Url::parse("https://api.example.com/.well-known/jmap").unwrap()
    }

    #[test]
    fn a_redirect_to_the_session_on_the_same_origin_is_followed() {
        let next = same_origin_hop(&session(), StatusCode::FOUND, Some("/jmap/session"));
        assert_eq!(
            next.map(String::from).as_deref(),
            Some("https://api.example.com/jmap/session")
        );
        let absolute = same_origin_hop(
            &session(),
            StatusCode::PERMANENT_REDIRECT,
            Some("https://api.example.com/jmap/session"),
        );
        assert!(absolute.is_some());
    }

    #[test]
    fn a_redirect_off_the_origin_or_off_https_is_not() {
        for location in [
            "https://elsewhere.example.net/jmap/session",
            "http://api.example.com/jmap/session",
            "https://api.example.com:8443/jmap/session",
        ] {
            assert_eq!(
                same_origin_hop(&session(), StatusCode::FOUND, Some(location)),
                None,
                "{location}"
            );
        }
    }

    #[test]
    fn only_a_redirect_with_somewhere_to_go_is_a_hop() {
        assert_eq!(
            same_origin_hop(&session(), StatusCode::UNAUTHORIZED, Some("/jmap/session")),
            None
        );
        assert_eq!(same_origin_hop(&session(), StatusCode::FOUND, None), None);
    }
}
