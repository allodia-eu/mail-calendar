// Which other dev builds' claims on the dev scheme to withdraw, the pure half of
// Program.ClaimDevSchemeForThisBuild, split out so it can be unit-tested.
//
// An unpackaged build registers the scheme against its own exe path, and the Windows App SDK keys
// that registration on a hash of the path. Every checkout, worktree and architecture is a separate
// path, so each one that ever ran adds a handler, nothing removes it, and a sign-in redirect then
// opens a "select an app" picker listing them all. Only the build that is running can finish the
// sign-in: the redirect is delivered to whichever exe the person picks, and another build's
// process has no sign-in waiting. So the build that launched last owns the scheme.

namespace Allodia.Mailcal.Services;

/// <summary>Decides which registered handlers of the dev scheme belong to other builds.</summary>
internal static class DevSchemeOwnership
{
    /// <summary>
    /// The exe paths among <paramref name="registered"/> that are not <paramref name="own"/>, once
    /// each, compared as Windows compares paths: case-insensitively, after normalising. An entry
    /// that names no exe is skipped, because the only way to withdraw a registration is by the
    /// exe it was made for.
    /// </summary>
    internal static IReadOnlyList<string> OthersToWithdraw(IEnumerable<string?> registered, string own)
    {
        var ownFull = Normalise(own);
        var seen = new HashSet<string>(StringComparer.OrdinalIgnoreCase);
        var others = new List<string>();
        foreach (var exe in registered)
        {
            if (string.IsNullOrWhiteSpace(exe) || Normalise(exe) is not { } full)
            {
                continue;
            }
            if (string.Equals(full, ownFull, StringComparison.OrdinalIgnoreCase) || !seen.Add(full))
            {
                continue;
            }
            others.Add(exe);
        }
        return others;
    }

    /// <summary>
    /// The exe a ProgID's <c>shell\open\command</c> launches: the quoted path when it is quoted,
    /// else everything up to the first space. <c>null</c> when there is none.
    /// </summary>
    internal static string? ExeFromCommand(string? command)
    {
        var text = command?.Trim();
        if (string.IsNullOrEmpty(text))
        {
            return null;
        }
        if (text[0] == '"')
        {
            var close = text.IndexOf('"', 1);
            return close > 1 ? text[1..close] : null;
        }
        var space = text.IndexOf(' ', StringComparison.Ordinal);
        return space < 0 ? text : text[..space];
    }

    private static string? Normalise(string path)
    {
        try
        {
            return Path.GetFullPath(path.Trim());
        }
        catch (Exception e) when (e is ArgumentException or NotSupportedException or PathTooLongException)
        {
            return null;
        }
    }
}
