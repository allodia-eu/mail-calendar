//! An account's browser sign-in, run by the core from the press to the account it changes
//! (`docs/waiting.md`, `docs/provider-oauth.md` rule 13): the server asked to start one on a
//! thread of its own, the question a switch would raise, the wait on the browser, a Nextcloud
//! server asked whether the person has finished, and the completion, queued as a change on the
//! page. A client opens the browser and hands back what it did; it keeps no step of its own.

use std::{sync::Arc, time::Duration};

use crate::{
    AccountCapability, AccountChangeNotice, AccountChangeRefusal, AccountPage, AccountPageIntent,
    AccountProvider, BrowserSignIn, BrowserSignInStage, CapabilityState, ControlState, MailcalApp,
    MailcalError, NextcloudSignInPoll, NoticeAction, SignInBrowserEnd,
    account_page::browser::{Began, Completion, Ended, Pending, Start},
    account_work::Change,
};

/// Which sign-in an account runs in its browser.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Way {
    /// A Microsoft or Google account's consent.
    Consent,
    /// A calendar and contacts account at a Nextcloud server.
    Nextcloud,
    /// A standards or JMAP account at its provider.
    Account,
}

#[uniffi::export]
impl MailcalApp {
    /// The browser sign-in out, or `None`. Read again after every `Surface::Settings` signal. A
    /// client opens the browser at `BrowserSignInStage::InBrowser`'s `url` once for each
    /// attempt, listens at its `redirect_uri`, and hands back what the browser did as
    /// `AccountPageIntent::SignInBrowserEnded`.
    #[must_use]
    pub fn browser_sign_in(&self) -> Option<BrowserSignIn> {
        self.account_work.browser.record()
    }
}

impl MailcalApp {
    /// Handles a step of the browser sign-in, which returns at once: whatever waits on a server
    /// runs on a thread of its own.
    pub(crate) fn dispatch_sign_in(&self, intent: AccountPageIntent) {
        let Some(app) = self.rediscovery.app() else {
            log::warn!("sign-in: a browser sign-in was asked for while the app shuts down");
            return;
        };
        let browser = &self.account_work.browser;
        match intent {
            AccountPageIntent::SignIn {
                account,
                adding,
                redirect_uri,
            } => app.start_sign_in(account, adding, redirect_uri),
            AccountPageIntent::AnswerUnreached {
                attempt, answer, ..
            } => {
                if let Some(serial) = browser.answered(attempt, answer) {
                    Self::cap_wait(app, attempt, serial);
                }
            }
            AccountPageIntent::SignInBrowserEnded {
                account,
                attempt,
                end,
            } => match end {
                SignInBrowserEnd::Redirected { callback_url } => {
                    if browser.returned(attempt, callback_url) {
                        self.queue_change(Change::SignIn { account, attempt });
                    }
                }
                // A sign-in that returns nothing may have been finished before the browser was
                // closed: its server is asked once more.
                SignInBrowserEnd::Closed if !browser.redirects(attempt) => {
                    std::thread::spawn(move || {
                        app.poll_once(attempt, true);
                    });
                }
                SignInBrowserEnd::Closed => {
                    browser.end(attempt);
                }
                // Said on a thread of its own: the notices it marks are read from the store.
                SignInBrowserEnd::NotOpened => {
                    if let Some(ended) = browser.end(attempt) {
                        std::thread::spawn(move || {
                            app.sign_in_ended(&ended, AccountChangeRefusal::BrowserNotOpened);
                            app.signal_sign_in();
                        });
                    }
                }
            },
            AccountPageIntent::CancelSignIn { attempt, .. } => {
                browser.end(attempt);
            }
            _ => return,
        }
        self.signal_sign_in();
    }

    /// Starts `account`'s sign-in in place of the one out, or completes it at once from the grant
    /// its last one kept.
    fn start_sign_in(
        self: Arc<Self>,
        account: String,
        adding: Vec<AccountCapability>,
        redirect_uri: String,
    ) {
        let Some((way, switching)) = self.way_of(&account) else {
            log::warn!("sign-in: a browser sign-in was asked for an account that is gone");
            return;
        };
        // A client with nowhere to listen for the browser's return sends no redirect: the
        // browser is not opened, and the page says so.
        if redirect_uri.is_empty() && way != Way::Nextcloud {
            log::warn!("sign-in: no redirect was given for the browser's return");
            std::thread::spawn(move || {
                let ended = Ended { account, switching };
                self.sign_in_ended(&ended, AccountChangeRefusal::BrowserNotOpened);
                self.signal_sign_in();
            });
            return;
        }
        match self
            .account_work
            .browser
            .start(&account, &adding, switching)
        {
            Start::Refused => {}
            Start::Complete { attempt } => self.queue_change(Change::SignIn { account, attempt }),
            Start::Begin { attempt, withdraw } => {
                // The page's next change has started, so what the last one said goes.
                self.account_work.unsay(&account);
                if let Some(grant) = withdraw {
                    self.abandon_nextcloud_sign_in(grant);
                }
                std::thread::spawn(move || {
                    self.begin_sign_in(attempt, &account, way, adding, redirect_uri)
                });
            }
        }
    }

