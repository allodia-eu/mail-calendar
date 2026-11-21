//! Settings → Advanced's record: the assistant access panel (`mcp`) and the reset beside it.

use crate::{AdvancedPage, ControlState, MailcalApp, ResetOutcome};

#[uniffi::export]
impl MailcalApp {
    /// Settings → Advanced (pulled after a `Surface::Settings` signal, which the reset starting
    /// and ending and the assistant server binding or refusing its endpoint all raise). Read from
    /// memory, so a UI thread may ask it.
    pub fn advanced_page(&self) -> AdvancedPage {
        let reset = self.app.reset_status();
        AdvancedPage {
            assistants: self.mcp_panel(),
            reset: ControlState {
                enabled: !reset.running,
                running: reset.running,
            },
            reset_outcome: reset.outcome.map(Into::into),
        }
    }
}

impl From<mailcal_viewmodel::ResetOutcome> for ResetOutcome {
    fn from(outcome: mailcal_viewmodel::ResetOutcome) -> Self {
        match outcome {
            mailcal_viewmodel::ResetOutcome::Done => Self::Done,
            mailcal_viewmodel::ResetOutcome::Failed => Self::Failed,
        }
    }
}

#[cfg(test)]
#[path = "tests_advanced.rs"]
mod tests;
