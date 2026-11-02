//! Telling the authorisation redirect apart from anything else that reaches a loopback listener.
//!
//! A desktop sign-in waits on `http://127.0.0.1:<port>/` for the browser to come back, and the
//! browser may ask that port for something else first, such as `/favicon.ico`. Taking the first
//! request as the redirect hands [`crate::parse_callback`] a URL with no `state`, and the sign-in
//! fails although the person completed it. A listener therefore answers every other request and
//! keeps waiting for the one this accepts.

/// Whether the request target or URL `target` is an authorisation response (RFC 6749 §4.1.2): its
/// query carries `state` and either `code` or `error`. Only the parameters' names are read;
/// [`crate::parse_callback`] still decides whether the response answers this sign-in.
#[must_use]
pub fn is_callback(target: &str) -> bool {
    let Some((_, query)) = target.split_once('?') else {
        return false;
    };
    let query = query.split('#').next().unwrap_or_default();
    let named = |name: &str| {
        query
            .split('&')
            .any(|part| part.split('=').next() == Some(name))
    };
    named("state") && (named("code") || named("error"))
}

#[cfg(test)]
mod tests {
    use super::is_callback;

    #[test]
    fn the_redirect_is_a_state_with_a_code_or_an_error() {
        assert!(is_callback("/?state=s&code=c&scope=x"));
        assert!(is_callback("http://127.0.0.1:53126/?code=c&state=s"));
        assert!(is_callback("/?error=access_denied&state=s"));
    }

    #[test]
    fn anything_else_a_browser_asks_the_port_for_is_not() {
        assert!(!is_callback("/favicon.ico"));
        assert!(!is_callback("/"));
        assert!(!is_callback("/?code=c"));
        assert!(!is_callback("/?state=s"));
        assert!(!is_callback("/?statefoo=s&code=c"));
        assert!(!is_callback("/#state=s&code=c"));
    }
}
