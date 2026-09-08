//! Tests for rich mail submission and composer blob resolution.

use std::sync::{Arc, Mutex};

use engine_api::{AccountId, Draft, DraftAttachmentDisposition, EmailAddress, Engine, TimeZoneId};
use mailcal_composer::{
    AttachmentDisposition, AttachmentId, Block, ComposerDocument,
    DraftAttachment as ComposerAttachment, DraftBlobHandle, InlineContent, InlineImage, Paragraph,
    TextRun,
};

use super::{
    Account, App, AppObserver, ComposerBlob, Intent, SendStatus, Surface, Telemetry, TimeZoneInit,
};

// The submitting fake every test here drives, in its own file so this one stays under the
// 500-line limit. A child module, so it reaches the parent's imports through `super`.
#[path = "mail_ops_fake_provider.rs"]
mod fake;

use fake::SubmitProvider;

struct SilentObserver;

impl AppObserver for SilentObserver {
    fn surface_changed(&self, _surface: Surface) {}
}

/// Counts the [`Surface::Sending`] signals the app raises.
///
/// Every `set_send_status` publishes, and nothing else publishes that surface, so the count is
/// exactly how many times the send status changed. That makes it the one observable a send-timer
/// test can assert on **without catching a window**: which task runs when is scheduling, but every
/// publish has happened by the time both dispatches have finished, whatever order they ran in.
#[derive(Default)]
struct SendSignals {
    raised: std::sync::atomic::AtomicUsize,
}

impl SendSignals {
    fn record(&self) {
        self.raised
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    }

    fn count(&self) -> usize {
        self.raised.load(std::sync::atomic::Ordering::SeqCst)
    }
}

struct CountingObserver(Arc<SendSignals>);

impl AppObserver for CountingObserver {
    fn surface_changed(&self, surface: Surface) {
        if surface == Surface::Sending {
            self.0.record();
        }
    }
}

/// Builds a one-account app over a fresh `SubmitProvider`, returning the app (behind an
/// `Arc` so a send can be driven on a spawned task) and the provider's submission log.
fn submit_app() -> (Arc<App<SubmitProvider>>, Arc<Mutex<Vec<Draft>>>) {
    let engine = Engine::open_in_memory().unwrap();
    let provider = SubmitProvider::new();
    let submissions = provider.submissions();
    let app = App::new(
        engine,
        vec![Account {
            id: AccountId::try_from("acct-1").unwrap(),
            providers: vec![provider],
            calendar_providers: Vec::new(),
            contact_providers: Vec::new(),
            identity: EmailAddress::new("me@allodia.local"),
        }],
        TimeZoneInit {
            device_zone: TimeZoneId::utc(),
            prefs_path: None,
        },
        None,
        std::sync::Arc::new(SilentObserver),
        Telemetry::off(None),
    );
    (Arc::new(app), submissions)
}

/// The twin of [`submit_app`] whose observer counts send signals.
fn submit_app_counting() -> (Arc<App<SubmitProvider>>, Arc<SendSignals>) {
    let signals = Arc::new(SendSignals::default());
    let app = App::new(
        Engine::open_in_memory().unwrap(),
        vec![Account {
            id: AccountId::try_from("acct-1").unwrap(),
            providers: vec![SubmitProvider::new()],
            calendar_providers: Vec::new(),
            contact_providers: Vec::new(),
            identity: EmailAddress::new("me@allodia.local"),
        }],
        TimeZoneInit {
            device_zone: TimeZoneId::utc(),
            prefs_path: None,
        },
        None,
        Arc::new(CountingObserver(Arc::clone(&signals))),
        Telemetry::off(None),
    );
    (Arc::new(app), signals)
}

