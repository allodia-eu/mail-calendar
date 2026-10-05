//! A send the previous process left mid-attempt, put back by the constructor every native
//! client boots through: waiting again if it stopped before handing the message over, awaiting
//! the user's answer if it stopped after.
//!
//! Without the recovery the row would read as still being sent until its attempt expired,
//! minutes into the next run, which is what each assertion tells apart. A child of
//! [`super`], reusing its boot fakes.

use std::sync::Arc;

use engine_api::{Draft, EmailAddress, MessageIdHeader, PendingOpState};
use engine_provider::{HandOver, SubmissionReceipt};

use super::*;

/// A submission that stops for good, before or after recording its hand-over.
struct StuckSubmitProvider {
    caps: Capabilities,
    after_hand_over: bool,
    reached: Mutex<Option<oneshot::Sender<()>>>,
}

#[async_trait::async_trait]
impl Provider for StuckSubmitProvider {
    fn connection_info(&self) -> ConnectionInfo {
        ConnectionInfo::new(self.caps)
    }

    async fn submit_email(
        &self,
        _account: &AccountId,
        _draft: &Draft,
        hand_over: &HandOver<'_>,
    ) -> ProviderResult<SubmissionReceipt> {
        if self.after_hand_over {
            hand_over.commit().await?;
        }
        if let Some(reached) = self.reached.lock().expect("reached mutex poisoned").take() {
            let _ = reached.send(());
        }
        std::future::pending().await
    }
}

impl engine_api::MailboxWrites for StuckSubmitProvider {}
impl CalendarWrites for StuckSubmitProvider {}

/// Sends from a process that then ends, boots the app over its store, and answers what the
/// Outbox holds afterwards.
fn state_after_boot(name: &str, after_hand_over: bool) -> PendingOpState {
    let data_dir = temp_data_dir(name);
    fs::create_dir_all(&data_dir).expect("create temp data dir");
    let db = data_dir.join("mailcal.sqlite");
    let account = AccountId::try_from("boot-send@example.com").expect("valid account id");

    let runtime = tokio::runtime::Runtime::new().expect("runtime");
    let engine = Arc::new(Engine::open(&db).expect("open engine"));
    let (reached_tx, reached_rx) = oneshot::channel();
    let provider = Arc::new(StuckSubmitProvider {
        caps: Capabilities::none().with_submission(),
        after_hand_over,
        reached: Mutex::new(Some(reached_tx)),
    });
    let send = {
        let (engine, provider, account) =
            (Arc::clone(&engine), Arc::clone(&provider), account.clone());
        runtime.spawn(async move {
            let draft = Draft::new(
                MessageIdHeader::new("boot-send@example.com").expect("valid message id"),
                EmailAddress::new("boot-send@example.com"),
                vec![EmailAddress::new("you@example.com")],
                "Subject",
                "Body",
            );
            engine.submit_mail(&*provider, &account, &draft).await
        })
    };
    runtime
        .block_on(reached_rx)
        .expect("the send reached its stop");
    send.abort();
    let _ = runtime.block_on(send);
    drop(engine);
    drop(runtime);

    let app = MailcalApp::new_accounts(
        Box::new(NoopObserver),
        Box::new(NullLogger),
        LogLevel::Info,
        Vec::new(),
        data_dir.to_string_lossy().into_owned(),
        "Etc/UTC".to_owned(),
        crate::analytics::test_device(),
        Box::new(NullCredentialStore),
    )
    .expect("boot over an interrupted send");
    drop(app);

    let runtime = tokio::runtime::Runtime::new().expect("runtime");
    let engine = Engine::open(&db).expect("reopen engine");
    let rows = runtime
        .block_on(engine.outbox(&account))
        .expect("read the outbox after boot");
    assert_eq!(rows.len(), 1, "the message must still be in the Outbox");
    let _ = fs::remove_dir_all(data_dir);
    rows[0].state
}

#[test]
fn a_send_cut_off_before_its_hand_over_is_waiting_again_after_boot() {
    assert_eq!(
        state_after_boot("boot-send-before", false),
        PendingOpState::Pending
    );
}

#[test]
fn a_send_cut_off_after_its_hand_over_awaits_an_answer_after_boot() {
    assert_eq!(
        state_after_boot("boot-send-after", true),
        PendingOpState::NeedsConfirmation
    );
}
