//! The one rule every token-refreshing provider wrapper follows when it reports
//! [`ConnectionInfo`]: cap the capabilities, forward the transport facts.
//!
//! A wrapper reports its **own** capabilities on purpose: a flag omitted there is a flag the
//! account does not have, however loudly the adapter underneath advertises it, and that is what
//! stops us promising a write we never forward. The other fields are not promises at all. They
//! are observations about the delegate's live connection, and a wrapper that reconstructs them
//! from [`ConnectionInfo::new`] does not report "unknown"; it reports the *default*, which is a
//! confident wrong answer.
//!
//! That distinction had already cost something before this function existed:
//! `concurrent_fetches` paces the host's body warm, so a Gmail account whose wrapper rebuilt it
//! reported `1` and warmed one message per round trip; on the provider where that costs the
//! most; while the adapter underneath was reporting 20.

use engine_provider::ConnectionInfo;

/// Combines a wrapper's capped `capabilities` with the live `delegate`'s transport facts.
///
/// `delegate` is `None` before one has been built (nothing has dialled yet), in which case the
/// capped value's own conservative defaults stand: no TLS or HTTP version observed, and a fetch
/// width of one. All three self-correct once a delegate exists.
pub(crate) fn with_delegate_transport(
    capped: ConnectionInfo,
    delegate: Option<ConnectionInfo>,
) -> ConnectionInfo {
    let Some(delegate) = delegate else {
        return capped;
    };
    ConnectionInfo {
        tls_version: delegate.tls_version,
        http_version: delegate.http_version,
        concurrent_fetches: delegate.concurrent_fetches,
        ..capped
    }
}

#[cfg(test)]
mod tests {
    use engine_provider::{Capabilities, ConnectionInfo, HttpVersion};

    use super::with_delegate_transport;

    fn capped() -> ConnectionInfo {
        ConnectionInfo::new(Capabilities::none().with_mail())
    }

    fn delegate() -> ConnectionInfo {
        ConnectionInfo {
            http_version: Some(HttpVersion::Http2),
            // A capability the wrapper deliberately does not forward.
            ..ConnectionInfo::new(Capabilities::none().with_mail().with_mail_writes())
                .with_concurrent_fetches(20)
        }
    }

    #[test]
    fn the_delegates_transport_facts_reach_the_caller() {
        // The regression: a wrapper that rebuilt this value reported the *default* width of 1
        // rather than the delegate's, and the body warm believed it; one round trip per
        // message on exactly the provider that can overlap them.
        let info = with_delegate_transport(capped(), Some(delegate()));
        assert_eq!(info.concurrent_fetches, 20);
        assert_eq!(info.http_version, Some(HttpVersion::Http2));
    }

    #[test]
    fn the_wrappers_capability_cap_still_wins() {
        // The half that must NOT be forwarded: the delegate advertises mail writes, the
        // wrapper does not forward them, so the account does not have them.
        let info = with_delegate_transport(capped(), Some(delegate()));
        assert!(info.capabilities.mail());
        assert!(
            !info.capabilities.mail_writes(),
            "a capability the wrapper caps is not restored by forwarding transport facts",
        );
    }

    #[test]
    fn before_a_delegate_exists_the_conservative_defaults_stand() {
        let info = with_delegate_transport(capped(), None);
        assert_eq!(info.concurrent_fetches, 1);
        assert_eq!(info.http_version, None);
    }
}
