// A debug-only refusal to send outside the local test harness.
//
// Driving the app against the seeded Stalwart harness means replying to fixtures whose senders are
// deliberately REAL-LOOKING external addresses (`news@example.com`, `ahmed.elamrani@example.eu`,
// …). A reply pre-fills its recipients from the original, so the composer opens with an external
// address already in To, and pressing Send hands it to the harness's outbound queue, which then
// tries to reach that domain for real and retries for days. Nothing about the composer says the
// recipient is not local, and nothing about the send says it left the fixture.
//
// It is not a hypothetical: it happened while this feature was being verified. The fix is a gate
// rather than a rule to remember, because the mistake is one keystroke away from every reply and
// the person making it is looking at the message, not the address.
//
// Scope, deliberately narrow:
//   * DEBUG builds only, compiled out of anything shipped.
//   * Only when a **harness** dev account is active (`MAILCAL_DEV_ACCOUNT=stalwart*`). A
//     `personal` run is the developer's real mail and must send wherever they say.
// So a release build, and a normal debug run against real accounts, behave exactly as before.

using System.Linq;

namespace Allodia.Mailcal.Services;

/// <summary>Whether a send is allowed to leave the local harness, while one is what we are
/// connected to.</summary>
internal static class HarnessRecipientGate
{
    /// <summary>The harness's own domain. Everything it seeds, and every account it serves, is
    /// under this; anything else is off-fixture and would be a real outbound delivery.</summary>
    internal const string LocalDomain = "test.local";

    /// <summary>
    /// The recipients in <paramref name="recipients"/> that are **not** local to the harness, in
    /// the order they appear. Empty when the send is safe.
    ///
    /// Parsing is deliberately forgiving in the unsafe direction: anything this cannot read as an
    /// address is reported as external, so a malformed entry blocks the send rather than slipping
    /// through it.
    /// </summary>
    internal static IReadOnlyList<string> ExternalRecipients(params string?[] recipients) =>
        recipients
            .Where(field => !string.IsNullOrWhiteSpace(field))
            .SelectMany(field => field!.Split([',', ';'], StringSplitOptions.RemoveEmptyEntries))
            .Select(entry => entry.Trim())
            .Where(entry => entry.Length > 0)
            .Where(entry => !IsLocal(entry))
            .ToList();

    // True when the entry's domain is the harness's. Handles both a bare address and a
    // `Name <addr>` form, which is what a pre-filled reply carries.
    private static bool IsLocal(string entry)
    {
        var address = entry;
        var open = entry.LastIndexOf('<');
        var close = entry.LastIndexOf('>');
        if (open >= 0 && close > open)
        {
            address = entry[(open + 1)..close].Trim();
        }
        var at = address.LastIndexOf('@');
        return at >= 0
            && address[(at + 1)..].Trim().Equals(LocalDomain, StringComparison.OrdinalIgnoreCase);
    }
}
