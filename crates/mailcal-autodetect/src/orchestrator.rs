//! The priority orchestrator, Thunderbird's `PriorityParallelRunner` semantics in one
//! `detect()` call.
//!
//! All strategies start at once. Their results are consumed **in priority order**, so a
//! lower-priority success is never taken before every higher-priority strategy has
//! finished (a slower-but-higher-priority provider wins over a faster lower one). The
//! first success cancels the rest; a whole-run deadline caps the wait. "Nothing found"
//! is reported as offline only when *every* strategy failed on transport.

use std::sync::Arc;

use tokio::task::JoinHandle;

use crate::{
    DetectConfig, caldav,
    fetch::{Fetch, Fetcher},
    jmap_probe, mx, srv, strategy,
    strategy::StrategyOutcome,
    types::{DetectError, Detected, EmailParts},
};

/// Detects mail-server settings for `email`. `resolver` enables the MX fallback (the
/// host's native DNS); `None` skips it. Errors only before the run starts (an invalid
/// email or a TLS build failure); every in-run miss folds into [`Detected::Nothing`].
///
/// # Errors
///
/// Returns [`DetectError::InvalidEmail`] for an unparseable address, or
/// [`DetectError::Tls`] if the HTTP client cannot be built.
pub async fn detect(
    email: &str,
    resolver: Option<Arc<dyn mx::MxResolver>>,
    config: &DetectConfig,
) -> Result<Detected, DetectError> {
    let email = EmailParts::parse(email).ok_or(DetectError::InvalidEmail)?;
    let fetcher: Arc<dyn Fetch> = Arc::new(Fetcher::new(config)?);
    Ok(orchestrate(fetcher, email, resolver, config.clone()).await)
}

/// The runtime core, split out so tests drive it with a fake fetcher under paused time.
pub(crate) async fn orchestrate(
    fetcher: Arc<dyn Fetch>,
    email: EmailParts,
    resolver: Option<Arc<dyn mx::MxResolver>>,
    config: DetectConfig,
) -> Detected {
    let deadline = config.overall_deadline;
    let handles = spawn_all(&fetcher, &email, resolver, &config);
    let aborts: Vec<_> = handles.iter().map(JoinHandle::abort_handle).collect();

    let detected = match tokio::time::timeout(deadline, collect(handles)).await {
        Ok(detected) => detected,
        Err(_elapsed) => {
            for abort in &aborts {
                abort.abort();
            }
            log::debug!("autodetect reached the overall deadline");
            Detected::Nothing {
                network_error: false,
            }
        }
    };
    let detected = with_caldav(fetcher.as_ref(), &email, &config, detected).await;
    log_outcome(&detected);
    detected
}

/// Logs the run's outcome at **info**: the winning strategy, the outcome kind, and whether
/// it was trusted, and nothing more. The domain and any URL stay at debug (rule 2 / the
/// privacy section), so a detection is visible in the diagnostic log without disclosing what
/// was looked up.
fn log_outcome(detected: &Detected) {
    let trust = |trusted: bool| if trusted { "trusted" } else { "untrusted" };
    match detected {
        Detected::Jmap(jmap) => {
            log::info!(
                "autodetect: jmap via {:?} ({})",
                jmap.source.kind,
                trust(jmap.is_trusted)
            );
        }
        Detected::Mail(mail) => {
            log::info!(
                "autodetect: mail via {:?} ({}){}",
                mail.source.kind,
                trust(mail.is_trusted),
                if mail.caldav_url.is_some() {
                    " +caldav"
                } else {
                    ""
                }
            );
        }
        Detected::Nothing { network_error } => {
            log::info!(
                "autodetect: nothing found ({})",
                if *network_error {
                    "offline"
                } else {
                    "none advertised"
                }
            );
        }
    }
}

