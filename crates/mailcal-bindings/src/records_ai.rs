//! The writing assistant's records (`docs/ai.md`): what every surface of it shares (where a
//! request may reach, what can go wrong and what that offers), and the own endpoint's form. A
//! client draws these and words them; it decides none of what they say.

use mailcal_app::ai as core;

use crate::ControlState;

/// A mode that refuses something: every mode but the one admitting anything, which refuses
/// nothing. Read, never changed: no client offers to change the mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum JurisdictionLimit {
    /// Anywhere that processes in the EU, whoever runs it.
    EuHosted,
    /// Only what processes in the EU and is run from the EU.
    EuNative,
}

/// Why the gate refuses the own endpoint as declared: the class it does not admit, with the mode
/// in force where more than one refuses it. Only a refusal the gate can give is expressible: no
/// mode refuses an endpoint declared EU-native.
#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum JurisdictionRefusal {
    /// Declared as run in the EU by a company based outside it, which only `EuNative` refuses.
    EuHosted,
    /// Declared as run outside the EU.
    NonEu { mode: JurisdictionLimit },
    /// Not declared.
    Unknown { mode: JurisdictionLimit },
}

/// Where an own endpoint runs, as the person declared it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum JurisdictionClass {
    /// In the EU, by a company based in the EU, or on the person's own computer.
    EuNative,
    /// In the EU, by a company based outside it.
    EuHosted,
    /// Outside the EU.
    NonEu,
    /// Not said.
    Unknown,
}

/// Why learning or drafting did not happen (`docs/ai.md` rule 27). The client words it, and never
/// shows a server's sentence. Stopping is not a failure.
#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum AiFailure {
    /// The relay refused the Allodia sign-in, or it lacks the assistant's permission.
    RelaySignIn,
    /// The own endpoint refused its key.
    EndpointKeyRefused,
    /// The gate refuses the own endpoint as declared, under the mode in force.
    Refused { refusal: JurisdictionRefusal },
    /// No own endpoint is set up and the relay cannot be asked.
    NowhereToSend,
    /// The destination could not be reached, or did not answer in time.
    Unreachable,
    /// The destination answered with an error.
    ServerError,
    /// The destination asked for fewer requests.
    TooManyRequests,
    /// An answer arrived that could not be read.
    Unreadable,
    /// No credits are left.
    OutOfCredits,
    /// The plan no longer includes the assistant.
    NotEntitled,
    /// The account has no Sent folder.
    NoSentFolder { another_account: bool },
    /// Nothing in the range says enough to learn from.
    NothingToLearn,
    /// The sample was too large to send.
    SampleTooLarge,
    /// No style has been learned yet.
    NoStyle,
    /// This account drafts in no style.
    NoStyleForAccount,
    /// The message answered is no longer on this device.
    MessageGone,
}

/// What a failure offers: one action, drawn as one control.
#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum AiRemedy {
    SignInToAllodia,
    OpenAiEndpoint,
    TryAgain,
    ChooseAccount,
    ChooseRange,
    LearnStyle,
    ChooseStyle,
}

/// A failure's remedy as a surface draws it: which one, and its control, held while the surface
/// it sits on holds its controls. It never runs: what it starts is drawn where it leads.
#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Record)]
pub struct AiRemedyControl {
    pub remedy: AiRemedy,
    pub control: ControlState,
}

impl From<core::RemedyControl> for AiRemedyControl {
    fn from(remedy: core::RemedyControl) -> Self {
        Self {
            remedy: remedy.remedy.into(),
            control: ControlState {
                enabled: remedy.enabled,
                running: false,
            },
        }
    }
}

/// The own endpoint's form under Settings → Advanced (`docs/ai.md` rules 6 and 7), a record an
/// Advanced page embeds. Pulled after `Surface::AiEndpoint` and `Surface::Settings`.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct AiEndpointForm {
    /// The address: from the edit being saved, then from an edit a save refused, otherwise as
    /// stored. Empty with nothing set up.
    pub address: String,
    /// The model, drawn as the address is.
    pub model: String,
    /// Where it runs, drawn as the address is; `None` until declared.
    pub declared: Option<JurisdictionClass>,
    /// Whether a key is kept, which Remove takes out. The key is never drawn back: the field stays
    /// empty, and what it says about the key kept is `AiEndpointGaps::kept_key`, asked of the
    /// address as typed, since a save to another server takes the key out.
    pub key_kept: bool,
    /// Save: running while a save runs. A client holds it besides while
    /// [`crate::MailcalApp::ai_endpoint_gap`], which it asks of the fields as typed and says
    /// beside each field, names a gap.
    pub save: ControlState,
    /// Remove: `None` while there is nothing to remove, which is when neither an endpoint nor a
    /// key is stored; running while a removal runs.
    pub remove: Option<ControlState>,
    /// What the last change came to, until the next starts.
    pub outcome: Option<AiEndpointOutcome>,
}

