// Why a writing-style request produced nothing, as the sentence it earns (docs/ai.md: a failure is
// a WritingStyleFailure variant, never a server's sentence).
//
// A code rather than a string, like AllodiaWriteResult, so this half stays free of WinUI and L10n
// and Mailcal.Tests can pin it; the words are WritingStyleText. Two of its rules are silent when
// wrong: a refusal names the mode in force, and a refused sign-in means "sign in again" on
// Allodia's relay but "the key was refused" on an own endpoint.

using System;
using uniffi.mailcal_bindings;

namespace Allodia.Mailcal.Services;

/// <summary>Which sentence a writing-style failure earns.</summary>
internal enum AiProblem
{
    /// <summary>AI is not set up, or the core named no reason.</summary>
    Unavailable,

    /// <summary>A learning run is already going.</summary>
    Busy,

    /// <summary>The account has no Sent folder on this device.</summary>
    NoSentFolder,

    /// <summary>Nothing in the range says enough to learn from.</summary>
    NothingToLearn,

    /// <summary>No style to draft in.</summary>
    NoStyle,

    /// <summary>The message is no longer on this device.</summary>
    NotFound,

    /// <summary>The gate refused under the EU-native mode.</summary>
    RefusedEuNative,

    /// <summary>The gate refused under the EU-hosted mode.</summary>
    RefusedEuHosted,

    /// <summary>No credits left on the relay.</summary>
    OutOfCredits,

    /// <summary>The plan does not include AI.</summary>
    NotEntitled,

    /// <summary>The relay refused the Allodia sign-in.</summary>
    SignInAgain,

    /// <summary>An own endpoint refused its key.</summary>
    KeyRefused,

    /// <summary>Too many requests just now.</summary>
    RateLimited,

    /// <summary>The endpoint could not be reached in time.</summary>
    Unreachable,

    /// <summary>The endpoint answered with another HTTP status.</summary>
    Status,

    /// <summary>The answer could not be read.</summary>
    Malformed,

    /// <summary>The person stopped it.</summary>
    Cancelled,
}

/// <summary>A failure, with the HTTP status the one sentence that names a code needs.</summary>
/// <param name="Problem">Which sentence.</param>
/// <param name="StatusCode">For <see cref="AiProblem.Status"/>: the status the endpoint sent.</param>
internal sealed record AiFailure(AiProblem Problem, int StatusCode = 0)
{
    /// <summary>The failure <paramref name="error"/> reports, worded for <paramref name="route"/>.</summary>
    /// <remarks>
    /// Anything that is not a <see cref="WritingStyleFailure"/> (a panic crossing the FFI) is
    /// reported as unavailable: its own text is never shown.
    /// </remarks>
    internal static AiFailure For(Exception error, AiRoute? route) => error switch
    {
        WritingStyleFailure.Unavailable => new(AiProblem.Unavailable),
        WritingStyleFailure.Busy => new(AiProblem.Busy),
        WritingStyleFailure.NoSentFolder => new(AiProblem.NoSentFolder),
        WritingStyleFailure.NothingToLearn => new(AiProblem.NothingToLearn),
        WritingStyleFailure.NoStyle => new(AiProblem.NoStyle),
        WritingStyleFailure.NotFound => new(AiProblem.NotFound),
        WritingStyleFailure.Refused refused => Refusal(refused.mode),
        WritingStyleFailure.OutOfCredits => new(AiProblem.OutOfCredits),
        WritingStyleFailure.NotEntitled => new(AiProblem.NotEntitled),
        WritingStyleFailure.Unauthorized =>
            new(route == AiRoute.Relay ? AiProblem.SignInAgain : AiProblem.KeyRefused),
        WritingStyleFailure.RateLimited => new(AiProblem.RateLimited),
        WritingStyleFailure.Unreachable => new(AiProblem.Unreachable),
        WritingStyleFailure.Status status => new(AiProblem.Status, status.code),
        WritingStyleFailure.Malformed => new(AiProblem.Malformed),
        WritingStyleFailure.Cancelled => new(AiProblem.Cancelled),
        _ => new(AiProblem.Unavailable),
    };

    /// <summary>
    /// The gate's refusal under <paramref name="mode"/>. Only the two EU modes refuse anything; a
    /// mode that admits everything never reaches here.
    /// </summary>
    internal static AiFailure Refusal(JurisdictionMode mode) =>
        new(mode == JurisdictionMode.EuHosted ? AiProblem.RefusedEuHosted : AiProblem.RefusedEuNative);
}
