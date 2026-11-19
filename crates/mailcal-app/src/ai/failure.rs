//! What can go wrong for the writing assistant, and the one remedy each offers (`docs/ai.md`
//! rule 27). Decided here once; a client words both and never shows a server's sentence.

use mailcal_ai::{AiError, Destination, Refused};

/// Why learning or drafting did not happen. Stopping is not a failure, and has no value here.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AiFailure {
    /// The relay refused the Allodia sign-in, or the sign-in lacks `mailcal:ai:use`.
    RelaySignIn,
    /// The own endpoint refused its key.
    EndpointKeyRefused,
    /// The jurisdiction gate refuses the own endpoint as declared, under the mode in force.
    Refused(Refused),
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
    /// The relay has no credits left for this person.
    OutOfCredits,
    /// The Allodia entitlement no longer carries `ai`; the feature stops being offered with it.
    NotEntitled,
    /// The account has no Sent folder.
    NoSentFolder {
        /// Whether another account used for mail could be learned from instead.
        another_account: bool,
    },
    /// Nothing in the range says enough to learn from.
    NothingToLearn,
    /// The sample was too large for the destination to take.
    SampleTooLarge,
    /// No style has been learned yet.
    NoStyle,
    /// The account the reply is sent from drafts in no style.
    NoStyleForAccount,
    /// The message answered is no longer on this device.
    MessageGone,
}

/// What a failure offers the person. Each is one action a client draws as one control.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AiRemedy {
    /// Sign in to the Allodia account again.
    SignInToAllodia,
    /// Open the own endpoint's form under Settings → Advanced.
    OpenAiEndpoint,
    /// Ask again.
    TryAgain,
    /// Go back to choosing the account to learn from.
    ChooseAccount,
    /// Go back to choosing the range to learn from.
    ChooseRange,
    /// Learn a style, in Settings → Writing style.
    LearnStyle,
    /// Choose the style this account drafts in, in Settings → Writing style.
    ChooseStyle,
}

/// A failure's remedy as a surface draws it: which one, and whether it can be pressed now. It is
/// held while the surface it sits on holds its controls, and never runs: what it starts is drawn
/// where it leads.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RemedyControl {
    /// The remedy.
    pub remedy: AiRemedy,
    /// It can be pressed now.
    pub enabled: bool,
}

impl AiFailure {
    /// The remedy this failure offers, as a control that is `enabled` or held.
    #[must_use]
    pub const fn remedy_control(self, enabled: bool) -> Option<RemedyControl> {
        match self.remedy() {
            Some(remedy) => Some(RemedyControl { remedy, enabled }),
            None => None,
        }
    }

    /// The one remedy this failure offers, or `None` when there is nothing the person can do here.
    #[must_use]
    pub const fn remedy(self) -> Option<AiRemedy> {
        match self {
            Self::RelaySignIn => Some(AiRemedy::SignInToAllodia),
            Self::EndpointKeyRefused | Self::Refused(_) | Self::NowhereToSend => {
                Some(AiRemedy::OpenAiEndpoint)
            }
            Self::Unreachable | Self::ServerError | Self::TooManyRequests | Self::Unreadable => {
                Some(AiRemedy::TryAgain)
            }
            Self::NoSentFolder {
                another_account: true,
            } => Some(AiRemedy::ChooseAccount),
            Self::NothingToLearn | Self::SampleTooLarge => Some(AiRemedy::ChooseRange),
            Self::NoStyle => Some(AiRemedy::LearnStyle),
            Self::NoStyleForAccount => Some(AiRemedy::ChooseStyle),
            Self::OutOfCredits
            | Self::NotEntitled
            | Self::NoSentFolder {
                another_account: false,
            }
            | Self::MessageGone => None,
        }
    }