/// What is wrong with the address.
#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum AiEndpointAddressGap {
    /// None was given.
    Missing,
    /// It is not a usable address.
    Unusable,
    /// It needs HTTPS, being to somewhere other than this device.
    NeedsHttps,
}

/// What is wrong with the declaration of where the endpoint runs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum AiEndpointDeclarationGap {
    /// Nothing was declared.
    Missing,
    /// The gate refuses what was declared under the mode in force.
    Refused { refusal: JurisdictionRefusal },
}

/// What a save with the key field empty does to the key kept (`docs/ai.md` rule 7).
#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum AiEndpointKeptKey {
    /// It stays: the address as typed is on the stored endpoint's server.
    Carried,
    /// It is taken out: the address as typed is on another server, or none is stored. The key
    /// field says none is kept for this address.
    Dropped,
}

/// Every field's problem at once; a field without one has none here. Beside them, what the save
/// would do to the key kept, which refuses nothing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Record)]
pub struct AiEndpointGaps {
    pub address: Option<AiEndpointAddressGap>,
    pub model_missing: bool,
    pub declaration: Option<AiEndpointDeclarationGap>,
    /// What a save with the key field empty does to the key kept, which the key field's hint
    /// says; `None` while no key is kept.
    pub kept_key: Option<AiEndpointKeptKey>,
}

/// Which change to the own endpoint.
#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum AiEndpointChangeKind {
    Save,
    Remove,
}

/// What a change to the own endpoint came to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum AiEndpointResult {
    /// Saved: the key in the keystore and the rest in the preferences.
    Saved,
    /// Removed, the key with it.
    Removed,
    /// Not saved, for these reasons; the form keeps the edit.
    Refused { gaps: AiEndpointGaps },
    /// The keystore refused the key, so nothing changed; the form says so and keeps the edit.
    KeyNotStored,
    /// Made, and the preferences could not be written, so it holds until the app closes. The
    /// failed-save notice says so.
    NotWritten,
}

/// The last change and what it came to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Record)]
pub struct AiEndpointOutcome {
    /// Grows with each change, so a client acts on each outcome once.
    pub serial: u64,
    pub kind: AiEndpointChangeKind,
    pub result: AiEndpointResult,
}

impl From<core::Refused> for JurisdictionRefusal {
    fn from(refused: core::Refused) -> Self {
        let limit = |mode| match mode {
            core::Limit::EuHosted => JurisdictionLimit::EuHosted,
            core::Limit::EuNative => JurisdictionLimit::EuNative,
        };
        match refused {
            core::Refused::EuHosted => Self::EuHosted,
            core::Refused::NonEu { mode } => Self::NonEu { mode: limit(mode) },
            core::Refused::Unknown { mode } => Self::Unknown { mode: limit(mode) },
        }
    }
}

impl From<core::Class> for JurisdictionClass {
    fn from(class: core::Class) -> Self {
        match class {
            core::Class::EuNative => Self::EuNative,
            core::Class::EuHosted => Self::EuHosted,
            core::Class::NonEu => Self::NonEu,
            core::Class::Unknown => Self::Unknown,
        }
    }
}

impl From<JurisdictionClass> for core::Class {
    fn from(class: JurisdictionClass) -> Self {
        match class {
            JurisdictionClass::EuNative => Self::EuNative,
            JurisdictionClass::EuHosted => Self::EuHosted,
            JurisdictionClass::NonEu => Self::NonEu,
            JurisdictionClass::Unknown => Self::Unknown,
        }
    }
}