    /// Which sign-in `account` runs, and whether it moves the account off a password.
    fn way_of(&self, account: &str) -> Option<(Way, bool)> {
        Some(match self.registry.provider(account)? {
            AccountProvider::Microsoft | AccountProvider::Google => (Way::Consent, false),
            provider => match self.calendar_account(account) {
                Some(config) => (Way::Nextcloud, config.nextcloud.is_none()),
                None => (
                    Way::Account,
                    matches!(provider, AccountProvider::Password | AccountProvider::Jmap),
                ),
            },
        })
    }

    /// Asks the server to start `attempt`, then waits on the browser: for its redirect, or by
    /// asking a Nextcloud server whether the person has finished.
    fn begin_sign_in(
        self: Arc<Self>,
        attempt: u64,
        account: &str,
        way: Way,
        adding: Vec<AccountCapability>,
        redirect_uri: String,
    ) {
        let began = match way {
            Way::Consent => self
                .begin_account_consent(account.to_owned(), redirect_uri.clone(), adding)
                .map(|start| Began {
                    pending: Pending::Consent(start.pending),
                    url: start.authorization_url,
                    redirect_uri: Some(redirect_uri),
                    unreached: Vec::new(),
                }),
            Way::Nextcloud => self
                .begin_account_nextcloud_sign_in(account.to_owned())
                .map(|start| Began {
                    pending: Pending::Nextcloud(start.pending),
                    url: start.login_url,
                    redirect_uri: None,
                    unreached: Vec::new(),
                }),
            // A grant signed in again returns where it was registered to, which may not be where
            // the client listens now: the authorization URL says.
            Way::Account => self
                .begin_account_sign_in(account.to_owned(), redirect_uri.clone())
                .map(|start| Began {
                    redirect_uri: Some(
                        redirect_of(&start.authorization_url).unwrap_or(redirect_uri),
                    ),
                    pending: Pending::Account(start.pending),
                    url: start.authorization_url,
                    unreached: start.unreached,
                }),
        };
        let browser = &self.account_work.browser;
        match began {
            Err(error) => {
                if let Some(ended) = browser.end(attempt) {
                    log::info!("sign-in: the server did not start a sign-in ({error})");
                    self.sign_in_ended(&ended, AccountChangeRefusal::from(&error));
                }
                self.signal_sign_in();
            }
            Ok(began) => {
                let asks_server = began.redirect_uri.is_none();
                let asking = !began.unreached.is_empty();
                let Some(serial) = browser.began(attempt, began) else {
                    return;
                };
                self.signal_sign_in();
                if asks_server {
                    self.poll_until_over(attempt);
                } else if !asking {
                    Self::cap_wait(self, attempt, serial);
                }
            }
        }
    }

    /// Ends `attempt` if it still waits on the browser at `serial` once its cap has passed: a
    /// browser closed without returning tells nobody.
    fn cap_wait(app: Arc<Self>, attempt: u64, serial: u64) {
        std::thread::spawn(move || {
            std::thread::sleep(app.account_work.browser.cap());
            if let Some(ended) = app.account_work.browser.timed_out(attempt, serial) {
                log::info!("sign-in: nothing came back from the browser in time");
                app.sign_in_ended(&ended, AccountChangeRefusal::NotFinished);
                app.signal_sign_in();
            }
        });
    }

    /// Asks `attempt`'s Nextcloud server whether the person has finished, at its interval, until
    /// the sign-in is over.
    fn poll_until_over(&self, attempt: u64) {
        let interval = Duration::from_millis(crate::nextcloud_poll_interval_ms());
        loop {
            std::thread::sleep(interval);
            if !self.poll_once(attempt, false) {
                return;
            }
        }
    }

    /// Asks `attempt`'s Nextcloud server once; `last` ends a sign-in still waiting, its browser
    /// closed. Answers whether to ask again.
    fn poll_once(&self, attempt: u64, last: bool) -> bool {
        let browser = &self.account_work.browser;
        let Some(pending) = browser.polling(attempt) else {
            return false;
        };
        let ended = match self.poll_nextcloud_sign_in(pending) {
            Ok(NextcloudSignInPoll::Waiting) if !last => return true,
            Ok(NextcloudSignInPoll::Waiting) => {
                browser.end(attempt);
                None
            }
            Ok(NextcloudSignInPoll::Expired) => browser
                .end(attempt)
                .map(|ended| (ended, AccountChangeRefusal::NotFinished)),
            Ok(NextcloudSignInPoll::Granted { grant }) => {
                match browser.account_of(attempt) {
                    Some(account) if browser.granted(attempt, grant.clone()) => {
                        self.queue_change(Change::SignIn { account, attempt });
                    }
                    // Over meanwhile: the password it carries becomes no account.
                    _ => self.abandon_nextcloud_sign_in(grant),
                }
                None
            }
            Err(error) => browser
                .end(attempt)
                .map(|ended| (ended, AccountChangeRefusal::from(&error))),
        };
        if let Some((ended, refusal)) = ended {
            self.sign_in_ended(&ended, refusal);
        }
        self.signal_sign_in();
        false
    }

