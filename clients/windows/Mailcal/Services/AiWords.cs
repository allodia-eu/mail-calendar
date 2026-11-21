// The writing assistant's failures and remedies in words, and where each remedy leads (docs/ai.md
// rules 11 and 27). Which failure happened and which remedy it offers are the core's; this words
// and routes them, once, for every surface that draws one: Settings → Writing style, its learning
// flow and Draft a reply. WinUI-free and L10n-free, so Mailcal.Tests can link it; the catalog's
// words arrive as a record (AiWords.Catalog.cs builds it in the app).

using uniffi.mailcal_bindings;

namespace Allodia.Mailcal.Services;

/// <summary>The catalog's <c>ai_failure_*</c> and <c>ai_remedy_*</c> words.</summary>
internal sealed partial record AiWords(
    string RelaySignIn,
    string KeyRefused,
    string RefusedEuNative,
    string RefusedEuHosted,
    string RefusedUndeclared,
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
        AiFailure.Refused { Refusal: JurisdictionRefusal.Unknown } => RefusedUndeclared,
        AiFailure.Refused refused => Refused(refused.Refusal, RefusedEuNative, RefusedEuHosted),
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

    /// <summary>
    /// A refusal, in the words of the limit that refused it: an endpoint run in the EU from outside
    /// it is refused only where everything must be run from the EU, and anything else by the limit
    /// the record names. <paramref name="euNative"/> and <paramref name="euHosted"/> say what each
    /// limit admits. A failure says an undeclared endpoint's refusal in words of its own, before it
    /// reaches this; the form's declaration line words it by the limit.
    /// </summary>
    internal static string Refused(JurisdictionRefusal refusal, string euNative, string euHosted) => refusal switch
    {
        JurisdictionRefusal.NonEu { Mode: JurisdictionLimit.EuHosted } or JurisdictionRefusal.Unknown { Mode: JurisdictionLimit.EuHosted } => euHosted,
        _ => euNative,
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

/// <summary>Where a remedy takes the person, from Settings → Writing style and from the composer
/// alike (docs/ai.md rules 11 and 27). Each surface keeps what it holds: the learning flow as its
/// Remedy left it, the composer its draft.</summary>
internal enum AiRemedyPlace
{
    /// <summary>What failed, again: the flow runs it, the composer reopens the instruction's
    /// input.</summary>
    Retry,

    /// <summary>The Allodia sign-in, started over whatever the remedy sat on; the learning flow
    /// waits on its review meanwhile.</summary>
    SignIn,

    /// <summary>Settings → Advanced, where the own endpoint is set up.</summary>
    Endpoint,

    /// <summary>Settings → Writing style, where an account's style is chosen; in the flow, the
    /// step its Remedy moved back to.</summary>
    WritingStyle,

    /// <summary>The learning flow, opened in Settings → Writing style.</summary>
    Learn,
}

internal static class AiRemedies
{
    internal static AiRemedyPlace PlaceOf(AiRemedy remedy) => remedy switch
    {
        AiRemedy.TryAgain => AiRemedyPlace.Retry,
        AiRemedy.SignInToAllodia => AiRemedyPlace.SignIn,
        AiRemedy.OpenAiEndpoint => AiRemedyPlace.Endpoint,
        AiRemedy.LearnStyle => AiRemedyPlace.Learn,
        AiRemedy.ChooseStyle or AiRemedy.ChooseAccount or AiRemedy.ChooseRange => AiRemedyPlace.WritingStyle,
        _ => throw new ArgumentOutOfRangeException(nameof(remedy), remedy, null),
    };
}
