// The catalog's words for AiWords, in the language the app is shown in.

namespace Allodia.Mailcal.Services;

internal sealed partial record AiWords
{
    /// <summary>The catalog's words, read once per drawing.</summary>
    internal static AiWords Catalog() => new(
        L10n.AiFailureRelaySignIn(),
        L10n.AiFailureKeyRefused(),
        L10n.AiFailureRefusedEuNative(),
        L10n.AiFailureRefusedEuHosted(),
        L10n.AiFailureRefusedUndeclared(),
        L10n.AiFailureNowhereToSend(),
        L10n.AiFailureUnreachable(),
        L10n.AiFailureServerError(),
        L10n.AiFailureTooManyRequests(),
        L10n.AiFailureUnreadable(),
        L10n.AiFailureOutOfCredits(),
        L10n.AiFailureNotEntitled(),
        L10n.AiFailureNoSentFolder(),
        L10n.AiFailureNothingToLearn(),
        L10n.AiFailureSampleTooLarge(),
        L10n.AiFailureNoStyle(),
        L10n.AiFailureNoStyleForAccount(),
        L10n.AiFailureMessageGone(),
        L10n.AiRemedySignIn(),
        L10n.AiRemedyOpenEndpoint(),
        L10n.AiRemedyTryAgain(),
        L10n.AiRemedyChooseAccount(),
        L10n.AiRemedyChooseRange(),
        L10n.AiRemedyLearnStyle(),
        L10n.AiRemedyChooseStyle());
}
