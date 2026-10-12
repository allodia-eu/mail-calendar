//! Microsoft Autodiscover ([MS-OXDISCO]): a domain whose mail Microsoft 365 hosts points its
//! Autodiscover service at Microsoft. Its `autodiscover.{domain}` host answers a plain-HTTP `GET`
//! with a redirect to Microsoft's service, or an `_autodiscover._tcp` SRV record names it
//! ([MS-OXDISCO] §3.1.5.3, §3.1.5.4).
//!
//! **Only Microsoft's own service is acted on.** This product reaches Microsoft 365 through
//! Microsoft Graph, so an answer that lands on Exchange Online routes the address to the Microsoft
//! sign-in, and anything else, a self-hosted Exchange included, is nothing found: there is no
//! Exchange protocol here to connect it with.
//!
//! **No request carries the address.** The redirect probe is a body-less `GET` of a URL built from
//! the domain, and the SRV name is the domain's. The Autodiscover `POST` would carry the address,
//! and Exchange Online answers it with `401` until the person has signed in, so it would decide
//! nothing here.
//!
//! **The plain-HTTP hop decides a route, never a server.** The redirect's only effect is that the
//! address is offered Microsoft's sign-in, which runs over Microsoft's own TLS endpoints; no host
//! the hop names is ever sent a credential, so nothing here needs the approval an untrusted
//! autoconfig does.

use std::sync::Arc;

use mapi_autodiscover::{EmailAddress, redirect_probe_url, srv_query};
use url::Url;

use crate::{
    DetectConfig,
    fetch::{Fetch, FetchOutcome},
    mx::{self, MxResolver},
    strategy::StrategyOutcome,
    types::{DetectedMicrosoft365, EmailParts, Source, SourceKind},
};

/// The hosts Microsoft runs Autodiscover on for Microsoft 365 (Exchange Online) in its worldwide
/// cloud: a custom domain's `autodiscover` record names the first, and the redirect from it lands
/// on the second. The national clouds have hosts of their own and sign in elsewhere, so they are
/// not here.
const MICROSOFT_365_HOSTS: [&str; 2] = ["autodiscover.outlook.com", "autodiscover-s.outlook.com"];

/// Whether `host` is Microsoft's own Autodiscover service for Microsoft 365.
fn is_microsoft_365(host: &str) -> bool {
    let host = host.trim().trim_end_matches('.').to_ascii_lowercase();
    MICROSOFT_365_HOSTS.contains(&host.as_str())
}

/// Runs the strategy for `email`'s domain: the SRV record when the host resolves DNS, and the
/// redirect probe, side by side.
pub(crate) async fn run(
    fetcher: Arc<dyn Fetch>,
    email: EmailParts,
    resolver: Option<Arc<dyn MxResolver>>,
    config: DetectConfig,
) -> StrategyOutcome {
    // The crate builds its locations from an address. The normalised domain goes in rather than
    // the one typed, so the lookups name the same domain every other strategy does; a local part
    // the crate refuses (it would need escaping in the XML this strategy never sends) leaves
    // nothing to ask.
    let Ok(address) = EmailAddress::new(format!("{}@{}", email.local, email.domain)) else {
        return StrategyOutcome::Nothing;
    };
    let srv = async {
        let resolver = resolver.as_ref()?;
        let name = srv_query(&address);
        let resolution = mx::resolve_srv(resolver, &name, config.dns_timeout).await?;
        mx::usable_srv_targets(&resolution)
            .iter()
            .any(|target| is_microsoft_365(&target.target))
            .then_some(name)
    };
    let probe_url = redirect_probe_url(&address);
    let redirect = async {
        let Ok(url) = Url::parse(&probe_url) else {
            return FetchOutcome::Miss;
        };
        fetcher.get(&url).await
    };
    let (srv, redirect) = tokio::join!(srv, redirect);

    if let Some(name) = srv {
        return microsoft(name);
    }
    match redirect {
        FetchOutcome::Response(response)
            if response.final_url.host_str().is_some_and(is_microsoft_365) =>
        {
            microsoft(probe_url)
        }
        FetchOutcome::NetworkError => StrategyOutcome::NetworkError,
        FetchOutcome::Response(_) | FetchOutcome::Miss => StrategyOutcome::Nothing,
    }
}

fn microsoft(url: String) -> StrategyOutcome {
    StrategyOutcome::Microsoft365(DetectedMicrosoft365 {
        source: Source {
            kind: SourceKind::Autodiscover,
            url,
        },
    })
}

#[cfg(test)]
#[path = "autodiscover_tests.rs"]
mod autodiscover_tests;