/// A found IMAP config gets a follow-on RFC 6764 CalDAV probe; autoconfig/ISPDB describe
/// mail only, so the calendar endpoint (when there is one) is discovered separately. Every
/// other outcome passes through untouched. The probe is **soft**: bounded by its own
/// timeout and outside the overall deadline, it never turns a found config into a miss, so
/// a slow or absent calendar host never costs the user their mail settings.
async fn with_caldav(
    fetcher: &dyn Fetch,
    email: &EmailParts,
    config: &DetectConfig,
    detected: Detected,
) -> Detected {
    let Detected::Mail(mut settings) = detected else {
        return detected;
    };
    settings.caldav_url = caldav::probe(fetcher, email, &settings, config.http_timeout).await;
    Detected::Mail(settings)
}

/// Spawns every strategy at once, in priority order: JMAP probe (apex + `_jmap._tcp`
/// SRV), autoconfig, ISPDB, then (only when a resolver is supplied) the IMAP/SMTP SRV
/// strategy and the MX fallback.
fn spawn_all(
    fetcher: &Arc<dyn Fetch>,
    email: &EmailParts,
    resolver: Option<Arc<dyn mx::MxResolver>>,
    config: &DetectConfig,
) -> Vec<JoinHandle<StrategyOutcome>> {
    let jmap = {
        let (fetcher, email, config, resolver) = (
            fetcher.clone(),
            email.clone(),
            config.clone(),
            resolver.clone(),
        );
        tokio::spawn(async move {
            jmap_probe::run(fetcher.as_ref(), &email, resolver.as_ref(), &config).await
        })
    };
    let autoconfig = {
        let (fetcher, email) = (fetcher.clone(), email.clone());
        tokio::spawn(async move { strategy::run_autoconfig(fetcher.as_ref(), &email).await })
    };
    let ispdb = {
        let (fetcher, email) = (fetcher.clone(), email.clone());
        tokio::spawn(async move { strategy::run_ispdb(fetcher.as_ref(), &email).await })
    };
    let mut handles = vec![jmap, autoconfig, ispdb];
    // The two DNS-derived strategies need the host resolver: IMAP/SMTP SRV (priority 3,
    // consulted before MX) then the MX fallback (priority 4).
    if let Some(resolver) = resolver {
        handles.push(tokio::spawn(srv::run(
            email.clone(),
            resolver.clone(),
            config.clone(),
        )));
        handles.push(tokio::spawn(mx::run(
            fetcher.clone(),
            email.clone(),
            resolver,
            config.clone(),
        )));
    }
    handles
}

/// Consumes results in priority order. The first [`StrategyOutcome::Jmap`]/`Mail` wins
/// and aborts the strategies not yet consulted; awaiting each handle before the next is
/// what makes a lower-priority success wait for the higher-priority ones.
async fn collect(handles: Vec<JoinHandle<StrategyOutcome>>) -> Detected {
    let mut pending: Vec<Option<JoinHandle<StrategyOutcome>>> =
        handles.into_iter().map(Some).collect();
    let mut all_network = true;

    for index in 0..pending.len() {
        let handle = pending[index]
            .take()
            .expect("each handle is taken exactly once");
        match handle.await {
            Ok(StrategyOutcome::Jmap(jmap)) => {
                abort_pending(&pending);
                return Detected::Jmap(jmap);
            }
            Ok(StrategyOutcome::Mail(mail)) => {
                abort_pending(&pending);
                return Detected::Mail(mail);
            }
            Ok(StrategyOutcome::NetworkError) => {}
            // A clean miss (or a panicked/aborted task) means we are not fully offline.
            Ok(StrategyOutcome::Nothing) | Err(_) => all_network = false,
        }
    }

    Detected::Nothing {
        network_error: all_network,
    }
}

/// Aborts every strategy still waiting to be consulted (the lower-priority losers).
fn abort_pending(pending: &[Option<JoinHandle<StrategyOutcome>>]) {
    for handle in pending.iter().flatten() {
        handle.abort();
    }
}

#[cfg(test)]
#[path = "orchestrator_tests.rs"]
mod orchestrator_tests;
