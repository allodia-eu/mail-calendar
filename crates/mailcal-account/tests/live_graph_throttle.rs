//! Gated live check that the **product** Graph path syncs a large mailbox's folders without
//! being throttled: the end-to-end version of allodia-eu/email-calendar-sync-engine#216.
//!
//! This exists because neither side alone can catch the bug it pins. The engine's live suite
//! builds its own gate and proves the ceiling holds; it cannot prove that *this host* hands
//! every one of an account's folder providers the same one. The host's unit tests prove the
//! registry returns one gate per account; they cannot prove a real mailbox is satisfied. The
//! failure lived exactly in the join: a bound that was correct (4) attached to the wrong
//! thing (a field on `GraphTokenSource`, of which a host builds one per core).
//!
//! What it drives is the shape the app actually produces: `connect_graph_mail_providers`
//! binds one `RefreshingGraphProvider` per eager role folder, six of them, and the engine
//! then streams them **five at a time** (`MAX_CONCURRENT_FOLDERS`). Five folder syncs against
//! a mailbox Exchange Online allows four concurrent requests is the reported symptom, so this
//! runs at that width deliberately rather than at a comfortable one.
//!
//! **Where the control arm is.** The paired proof that four is clean and more is not (one
//! gate versus a gate per provider, against this same mailbox) lives in the engine, in
//! `provider-graph/tests/live_throttle_gate.rs`, because forcing two gates onto one account is
//! precisely what this host's registry now refuses to do. So this file does not re-derive that
//! the ceiling is real; it proves the app's own connect path puts every folder provider under
//! **one** ceiling, and that a real pass at the engine's fan-out is then clean. The assertions
//! below are written so a quiet mailbox or a mis-wired gate fails rather than passes silently.
//!
//! It **skips** unless `MS_REFRESH_TOKEN`, `MS_CLIENT_ID` and `MS_TEST_ADDRESS` are set, so
//! the offline `cargo test` stays green; there is no CI harness (no live Microsoft account in
//! CI). It is **read-only**: folder lists and message deltas, no write of any kind.
//!
//! Run locally against a large mailbox. The throwaway `outlook.com` account the engine's
//! fixtures come from has eight folders and cannot reproduce anything here:
//! ```sh
//! T=<engine checkout>/tools/graph-oauth/.local/tokens-m365.json
//! MS_TEST_ADDRESS=<the account's own address> \
//! MS_CLIENT_ID=$(python3 -c "import json;print(json.load(open('$T'))['client_id'])") \
//! MS_REFRESH_TOKEN=$(python3 -c "import json;print(json.load(open('$T'))['refresh_token'])") \
//!   cargo test -p mailcal-account --test live_graph_throttle -- --nocapture
//! ```

use std::sync::{
    Arc, Mutex, OnceLock,
    atomic::{AtomicUsize, Ordering},
};

use engine_core::{ids::AccountId, sync::SyncWindow, time::CalendarDate};
use engine_provider::{PassMode, Provider};
use futures::StreamExt;
use mailcal_account::{
    CredentialOrigin, GraphTokenSource, MicrosoftConfig, Secret, connect_graph_mail_providers,
};

/// The engine's own per-account folder fan-out (`engine_sync`'s `MAX_CONCURRENT_FOLDERS`),
/// restated here because that is the width this test has to reproduce and the engine does not
/// export it: it is a store figure, not a knob.
const ENGINE_FOLDER_FANOUT: usize = 5;

/// How far back each folder syncs. Wide enough that the pass is real work on an active
/// mailbox, narrow enough to finish in seconds. An unwindowed `Archiveren` of 22,704
/// messages took **twelve minutes**, which is how a live test stops being run.
const WINDOW_DAYS: i64 = 90;

/// The fewest messages this pass must move before its clean result means anything.
///
/// Without a floor, a mailbox that happened to be quiet would report "no throttles" for the
/// same reason an unplugged network would. This is not a throughput assertion; it is the
/// guard that stops an absence from being vacuous.
const MIN_MESSAGES: usize = 50;

/// Counts the throttle lines the host's own `ThrottleObserver` writes.
///
/// The observer logs rather than returning anything (`mailcal_account::throttle`), so the only
/// seam from out here is the log. A process-global logger is normally the wrong tool; in a
/// dedicated integration-test binary it is the whole process, and it is what lets this assert
/// on the *product* wiring rather than on a stand-in.
struct CountThrottleLines;

static THROTTLES: AtomicUsize = AtomicUsize::new(0);
static LINES: OnceLock<Mutex<Vec<String>>> = OnceLock::new();

fn lines() -> &'static Mutex<Vec<String>> {
    LINES.get_or_init(|| Mutex::new(Vec::new()))
}

impl log::Log for CountThrottleLines {
    fn enabled(&self, _: &log::Metadata<'_>) -> bool {
        true
    }

    fn log(&self, record: &log::Record<'_>) {
        let line = record.args().to_string();
        // The wording `throttle::describe` produces, both the absorbed and gave-up forms.
        if line.contains("limiting how fast we can fetch") || line.contains("still limiting us") {
            THROTTLES.fetch_add(1, Ordering::SeqCst);
            lines().lock().expect("line log poisoned").push(line);
        }
    }

