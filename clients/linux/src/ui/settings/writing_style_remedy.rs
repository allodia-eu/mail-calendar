//! What a writing-assistant remedy opens from Settings → Writing style (`docs/ai.md` rules 11 and
//! 27): the flow takes a remedy first, and one that leads elsewhere is then opened where it lives.

use mailcal_bindings::{AiRemedy, StyleLearningIntent};

use super::{Category, View};
use crate::ui::AppInput;

impl View {
    /// The remedy of a run that failed with its host closed. The flow takes it first, as it does
    /// in the host; one that is a step of the flow opens the host on that step.
    pub(super) fn run_remedy(&self, remedy: AiRemedy) {
        if matches!(remedy, AiRemedy::ChooseAccount | AiRemedy::ChooseRange) {
            self.learning_intent(StyleLearningIntent::Open);
        }
        self.learning_intent(StyleLearningIntent::Remedy);
        self.open_remedy(remedy);
    }

    /// Opens what a remedy names outside the flow: the Allodia sign-in, over the flow when it is
    /// up (which waits on its review meanwhile) and from its category when it is not, or the
    /// endpoint form under Advanced.
    pub(super) fn open_remedy(&self, remedy: AiRemedy) {
        match remedy {
            AiRemedy::SignInToAllodia => {
                if self.learning.borrow().is_none() {
                    self.ctx.sidebar.show(Category::Allodia);
                }
                self.ctx.sender.emit(AppInput::StartAllodiaSignIn);
            }
            AiRemedy::OpenAiEndpoint => self.ctx.sidebar.show(Category::Advanced),
            AiRemedy::TryAgain
            | AiRemedy::ChooseAccount
            | AiRemedy::ChooseRange
            | AiRemedy::LearnStyle
            | AiRemedy::ChooseStyle => {}
        }
    }
}
