// Deciding whether a launch carries a mail link (`mailto:`), and pulling the URI out of the two
// shapes Windows delivers one in.
//
// Kept WinUI- and WinRT-free, and apart from the window, so the JVM-equivalent gate is reachable by
// the unit suite (MailLinkTests), the Android client splits MailtoLaunch off its Activity for the
// same reason. What is deliberately NOT here is the parse: the URI is decoded by the shared core
// (parse_mailto_uri), so every platform honours one header allowlist rather than three
// (docs/composer-security.md, Gate 12).
using System;
using System.Linq;

namespace Allodia.Mailcal.Services;

/// <summary>The mail-link (<c>mailto:</c>) activation gate: is this launch one, and which URI.</summary>
internal static class MailLink
{
    /// <summary>The URI scheme a mail link arrives on. Must equal the protocol name
    /// <c>Package.appxmanifest</c> declares, which is what makes the OS offer this app as a mail
    /// handler at all, <see cref="MailLinkTests"/> asserts the two against each other.</summary>
    internal const string Scheme = "mailto";

    private const string Prefix = Scheme + ":";

    /// <summary>
    /// Whether an activation on <paramref name="scheme"/> is a mail link.
    /// </summary>
    /// <remarks>
    /// The check that keeps this off the OAuth redirects: those arrive as protocol activations too
    /// (<c>eu.allodia.mailcal://auth</c>, <c>//jmap-oauth</c>), and treating one as a mail link
    /// would swallow a sign-in and pop a composer in the middle of adding an account. Matching on
    /// the activation kind alone is the bug this exists to prevent.
    /// <para>
    /// Whether the URI is a <i>well-formed</i> mail link is deliberately not decided here, the
    /// shared core answers that, and drops the headers a link may not set.
    /// </para>
    /// </remarks>
    internal static bool CarriesMailLink(string? scheme) =>
        string.Equals(scheme, Scheme, StringComparison.OrdinalIgnoreCase);

    /// <summary>
    /// The mail link in a process command line, or <c>null</c> when there is none.
    /// </summary>
    /// <remarks>
    /// The classic Win32 hand-off: a registered protocol handler is invoked as
    /// <c>"Mailcal.exe" "mailto:…"</c>, so the URI is an ordinary argument. The packaged build is
    /// activated through the manifest instead and never comes this way, but both shapes end at the
    /// same composer, and this one is what makes the feature reachable in the unpackaged dev loop,
    /// which deliberately does not register itself as the machine's mail handler.
    /// </remarks>
    internal static string? FromArguments(string[] args) =>
        args.FirstOrDefault(a => a.StartsWith(Prefix, StringComparison.OrdinalIgnoreCase));

    /// <summary>
    /// The mail link in a redirected launch's raw argument string, or <c>null</c>.
    /// </summary>
    /// <remarks>
    /// A launch activation hands the tail of the command line over unsplit, so this cannot use the
    /// whitespace split <see cref="StartupOptions.WantsCalendar(string?)"/> gets away with for a
    /// bare flag: an unencoded <c>?subject=lunch on Friday</c> would be truncated at the first
    /// space and the user would silently lose most of their subject. A line that <i>is</i> the URI
    /// is therefore taken whole, quotes stripped; only a line with something else in front of the
    /// link falls back to splitting it.
    /// </remarks>
    internal static string? FromArgumentLine(string? line)
    {
        if (string.IsNullOrWhiteSpace(line))
        {
            return null;
        }
        var trimmed = line.Trim().Trim('"');
        return trimmed.StartsWith(Prefix, StringComparison.OrdinalIgnoreCase)
            ? trimmed
            : FromArguments(trimmed.Split((char[]?)null, StringSplitOptions.RemoveEmptyEntries));
    }
}
