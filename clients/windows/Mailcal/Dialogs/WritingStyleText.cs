// The words the Writing style surface puts on screen for the codes and numbers the WinUI-free
// halves decide (AiFailure, OwnEndpointProblem, WritingStyleFormat). Here rather than
// in those files because L10n.cs cannot be linked into Mailcal.Tests, the same split as
// InvitationFormat and InvitationText.

using System.Collections.Generic;
using System.Globalization;
using Allodia.Mailcal.Services;
using uniffi.mailcal_bindings;

namespace Allodia.Mailcal.Dialogs;

/// <summary>Localised copy for the Writing style surface.</summary>
internal static class WritingStyleText
{
    /// <summary>The sentence a failure earns.</summary>
    internal static string Of(AiFailure failure) => failure.Problem switch
    {
        AiProblem.Busy => L10n.AiErrorBusy(),
        AiProblem.NoSentFolder => L10n.AiErrorNoSentFolder(),
        AiProblem.NothingToLearn => L10n.LearnReportNothing(),
        AiProblem.NoStyle => L10n.AiErrorNoStyle(),
        AiProblem.NotFound => L10n.AiErrorNotFound(),
        AiProblem.RefusedEuNative => L10n.WritingStyleRefusedEuNative(),
        AiProblem.RefusedEuHosted => L10n.WritingStyleRefusedEuHosted(),
        AiProblem.OutOfCredits => L10n.AiErrorOutOfCredits(),
        AiProblem.NotEntitled => L10n.AiErrorNotEntitled(),
        AiProblem.SignInAgain => L10n.AiErrorSignInAgain(),
        AiProblem.KeyRefused => L10n.AiErrorKeyRefused(),
        AiProblem.RateLimited => L10n.AiErrorRateLimited(),
        AiProblem.Unreachable => L10n.AiErrorUnreachable(),
        AiProblem.Status => L10n.AiErrorStatus(failure.StatusCode.ToString(CultureInfo.InvariantCulture)),
        AiProblem.Malformed => L10n.AiErrorMalformed(),
        AiProblem.Cancelled => L10n.AiErrorCancelled(),
        _ => L10n.AiErrorUnavailable(),
    };

    /// <summary>The sentence an own endpoint that was not saved earns.</summary>
    internal static string Of(OwnEndpointProblem problem) => problem switch
    {
        OwnEndpointProblem.Address => L10n.AiEndpointErrorAddress(),
        OwnEndpointProblem.Https => L10n.AiEndpointErrorHttps(),
        OwnEndpointProblem.Model => L10n.AiEndpointErrorModel(),
        _ => L10n.AiEndpointErrorKeystore(),
    };

    /// <summary>The word beside a greeting's or sign-off's bar. The core decides which, so every
    /// client agrees.</summary>
    internal static string Frequency(HabitFrequency frequency) => frequency switch
    {
        HabitFrequency.Mostly => L10n.RevealFrequencyMostly(),
        HabitFrequency.Often => L10n.RevealFrequencyOften(),
        _ => L10n.RevealFrequencySometimes(),
    };

    /// <summary>The credits line, with the balance's time in <paramref name="zone"/>.</summary>
    internal static string Credits(CreditBalance balance, string zone) =>
        L10n.WritingStyleCredits(
            WritingStyleFormat.Credits(balance.Credits, CultureInfo.CurrentCulture),
            WritingStyleFormat.AsOf(
                balance.AsOf, WritingStyleFormat.Zone(zone), DateTimeOffset.UtcNow, CultureInfo.CurrentCulture));

    /// <summary>A day in <paramref name="zone"/>, as the Writing style screens state one.</summary>
    internal static string Date(long unixSeconds, string zone) =>
        WritingStyleFormat.Date(unixSeconds, WritingStyleFormat.Zone(zone), CultureInfo.CurrentCulture);

    /// <summary>Languages by their own names, in the order given.</summary>
    internal static string Languages(IEnumerable<string> codes) =>
        WritingStyleFormat.Languages(codes, L10n.LanguageName);

    /// <summary>One language by its own name.</summary>
    internal static string LanguageName(string code) => Languages(new[] { code });
}