    /// Completes `attempt`, as a change on the page: answers what needs saying.
    pub(crate) fn complete_sign_in(&self, attempt: u64) -> Option<AccountChangeNotice> {
        let browser = &self.account_work.browser;
        let run = browser.take(attempt)?;
        let account = run.account.clone();
        let (answer, kept) = match run.completion {
            Completion::Consent {
                pending,
                callback_url,
            } => (self.complete_account_consent(pending, callback_url), None),
            Completion::Account {
                pending,
                callback_url,
                unreached,
            } => (
                self.complete_account_sign_in(account.clone(), pending, callback_url, unreached),
                None,
            ),
            // The server hands a grant out once: kept for the next try unless it is spent, or
            // signs in as somebody else, whose password the completion already withdrew.
            Completion::Nextcloud { grant } => {
                let answer =
                    self.complete_account_nextcloud_sign_in(account.clone(), grant.clone());
                let spent = matches!(answer, Ok(()) | Err(MailcalError::DifferentAccount));
                (answer, (!spent).then_some(grant))
            }
        };
        browser.completed(attempt, kept);
        self.app.signal_notices();
        match answer {
            Ok(()) => self.withheld_after_sign_in(&account),
            Err(error) => {
                log::info!("sign-in: the browser sign-in did not complete ({error})");
                self.sign_in_remedy_failed(&account);
                Some(AccountChangeNotice::SignInFailed {
                    switching: run.switching,
                    refusal: AccountChangeRefusal::from(&error),
                })
            }
        }
    }

    /// The uses `account` is used for that its provider still withholds, said after a sign-in
    /// that worked.
    fn withheld_after_sign_in(&self, account: &str) -> Option<AccountChangeNotice> {
        let entry = self
            .accounts_snapshot()
            .accounts
            .into_iter()
            .find(|entry| entry.id == account)?;
        let uses: Vec<AccountCapability> = entry
            .uses
            .iter()
            .filter(|use_| use_.state == CapabilityState::NeedsPermission)
            .map(|use_| use_.capability)
            .collect();
        (!uses.is_empty()).then_some(AccountChangeNotice::SignedInWithheld { uses })
    }

    /// Says on its account's page why a sign-in ended before its completion, and on the notices
    /// whose remedy it was.
    fn sign_in_ended(&self, ended: &Ended, refusal: AccountChangeRefusal) {
        self.account_work.say(
            &ended.account,
            AccountChangeNotice::SignInFailed {
                switching: ended.switching,
                refusal,
            },
        );
        self.sign_in_remedy_failed(&ended.account);
    }

    /// Marks the remedy of every notice that signs `account` in again as failed, whichever
    /// surface started the sign-in: it did not answer them.
    fn sign_in_remedy_failed(&self, account: &str) {
        let shown = self.notices();
        let mut book = self.notice_book.lock().expect("notice-book mutex poisoned");
        let mut marked = false;
        for notice in shown.notices.iter().filter(|notice| {
            notice.account.as_deref() == Some(account)
                && notice.action == Some(NoticeAction::SignInAgain)
        }) {
            marked |= book.note_remedy_failed(&notice.id);
        }
        drop(book);
        if marked {
            self.app.signal_notices();
        }
    }

    /// The sign-in moved: the pages and the notices that draw it are read again.
    fn signal_sign_in(&self) {
        self.app.accounts_changed();
        self.app.signal_notices();
    }

    /// Draws `account`'s sign-in on its page: the control that started it running while the
    /// server starts or completes it, the password field held while it is out, and the section
    /// saying it waits on the browser.
    pub(crate) fn draw_browser_sign_in(&self, page: &mut AccountPage) {
        let Some((browser, adding)) = self.account_work.browser.of(&page.account) else {
            return;
        };
        let working = matches!(
            browser.stage,
            BrowserSignInStage::Starting | BrowserSignInStage::Completing
        );
        let running = ControlState {
            enabled: false,
            running: true,
        };
        if working {
            for switch in &mut page.uses {
                if adding.contains(&switch.capability) {
                    switch.control = running;
                }
            }
        }
        if let Some(section) = &mut page.sign_in {
            if working && adding.is_empty() {
                section.control = running;
            }
            if let Some(field) = &mut section.password {
                field.save.enabled = false;
            }
            section.browser = Some(browser);
        }
    }
}

/// The `redirect_uri` an authorization URL names.
fn redirect_of(authorization_url: &str) -> Option<String> {
    url::Url::parse(authorization_url)
        .ok()?
        .query_pairs()
        .find(|(key, _)| key == "redirect_uri")
        .map(|(_, value)| value.into_owned())
}

#[cfg(test)]
#[path = "app_account_page_browser_tests.rs"]
mod tests;