/// A one-account app over `provider`: the shared body of the builders around it.
fn app_over(provider: SubmitProvider) -> Arc<App<SubmitProvider>> {
    let app = App::new(
        Engine::open_in_memory().unwrap(),
        vec![Account {
            id: AccountId::try_from("acct-1").unwrap(),
            providers: vec![provider],
            calendar_providers: Vec::new(),
            contact_providers: Vec::new(),
            identity: EmailAddress::new("me@allodia.local"),
        }],
        TimeZoneInit {
            device_zone: TimeZoneId::utc(),
            prefs_path: None,
        },
        None,
        std::sync::Arc::new(SilentObserver),
        Telemetry::off(None),
    );
    Arc::new(app)
}

/// A one-account app whose provider's every send fails with a permanent error carrying `detail`;
/// drives the "a refused send raises the mail-reconnect prompt" path.
fn submit_app_failing(detail: &str) -> Arc<App<SubmitProvider>> {
    app_over(SubmitProvider::failing_with(detail))
}

/// Spawns `intent` as a fire-and-forget dispatch (mirroring the bindings' runtime) and
/// cooperatively drives it (under paused virtual time) until it has set its terminal
/// `target` status, finished the post-send `refresh_mail`, and **parked on the auto-clear
/// sleep**. Returns the join handle so the caller can advance virtual time and then drive
/// the in-line clear.
///
/// Reaching `target` only marks the terminal status; the dispatch still runs `refresh_mail`
/// before registering the sleep. We keep yielding past that to give it the chance.
///
/// ⚠️ **The yields are best-effort, not a guarantee**, and no number of them fixes that:
/// `refresh_mail` does store work over `spawn_blocking`, which `yield_now` reschedules around
/// rather than waits for. What callers *can* rely on is that `yield_now` never advances the
/// paused clock, so wherever the sleep registers, it registers at a virtual instant the caller
/// chose: a send started after an `advance` still gets the later deadline. What they must not
/// assume is that the task is **parked**: awaiting one that is not parks the runtime, and a
/// parked runtime under `start_paused` auto-advances the clock into the next timer.
async fn dispatch_until(
    app: &Arc<App<SubmitProvider>>,
    intent: Intent,
    target: SendStatus,
) -> tokio::task::JoinHandle<()> {
    use std::sync::atomic::Ordering;

    // Waiting for the status alone is wrong when a *previous* send has already left it at
    // `target`: the loop exits before this dispatch has run at all, and the caller then asserts
    // on the previous send's state. The generation counts every status change, so requiring two
    // of them (this send's `Sending`, then its terminal status) is what makes the wait this
    // dispatch's own. (A second send in the same test hit exactly this under load: the helper
    // returned on send #1's `Sent`, and the caller read `Sending` a moment later.)
    let generation = app.send_status_generation.load(Ordering::SeqCst);
    let task = tokio::spawn({
        let app = Arc::clone(app);
        async move {
            app.dispatch(intent).await;
        }
    });
    while app.send_status() != target
        || app.send_status_generation.load(Ordering::SeqCst) < generation + 2
    {
        tokio::task::yield_now().await;
    }
    // Flush refresh_mail's remaining await points so the task reaches and registers its
    // sleep. refresh_mail now syncs each account's folders concurrently (real in-memory
    // store work over `spawn_blocking`), so generously over-yield; yields never advance the
    // paused clock, so the only cost of extra iterations is letting that work drain before
    // the caller advances time.
    for _ in 0..2048 {
        tokio::task::yield_now().await;
    }
    task
}

fn plain_send() -> Intent {
    Intent::SubmitMail {
        to: "you@test.local".to_owned(),
        subject: "Hi".to_owned(),
        body: "Body".to_owned(),
    }
}

