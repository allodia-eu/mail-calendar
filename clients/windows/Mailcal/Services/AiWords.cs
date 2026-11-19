// The writing assistant's failures and remedies in words (docs/ai.md rule 27). Which failure
// happened and which remedy it offers are the core's; this words them, once, for every surface
// that draws one: Settings → Writing style, its learning flow and Draft a reply. WinUI-free and
// L10n-free, so Mailcal.Tests can link it; the catalog's words arrive as a record
// (AiWords.Catalog.cs builds it in the app).

using uniffi.mailcal_bindings;

namespace Allodia.Mailcal.Services;

/// <summary>The catalog's <c>ai_failure_*</c> and <c>ai_remedy_*</c> words.</summary>
internal sealed partial record AiWords(
    string RelaySignIn,
    string KeyRefused,
    string RefusedEuNative,
    string RefusedEuHosted,
    string NowhereToSend,
    string Unreachable,
    string ServerError,
    string TooManyRequests,
    string Unreadable,
    string OutOfCredits,
    string NotEntitled,
    string NoSentFolder,
    string NothingToLearn,
    string SampleTooLarge,
    string NoStyle,
    string NoStyleForAccount,
    string MessageGone,
    string RemedySignIn,
    string RemedyOpenEndpoint,
    string RemedyTryAgain,
    string RemedyChooseAccount,
    string RemedyChooseRange,
    string RemedyLearnStyle,
    string RemedyChooseStyle)
{
    /// <summary>What went wrong, as the person reads it; never a server's own sentence.</summary>
    internal string Of(AiFailure failure) => failure switch
    {
        AiFailure.RelaySignIn => RelaySignIn,
        AiFailure.EndpointKeyRefused => KeyRefused,
        // Only a mode narrower than "anywhere" refuses, and each says what it admits.
        AiFailure.Refused refused => refused.Mode == JurisdictionMode.EuHosted ? RefusedEuHosted : RefusedEuNative,
        AiFailure.NowhereToSend => NowhereToSend,
        AiFailure.Unreachable => Unreachable,
        AiFailure.ServerError => ServerError,
        AiFailure.TooManyRequests => TooManyRequests,
        AiFailure.Unreadable => Unreadable,
        AiFailure.OutOfCredits => OutOfCredits,
        AiFailure.NotEntitled => NotEntitled,
        AiFailure.NoSentFolder => NoSentFolder,
        AiFailure.NothingToLearn => NothingToLearn,
        AiFailure.SampleTooLarge => SampleTooLarge,
        AiFailure.NoStyle => NoStyle,
        AiFailure.NoStyleForAccount => NoStyleForAccount,
        AiFailure.MessageGone => MessageGone,
        _ => throw new ArgumentOutOfRangeException(nameof(failure), failure, null),
    };

    /// <summary>The words on the one control a failure offers.</summary>
    internal string Of(AiRemedy remedy) => remedy switch
    {
        AiRemedy.SignInToAllodia => RemedySignIn,
        AiRemedy.OpenAiEndpoint => RemedyOpenEndpoint,
        AiRemedy.TryAgain => RemedyTryAgain,
        AiRemedy.ChooseAccount => RemedyChooseAccount,
        AiRemedy.ChooseRange => RemedyChooseRange,
        AiRemedy.LearnStyle => RemedyLearnStyle,
        AiRemedy.ChooseStyle => RemedyChooseStyle,
        _ => throw new ArgumentOutOfRangeException(nameof(remedy), remedy, null),
    };
}