impl From<core::AiFailure> for AiFailure {
    fn from(failure: core::AiFailure) -> Self {
        use core::AiFailure as F;
        match failure {
            F::RelaySignIn => Self::RelaySignIn,
            F::EndpointKeyRefused => Self::EndpointKeyRefused,
            F::Refused(refused) => Self::Refused {
                refusal: refused.into(),
            },
            F::NowhereToSend => Self::NowhereToSend,
            F::Unreachable => Self::Unreachable,
            F::ServerError => Self::ServerError,
            F::TooManyRequests => Self::TooManyRequests,
            F::Unreadable => Self::Unreadable,
            F::OutOfCredits => Self::OutOfCredits,
            F::NotEntitled => Self::NotEntitled,
            F::NoSentFolder { another_account } => Self::NoSentFolder { another_account },
            F::NothingToLearn => Self::NothingToLearn,
            F::SampleTooLarge => Self::SampleTooLarge,
            F::NoStyle => Self::NoStyle,
            F::NoStyleForAccount => Self::NoStyleForAccount,
            F::MessageGone => Self::MessageGone,
        }
    }
}

impl From<core::AiRemedy> for AiRemedy {
    fn from(remedy: core::AiRemedy) -> Self {
        use core::AiRemedy as R;
        match remedy {
            R::SignInToAllodia => Self::SignInToAllodia,
            R::OpenAiEndpoint => Self::OpenAiEndpoint,
            R::TryAgain => Self::TryAgain,
            R::ChooseAccount => Self::ChooseAccount,
            R::ChooseRange => Self::ChooseRange,
            R::LearnStyle => Self::LearnStyle,
            R::ChooseStyle => Self::ChooseStyle,
        }
    }
}

impl From<core::AiEndpointGaps> for AiEndpointGaps {
    fn from(gaps: core::AiEndpointGaps) -> Self {
        Self {
            address: gaps.address.map(|gap| match gap {
                core::AiEndpointAddressGap::Missing => AiEndpointAddressGap::Missing,
                core::AiEndpointAddressGap::Unusable => AiEndpointAddressGap::Unusable,
                core::AiEndpointAddressGap::NeedsHttps => AiEndpointAddressGap::NeedsHttps,
            }),
            model_missing: gaps.model_missing,
            declaration: gaps.declaration.map(|gap| match gap {
                core::AiEndpointDeclarationGap::Missing => AiEndpointDeclarationGap::Missing,
                core::AiEndpointDeclarationGap::Refused(refused) => {
                    AiEndpointDeclarationGap::Refused {
                        refusal: refused.into(),
                    }
                }
            }),
            kept_key: gaps.kept_key.map(|kept| match kept {
                core::AiEndpointKeptKey::Carried => AiEndpointKeptKey::Carried,
                core::AiEndpointKeptKey::Dropped => AiEndpointKeptKey::Dropped,
            }),
        }
    }
}

/// The form for `view`: while a change runs or waits, both controls are held, and the one that
/// started it runs (`docs/waiting.md`); Remove is drawn only while there is something to remove.
pub(crate) fn endpoint_form(view: core::AiEndpointView) -> Option<AiEndpointForm> {
    if !view.offered {
        return None;
    }
    let busy = !view.work.is_empty();
    let runs = |kind: core::AiEndpointChangeKind| view.work.contains(&kind);
    Some(AiEndpointForm {
        address: view.fields.address.clone(),
        model: view.fields.model.clone(),
        declared: view.fields.declared.map(Into::into),
        key_kept: view.fields.key_kept,
        save: ControlState {
            enabled: !busy,
            running: runs(core::AiEndpointChangeKind::Save),
        },
        remove: view.removable.then(|| ControlState {
            enabled: !busy,
            running: runs(core::AiEndpointChangeKind::Remove),
        }),
        outcome: view.outcome.map(|outcome| AiEndpointOutcome {
            serial: outcome.serial,
            kind: match outcome.kind {
                core::AiEndpointChangeKind::Save => AiEndpointChangeKind::Save,
                core::AiEndpointChangeKind::Remove => AiEndpointChangeKind::Remove,
            },
            result: match outcome.result {
                core::AiEndpointResult::Saved => AiEndpointResult::Saved,
                core::AiEndpointResult::Removed => AiEndpointResult::Removed,
                core::AiEndpointResult::Refused { gaps } => {
                    AiEndpointResult::Refused { gaps: gaps.into() }
                }
                core::AiEndpointResult::KeyNotStored => AiEndpointResult::KeyNotStored,
                core::AiEndpointResult::NotWritten => AiEndpointResult::NotWritten,
            },
        }),
    })
}