#[tokio::test(start_paused = true)]
async fn rich_submit_renders_composer_and_resolves_blob_bytes() {
    let (app, submissions) = submit_app();
    let inline_id = AttachmentId::new("inline-chart").unwrap();
    let file_id = AttachmentId::new("file-report").unwrap();
    let inline_blob = DraftBlobHandle::new("blob-inline").unwrap();
    let file_blob = DraftBlobHandle::new("blob-file").unwrap();
    let document = rich_document(
        inline_id.clone(),
        file_id,
        inline_blob.clone(),
        file_blob.clone(),
    );

    let intent = Intent::SubmitRichMail {
        from: None,
        to: "you@test.local, team@test.local".to_owned(),
        cc: "carol@test.local".to_owned(),
        bcc: "dave@test.local".to_owned(),
        subject: "Rich".to_owned(),
        document,
        blobs: vec![
            ComposerBlob::new(inline_blob, vec![1, 2, 3]),
            ComposerBlob::new(file_blob, b"PDF!".to_vec()),
        ],
    };
    let _task = dispatch_until(&app, intent, SendStatus::Sent).await;

    // A successful submit leaves the send-status hint at Sent (the submission was recorded
    // before the terminal status; the auto-clear is still pending behind its delay).
    assert_eq!(app.send_status(), SendStatus::Sent);

    let submissions = submissions.lock().unwrap();
    assert_eq!(submissions.len(), 1);
    let draft = &submissions[0];
    // The comma-separated To split into two recipients; Cc and Bcc carried through.
    let to: Vec<&str> = draft.to.iter().map(|a| a.email.as_str()).collect();
    assert_eq!(to, vec!["you@test.local", "team@test.local"]);
    assert_eq!(draft.cc.len(), 1);
    assert_eq!(draft.cc[0].email, "carol@test.local");
    assert_eq!(draft.bcc.len(), 1);
    assert_eq!(draft.bcc[0].email, "dave@test.local");
    assert_eq!(draft.subject, "Rich");
    assert_eq!(draft.text_body, "Hello [Chart]");
    let html = draft
        .html_body
        .as_deref()
        .expect("rich draft has an HTML body");
    assert!(html.starts_with("<!DOCTYPE html><html><head>"));
    assert!(html.ends_with("</body></html>"));
    assert!(html.contains(
        "<p><strong>Hello </strong><img src=\"cid:chart@test.local\" alt=\"Chart\" width=\"320\"></p>"
    ));
    assert_eq!(draft.attachments.len(), 2);
    assert_eq!(draft.attachments[0].content, vec![1, 2, 3]);
    match &draft.attachments[0].disposition {
        DraftAttachmentDisposition::Inline { content_id } => {
            assert_eq!(content_id.as_str(), "chart@test.local");
        }
        DraftAttachmentDisposition::Attachment => {
            panic!("expected inline attachment, got a regular attachment")
        }
    }
    assert_eq!(draft.attachments[1].content, b"PDF!".to_vec());
    assert!(matches!(
        draft.attachments[1].disposition,
        DraftAttachmentDisposition::Attachment
    ));
}

#[tokio::test(start_paused = true)]
async fn send_status_auto_clears_to_idle_after_delay() {
    let (app, _submissions) = submit_app();

    // The send sets Sending → Sent, then parks on the auto-clear sleep.
    let task = dispatch_until(&app, plain_send(), SendStatus::Sent).await;
    assert_eq!(app.send_status(), SendStatus::Sent);

    // Advance past the delay: the pending clear fires and resets to Idle. With its
    // generation still current, the guard lets it through.
    //
    // **Await the task rather than [`settle`], and the difference is a red CI.** The clear is not a
    // separate task: the sleep is awaited *inline* in the dispatch (see `mail_ops.rs`), so this
    // handle completing IS the clear having run. Waiting on it is exact.
    //
    // `settle` yields a fixed 64 times and hopes, which is not a synchronisation primitive: on a
    // loaded runner the task had not got there yet, and `main` went red on a test that passes
    // twenty times out of twenty on a quiet laptop.
    //
    // What `settle` exists to avoid; `.await` letting the paused clock auto-advance and fire
    // *other* pending timers; cannot happen here: this test has exactly one task and one
    // timer, and its deadline has already passed. (It is a real hazard in
    // `newer_send_survives_an_older_sends_pending_clear`, which juggles two overlapping deadlines,
    // and that is why this trick does not generalise to it.)
    tokio::time::advance(crate::mail_ops::AUTO_CLEAR_DELAY).await;
    task.await.unwrap();
    assert_eq!(app.send_status(), SendStatus::Idle);
}

