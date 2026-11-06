// The banner a refused Allodia sign-in raises beside the mail accounts' expired sign-in banner
// (`entitlement.md`, "A refused sign-in is also a banner"). The core decides when
// there is one; what is here is what it says.
//
// WinUI-free and L10n-free, so Mailcal.Tests links it: every word arrives as a parameter.

namespace Allodia.Mailcal.Services;

internal static class AllodiaSignInEndedBanner
{
    /// <summary>
    /// Whether the banner is drawn: exactly while the core names the address whose sign-in the
    /// service refused. A grant that is merely narrower than this build wants never names one.
    /// </summary>
    internal static bool Shown(string? endedFor) => !string.IsNullOrEmpty(endedFor);

    /// <summary>
    /// What a sign-in started from the banner leaves beneath its prompt: nothing when it completed
    /// or the person gave up, the stale-browser sentence when the browser came back from another
    /// sign-in, and the plain failure otherwise. Never the failure's own text, which is an OAuth
    /// protocol string and belongs in the log.
    /// </summary>
    internal static string? AfterAttempt(bool failed, bool stale, string staleSaid, string failedSaid) =>
        !failed ? null : stale ? staleSaid : failedSaid;

    /// <summary>The banner's message: the prompt, and the last attempt's failure after it.</summary>
    internal static string Message(string prompt, string? failure) =>
        failure is null ? prompt : $"{prompt} {failure}";
}
