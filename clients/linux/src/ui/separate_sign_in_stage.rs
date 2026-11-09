//! Where a separate server's sign-in stands on its form: which attempt the form is on, and the
//! steps it may take from there, so a superseded or retired attempt cannot move it.

use crate::ui::separate_sign_in::SeparateForm;

/// Where a sign-in at the server's own provider stands, by the attempt it is for.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) enum SignInStage {
    #[default]
    Idle,
    /// The core is finding the provider and registering with it.
    Starting(u64),
    /// The browser holds the sign-in.
    Waiting(u64),
    /// The browser came back; the core is exchanging the code and adding the account.
    Finishing(u64),
}

impl SeparateForm {
    /// Whether every control is held: the core holds Connect (a change runs on a page), or one
    /// of the sign-in's blocking core calls is running. The browser's wait holds nothing.
    pub(in crate::ui) const fn held(&self) -> bool {
        !self.section.connect.enabled
            || matches!(
                self.stage,
                SignInStage::Starting(_) | SignInStage::Finishing(_)
            )
    }

    /// Starts `attempt`, superseding the one in the browser. `false` while there is nothing to
    /// start: no offer, or a core call running.
    pub(in crate::ui) fn begin_sign_in(&mut self, attempt: u64) -> bool {
        if self.offers().is_none() || self.held() {
            return false;
        }
        self.stage = SignInStage::Starting(attempt);
        self.error = None;
        true
    }

    /// The core prepared `attempt`. `true` when the browser should open for it: it is still
    /// the form's attempt and the core found the provider.
    pub(in crate::ui) fn sign_in_prepared(
        &mut self,
        attempt: u64,
        result: Result<(), String>,
    ) -> bool {
        if self.stage != SignInStage::Starting(attempt) {
            return false;
        }
        match result {
            Ok(()) => {
                self.stage = SignInStage::Waiting(attempt);
                true
            }
            Err(error) => {
                self.stage = SignInStage::Idle;
                self.failed(error);
                false
            }
        }
    }

    /// The browser came back for `attempt`. `true` when it is still the form's attempt, so the
    /// exchange may add the account.
    pub(in crate::ui) fn sign_in_redirected(&mut self, attempt: u64) -> bool {
        if self.stage != SignInStage::Waiting(attempt) {
            return false;
        }
        self.stage = SignInStage::Finishing(attempt);
        true
    }

    /// `attempt` ended: back to the offer. `false` when the form has moved on from it.
    pub(in crate::ui) fn sign_in_ended(&mut self, attempt: u64) -> bool {
        let ours = matches!(
            self.stage,
            SignInStage::Starting(id) | SignInStage::Waiting(id) | SignInStage::Finishing(id)
                if id == attempt
        );
        if ours {
            self.stage = SignInStage::Idle;
        }
        ours
    }

    /// Takes the form off an attempt not yet exchanging. `true` when there was one, whose slot
    /// the caller releases. An exchange already running is a core call, and finishes.
    pub(in crate::ui) fn retire_sign_in(&mut self) -> bool {
        let retired = matches!(
            self.stage,
            SignInStage::Starting(_) | SignInStage::Waiting(_)
        );
        if retired {
            self.stage = SignInStage::Idle;
        }
        retired
    }
}