#[tokio::test(start_paused = true)]
async fn a_send_refused_for_lack_of_permission_raises_the_mail_reconnect_prompt() {
    // The user's "can't send" scenario end to end: a Graph `403 ErrorAccessDenied` on `sendMail`
    // (the OAuth grant predates `Mail.Send`) must, besides failing the send, raise the account's
    // "reconnect to send and manage mail" prompt: the reactive equivalent of the calendar boot
    // probe. This drives the *real* `ApiError` the outbox returns (`Sync(Provider(..))`), proving
    // the structured `source()`-chain classification finds the nested provider error.
    let app = submit_app_failing(
        "Graph HTTP 403 (code Some(\"ErrorAccessDenied\")): {\"error\":{\"code\":\
         \"ErrorAccessDenied\",\"message\":\"Access is denied. Check credentials and try again.\"}}",
    );

    // The send sets Sending → Failed, then parks on the auto-clear sleep.
    let task = dispatch_until(&app, plain_send(), SendStatus::Failed).await;
    assert_eq!(app.send_status(), SendStatus::Failed);

    // The account is flagged for a mail re-consent; surfaced to the host as the reconnect banner.
    // (Independent of the transient send-status hint, which auto-clears; this permission gap
    // persists until a re-auth or a successful send.)
    assert_eq!(
        app.connectivity().mail_reauth_accounts,
        vec!["acct-1".to_string()],
        "a permission-denied send raises the reconnect-to-send prompt",
    );

    // Let the auto-clear timer fire so the spawned dispatch completes; the prompt outlives it.
    tokio::time::advance(crate::mail_ops::AUTO_CLEAR_DELAY).await;
    task.await.unwrap();
    assert_eq!(app.send_status(), SendStatus::Idle);
    assert_eq!(
        app.connectivity().mail_reauth_accounts,
        vec!["acct-1".to_string()],
        "the reconnect prompt persists past the send-status auto-clear",
    );
}

#[tokio::test(start_paused = true)]
async fn a_successful_send_clears_a_raised_mail_reconnect_prompt() {
    // The self-heal half of the re-consent contract (confirmed live, now pinned): once an account
    // is flagged for mail re-consent, a send that goes through proves the grant now carries
    // `Mail.Send`, so the prompt must drop: a user who reconnects (or whose transient refusal
    // resolves) must not be left staring at a stale "reconnect to send" banner. Guards the `Ok`
    // branch in `submit_through_outbox` that calls `clear_mail_reauth_required`.
    let (app, _submissions) = submit_app();
    let id = AccountId::try_from("acct-1").unwrap();

    // Flag it as if a prior send/edit had been refused for lack of permission.
    app.note_mail_reauth_required(&id);
    assert_eq!(
        app.connectivity().mail_reauth_accounts,
        vec!["acct-1".to_string()],
        "precondition: the account is awaiting mail re-consent",
    );

    // A send that succeeds clears the prompt before it reaches the terminal Sent status.
    let task = dispatch_until(&app, plain_send(), SendStatus::Sent).await;
    assert!(
        app.connectivity().mail_reauth_accounts.is_empty(),
        "a successful send clears the reconnect-to-send prompt (the grant plainly works)",
    );

    tokio::time::advance(crate::mail_ops::AUTO_CLEAR_DELAY).await;
    task.await.unwrap();
    assert_eq!(app.send_status(), SendStatus::Idle);
}

