//! Settings → Writing style's intents (`docs/ai.md` rules 8 to 16), behind
//! `AiIntent::WritingStyle`.

use crate::StyleLearnRange;

/// A change on Settings → Writing style, in the learning flow, the reveal or the detail. Each
/// returns at once. A library change runs off the host's thread until written, its control running
/// meanwhile, and what it came to is the page's `outcome`; asked again while it runs, it is
/// dropped.
#[derive(uniffi::Enum)]
pub enum WritingStyleIntent {
    /// Moves the learning flow (`MailcalApp::writing_style_learning`).
    Learning { intent: StyleLearningIntent },
    /// Renames a style, from its detail. Refused with `BlankName` by
    /// `writing_style_name_refusal`'s rule.
    Rename { style: String, name: String },
    /// Replaces a style's notes, from its detail: sent on Return and on leaving the field.
    SetNotes { style: String, notes: String },
    /// Sets the style an account drafts in, or none, from the page's slot picker.
    Assign {
        account: String,
        style: Option<String>,
    },
    /// Forgets a style, once the detail's question was answered.
    Forget { style: String },
    /// Leaves the reveal by Save: the name and notes as typed, and the reveal shown.
    SaveReveal {
        style: String,
        name: String,
        notes: String,
    },
    /// Leaves the reveal without saving, which counts it as shown.
    CloseReveal { style: String },
}

/// What the person does in the learning flow. Each is drawn from a control on
/// `WritingStyleLearning` and acts only on the stage that offers it.
#[derive(uniffi::Enum)]
pub enum StyleLearningIntent {
    /// Opens the flow from the page's Learn, or joins the one under way.
    Open,
    /// Chooses the account on `ChooseAccount`, by its id.
    SelectAccount {
        account: String,
    },
    /// Chooses the range on `ChooseRange`, one the stage offers.
    SelectRange {
        range: StyleLearnRange,
    },
    Next,
    Back,
    /// The consent, on `Review`. `name` is the default name, worded by the client, that the style
    /// is stored under; `language` is the app's language as an ISO 639-1 code, which the
    /// description is written in.
    Learn {
        name: String,
        language: String,
    },
    Stop,
    /// The failure's remedy, on `Failed`, sent for every remedy, in the flow or on the page's run,
    /// and never with a `Close` of its own. `ChooseAccount`, `ChooseRange` and `TryAgain` move the
    /// flow; `SignInToAllodia` takes it back as Back does, to wait there while the client starts
    /// the sign-in; `OpenAiEndpoint` ends it, as `Close` does, and the client then opens Settings
    /// → Advanced (`docs/ai.md` rule 27).
    Remedy,
    /// Closing the host.
    Close,
}