    /// The failure a request sent to `destination` came to, or `None` when the person stopped it.
    ///
    /// A refused credential is "sign in again" from the relay and "the key was refused" from an
    /// own endpoint, which is why the destination is asked for: the two need different remedies.
    #[must_use]
    pub const fn of_request(error: &AiError, destination: Destination) -> Option<Self> {
        Some(match error {
            AiError::Cancelled => return None,
            AiError::Refused(refused) => Self::Refused(*refused),
            AiError::Unauthorized => match destination {
                Destination::AllodiaRelay => Self::RelaySignIn,
                Destination::OwnEndpoint { .. } => Self::EndpointKeyRefused,
            },
            AiError::OutOfCredits => Self::OutOfCredits,
            AiError::NotEntitled => Self::NotEntitled,
            AiError::RateLimited => Self::TooManyRequests,
            AiError::Unreachable => Self::Unreachable,
            // Payload Too Large: the one status a smaller range answers.
            AiError::Status(413) => Self::SampleTooLarge,
            AiError::Status(_) => Self::ServerError,
            AiError::Malformed => Self::Unreadable,
        })
    }
}

#[cfg(test)]
mod tests {
    use mailcal_ai::{AiError, Class, Destination, Limit, Refused};

    use super::{AiFailure, AiRemedy};

    const OWN: Destination = Destination::OwnEndpoint {
        declared: Some(Class::EuNative),
    };

    /// The table of `docs/ai.md` rule 27, row by row.
    #[test]
    fn each_failure_offers_the_remedy_the_contract_gives_it() {
        let table = [
            (AiFailure::RelaySignIn, Some(AiRemedy::SignInToAllodia)),
            (
                AiFailure::EndpointKeyRefused,
                Some(AiRemedy::OpenAiEndpoint),
            ),
            (
                AiFailure::Refused(Refused::NonEu {
                    mode: Limit::EuNative,
                }),
                Some(AiRemedy::OpenAiEndpoint),
            ),
            (AiFailure::NowhereToSend, Some(AiRemedy::OpenAiEndpoint)),
            (AiFailure::Unreachable, Some(AiRemedy::TryAgain)),
            (AiFailure::ServerError, Some(AiRemedy::TryAgain)),
            (AiFailure::TooManyRequests, Some(AiRemedy::TryAgain)),
            (AiFailure::Unreadable, Some(AiRemedy::TryAgain)),
            (AiFailure::OutOfCredits, None),
            (AiFailure::NotEntitled, None),
            (
                AiFailure::NoSentFolder {
                    another_account: true,
                },
                Some(AiRemedy::ChooseAccount),
            ),
            (
                AiFailure::NoSentFolder {
                    another_account: false,
                },
                None,
            ),
            (AiFailure::NothingToLearn, Some(AiRemedy::ChooseRange)),
            (AiFailure::SampleTooLarge, Some(AiRemedy::ChooseRange)),
            (AiFailure::NoStyle, Some(AiRemedy::LearnStyle)),
            (AiFailure::NoStyleForAccount, Some(AiRemedy::ChooseStyle)),
            (AiFailure::MessageGone, None),
        ];
        for (failure, remedy) in table {
            assert_eq!(failure.remedy(), remedy, "{failure:?}");
        }
    }

    #[test]
    fn a_refused_credential_is_a_sign_in_at_the_relay_and_a_refused_key_at_an_endpoint() {
        assert_eq!(
            AiFailure::of_request(&AiError::Unauthorized, Destination::AllodiaRelay),
            Some(AiFailure::RelaySignIn)
        );
        assert_eq!(
            AiFailure::of_request(&AiError::Unauthorized, OWN),
            Some(AiFailure::EndpointKeyRefused)
        );
    }

    #[test]
    fn stopping_is_no_failure_and_every_other_error_has_one() {
        assert_eq!(AiFailure::of_request(&AiError::Cancelled, OWN), None);
        let cases = [
            (
                AiError::Refused(Refused::EuHosted),
                AiFailure::Refused(Refused::EuHosted),
            ),
            (AiError::OutOfCredits, AiFailure::OutOfCredits),
            (AiError::NotEntitled, AiFailure::NotEntitled),
            (AiError::RateLimited, AiFailure::TooManyRequests),
            (AiError::Unreachable, AiFailure::Unreachable),
            (AiError::Status(413), AiFailure::SampleTooLarge),
            (AiError::Status(502), AiFailure::ServerError),
            (AiError::Malformed, AiFailure::Unreadable),
        ];
        for (error, failure) in cases {
            assert_eq!(
                AiFailure::of_request(&error, OWN),
                Some(failure),
                "{error:?}"
            );
        }
    }
}