    fn flush(&self) {}
}

/// A recent-mail floor, so each folder is a handful of pages rather than a full drain.
///
/// Windowed rather than cut short after N chunks: breaking out of the stream early ends the
/// folder's requests at a point that has nothing to do with the mailbox, and a Graph delta
/// hands whole message objects back inline, so an unwindowed `Archiveren` of 22,704 messages
/// is 450 pages. That measured Graph's page *rate* for twelve minutes and told us nothing more
/// about its concurrency than fifteen seconds does, and a live test nobody re-runs is one
/// that rots.
fn window() -> SyncWindow {
    let today = time::OffsetDateTime::now_utc().date();
    let floor = today.saturating_sub(time::Duration::days(WINDOW_DAYS));
    SyncWindow::since(
        CalendarDate::new(floor.year(), u8::from(floor.month()), floor.day())
            .expect("a real calendar date"),
    )
}

/// A Microsoft config built from the environment, or `None` to skip the gated test.
fn config() -> Option<MicrosoftConfig> {
    let refresh_token = std::env::var("MS_REFRESH_TOKEN")
        .ok()
        .filter(|t| !t.is_empty())?;
    let client_id = std::env::var("MS_CLIENT_ID")
        .ok()
        .filter(|t| !t.is_empty())?;
    let email = std::env::var("MS_TEST_ADDRESS")
        .ok()
        .filter(|t| !t.is_empty())?;
    Some(MicrosoftConfig {
        email,
        client_id,
        tenant: "common".to_owned(),
        redirect_uri: "http://localhost".to_owned(),
        scopes: vec![
            "offline_access".to_owned(),
            "User.Read".to_owned(),
            "Mail.Read".to_owned(),
        ],
        refresh_token: Secret::new(refresh_token),
    })
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn live_the_eager_folder_pass_is_not_throttled_on_a_large_mailbox() {
    let Some(config) = config() else {
        eprintln!(
            "skipping live_the_eager_folder_pass_is_not_throttled_on_a_large_mailbox: \
             MS_REFRESH_TOKEN / MS_CLIENT_ID / MS_TEST_ADDRESS unset"
        );
        return;
    };
    log::set_logger(&CountThrottleLines).expect("no other logger in this test binary");
    log::set_max_level(log::LevelFilter::Trace);

    let account: AccountId = config.account_id().expect("account id");
    let tokens = GraphTokenSource::new(&config, account.clone(), None, CredentialOrigin::Stored)
        .expect("token source");

    // Exactly what the app connects at startup: one provider per eager role folder.
    let providers = connect_graph_mail_providers(&account, Arc::clone(&tokens), None)
        .await
        .expect("connect the account's mail providers");
    assert!(
        providers.len() > 1,
        "a single-folder account proves nothing about a fan-out",
    );
    eprintln!("connected {} eager folder providers", providers.len());

    // The wiring, asserted directly rather than inferred from the absence of throttles: the
    // account's own gate, the one `GraphTokenSource::retry` hands every provider above, has
    // been narrowed by the adapter to what Exchange Online allows one mailbox. A provider that
    // had built a gate of its own would leave this `None`.
    assert_eq!(
        tokens.retry().gate().limit(),
        Some(4),
        "the account's gate was never narrowed, so these providers are not sharing a ceiling",
    );

    // The engine's fan-out, driven here because `Engine::sync_mail` would need a store and
    // this is about the network, not the apply.
    let account = &account;
    let messages = &AtomicUsize::new(0);
    let failures = futures::stream::iter(providers.iter().map(|provider| async move {
        let mut stream = provider.stream_email(account, None, window(), 50, 200);
        while let Some(chunk) = stream.next().await {
            match chunk {
                Ok(chunk) => {
                    messages.fetch_add(chunk.changed.len() + chunk.patched.len(), Ordering::SeqCst);
                }
                Err(err) => return Err(format!("{:?}: {err}", provider.email_scope(account))),
            }
        }
        Ok(())
    }))
    .buffer_unordered(ENGINE_FOLDER_FANOUT)
    .filter_map(|result: Result<(), String>| async move { result.err() })
    .collect::<Vec<_>>()
    .await;

    assert!(failures.is_empty(), "folder syncs failed: {failures:?}");
    let synced = messages.load(Ordering::SeqCst);
    eprintln!(
        "synced {synced} messages across {} folders",
        providers.len()
    );
    assert!(
        synced >= MIN_MESSAGES,
        "only {synced} messages moved in the last {WINDOW_DAYS} days, which is too little for \
         a clean run to mean anything; widen the window or use a busier mailbox",
    );
    let throttled = THROTTLES.load(Ordering::SeqCst);
    assert_eq!(
        throttled,
        0,
        "Graph refused {throttled} request(s) the account gate should have held back: {:?}",
        lines().lock().expect("line log poisoned"),
    );
    // `PassMode` is named so this file fails to compile if the streaming contract moves under
    // it, rather than silently testing a different pass.
    let _: PassMode = PassMode::Additive;
}
