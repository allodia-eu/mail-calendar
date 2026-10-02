//! Detection's scripted answers in a showcase (screenshot) build.

#[cfg(test)]
use super::{SetupRecommendation, convert};

/// The domain whose canned detection is the **happy path**: everything published over HTTPS,
/// with a calendar found beside the mailbox. The showcase's work account lives here, so the
/// documentation's setup walkthrough and its mailbox screenshots tell one story.
const SHOWCASE_TRUSTED_DOMAIN: &str = "northwind.example";

/// The domain whose canned detection comes back **untrusted**; settings that were only
/// reachable over a plain-HTTP hop. This is the screen that matters most in the setup
/// documentation: the one place a user is asked to approve something before a password is sent
/// (docs/account-autodetect.md), and the one a real provider gives us no reliable way to stage.
const SHOWCASE_UNTRUSTED_DOMAIN: &str = "oldschool.example";

/// Detection's answer in a showcase (screenshot) build: scripted, instant, and offline.
///
/// Three outcomes, because the account-setup documentation has three screens to show; the
/// settings were found and are trustworthy, they were found over an insecure hop and need
/// approval, and nothing was found so the manual form takes over. Every other address falls to
/// the last one, which is what makes the personal-domain example in the guide land on the manual
/// route without a special case.
///
/// Both domains are `.example` (RFC 2606), so even if this were ever reached outside a showcase
/// build it could not name a host that resolves. Adding the Microsoft or Google route here is a
/// new arm and nothing else, once a guide needs to picture one.
#[cfg(test)]
pub(super) fn showcase_recommendation(email: &str) -> SetupRecommendation {
    convert(showcase_route(email))
}

/// [`showcase_recommendation`] in the account layer's terms.
pub(super) fn showcase_route(email: &str) -> mailcal_account::SetupRecommendation {
    use mailcal_account::{
        ConnectionSecurity, MissReason, ServerSummary as DetectedServerRow, SetupRecommendation,
    };

    let Some((_, domain)) = email.rsplit_once('@') else {
        return SetupRecommendation::Manual {
            reason: MissReason::InvalidEmail,
        };
    };
    let domain = domain.trim().to_ascii_lowercase();
    if domain.is_empty() {
        return SetupRecommendation::Manual {
            reason: MissReason::InvalidEmail,
        };
    }
    let trusted = domain == SHOWCASE_TRUSTED_DOMAIN;
    if !trusted && domain != SHOWCASE_UNTRUSTED_DOMAIN {
        return SetupRecommendation::Manual {
            reason: MissReason::NothingFound,
        };
    }
    let imap_host = format!("imap.{domain}");
    let smtp_host = format!("smtp.{domain}");
    SetupRecommendation::Imap {
        // The showcase's canned detection stands in for a provider that names none: the
        // password field is the screen the documentation pictures.
        oauth_issuer: None,
        email: email.to_owned(),
        imap_host: imap_host.clone(),
        smtp_host: Some(smtp_host.clone()),
        imap_security: ConnectionSecurity::ImplicitTls,
        smtp_security: ConnectionSecurity::ImplicitTls,
        incoming: DetectedServerRow {
            protocol: "IMAP".to_owned(),
            hostname: imap_host,
            port: 993,
            security: "SSL/TLS".to_owned(),
            username: email.to_owned(),
        },
        outgoing: Some(DetectedServerRow {
            protocol: "SMTP".to_owned(),
            hostname: smtp_host,
            port: 465,
            security: "SSL/TLS".to_owned(),
            username: email.to_owned(),
        }),
        // Only the trusted domain publishes a calendar, so the guide can show the pre-checked
        // opt-out toggle on one screen and its absence on the other.
        caldav_url: trusted.then(|| format!("https://dav.{domain}/")),
        carddav_url: None,
        is_trusted: trusted,
        source: if trusted {
            format!("autoconfig (https://autoconfig.{domain}/mail/config-v1.1.xml)")
        } else {
            format!("autoconfig (http://autoconfig.{domain}/mail/config-v1.1.xml)")
        },
    }
}