#[tokio::test(start_paused = true)]
async fn newer_send_survives_an_older_sends_pending_clear() {
    let (app, signals) = submit_app_counting();

    // Send #1 reaches Sent and parks on its clear (deadline = now + DELAY).
    let first = dispatch_until(&app, plain_send(), SendStatus::Sent).await;

    // Advance halfway, then start send #2; its clear deadline is later than #1's.
    tokio::time::advance(crate::mail_ops::AUTO_CLEAR_DELAY / 2).await;
    let second = dispatch_until(&app, plain_send(), SendStatus::Sent).await;
    assert_eq!(app.send_status(), SendStatus::Sent);

    // Advance past #1's deadline and let both dispatches finish. #1's timer fires with a stale
    // generation (#2 bumped it) so its guard must decline; #2's is current and clears.
    tokio::time::advance(crate::mail_ops::AUTO_CLEAR_DELAY / 2).await;
    first.await.unwrap();
    second.await.unwrap();
    assert_eq!(app.send_status(), SendStatus::Idle);

    // **The assertion that tests the guard.** The final status cannot do it; it is `Idle` either
    // way, and neither can reading the status inside the window between the two timers: which
    // task has been polled when is scheduling, so that reads as `Sent` even when the guard is
    // gone, and awaiting a task to force the issue parks the runtime, which under `start_paused`
    // auto-advances the clock into #2's timer and fails the test at random. (That is exactly what
    // this test used to do, and why it failed roughly half the time under the full parallel
    // suite, where `refresh_mail`'s `spawn_blocking` work is slow enough to leave a task
    // unparked.) The signal count sidesteps all of it: every status change publishes, so a stale
    // timer that reset anyway shows up as a sixth signal, and by the time both dispatches have
    // finished every publish has happened whatever order they ran in.
    assert_eq!(
        signals.count(),
        5,
        "expected Sending/Sent for #1 and Sending/Sent/Idle for #2: a sixth signal is #1's \
         stale timer clearing a status that is no longer its own",
    );
}

fn rich_document(
    inline_id: AttachmentId,
    file_id: AttachmentId,
    inline_blob: DraftBlobHandle,
    file_blob: DraftBlobHandle,
) -> ComposerDocument {
    ComposerDocument {
        blocks: vec![Block::Paragraph(Paragraph {
            content: vec![
                InlineContent::Text(TextRun {
                    text: "Hello ".to_owned(),
                    bold: true,
                    italic: false,
                    underline: false,
                    font_size: None,
                    color: None,
                    highlight: None,
                }),
                InlineContent::Image(InlineImage {
                    attachment_id: inline_id.clone(),
                    alt_text: "Chart".to_owned(),
                    width_px: Some(320),
                }),
            ],
        })],
        attachments: vec![
            ComposerAttachment {
                id: inline_id,
                blob: Some(inline_blob),
                file_name: "chart.png".to_owned(),
                media_type: "image/png".to_owned(),
                size: Some(3),
                disposition: AttachmentDisposition::Inline {
                    cid: mailcal_composer::ContentId::new("chart@test.local").unwrap(),
                },
                data_url: None,
            },
            ComposerAttachment {
                id: file_id,
                blob: Some(file_blob),
                file_name: "report.pdf".to_owned(),
                media_type: "application/pdf".to_owned(),
                size: Some(4),
                disposition: AttachmentDisposition::Attachment,
                data_url: None,
            },
        ],
    }
}

// The pasted-picture send test lives in its own file (each test module stays under the 500-line
// limit), as a child module reusing this module's `submit_app`/`dispatch_until` fixtures.
#[path = "mail_ops_paste_tests.rs"]
mod paste;

// Rich reply/forward send tests live in their own file (each test module stays under the
// 500-line limit), as a child module they reuse this module's `rich_document` and
// `SilentObserver` fixtures.
#[path = "mail_ops_reply_tests.rs"]
mod reply;

// The retry/dismiss half of the unfiled-Sent-copy question, in its own file for the same
// reason; it reuses this module's `SubmitProvider` and app builders.
#[path = "mail_ops_unfiled_tests.rs"]
mod unfiled;
