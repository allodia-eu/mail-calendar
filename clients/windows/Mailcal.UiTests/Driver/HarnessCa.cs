// The harness's certificates, which a debug core trusts through MAILCAL_EXTRA_CA. scripts/dev/harness.sh
// writes them into the checkout it runs from on every up and reset, and they are gitignored, so a
// git worktree has none of its own, or a copy older than the harness now running. Whichever of
// this checkout's copy and its main checkout's is newer is the one the harness serves.

namespace Allodia.Mailcal.UiTests.Driver;

internal static class HarnessCa
{
    private static readonly string[] Bundle = ["docker", "stalwart", "tls", "harness-ca.pem"];

    /// <summary>The newer of <paramref name="repo"/>'s bundle and its main checkout's, or
    /// <paramref name="repo"/>'s own path when neither exists, for the launch to fail on by name.</summary>
    internal static string For(string repo)
    {
        var own = Path.Combine([repo, .. Bundle]);
        var candidates = new[] { own, MainCheckout(repo) is { } main ? Path.Combine([main, .. Bundle]) : null }
            .OfType<string>()
            .Where(File.Exists)
            .OrderByDescending(File.GetLastWriteTimeUtc);
        return candidates.FirstOrDefault() ?? own;
    }

    /// <summary>The checkout <paramref name="repo"/> was made from, when it is a linked git worktree;
    /// null for a main checkout, whose <c>.git</c> is a directory rather than a pointer.</summary>
    internal static string? MainCheckout(string repo)
    {
        var pointer = Path.Combine(repo, ".git");
        if (!File.Exists(pointer))
        {
            return null;
        }
        var line = File.ReadAllText(pointer).Trim();
        if (!line.StartsWith("gitdir:", StringComparison.Ordinal))
        {
            return null;
        }
        var own = Path.GetFullPath(Path.Combine(repo, line["gitdir:".Length..].Trim()));
        var commondir = Path.Combine(own, "commondir");
        if (!File.Exists(commondir))
        {
            return null;
        }
        // The shared git directory, named relative to the worktree's own; its parent is the checkout.
        var common = Path.GetFullPath(Path.Combine(own, File.ReadAllText(commondir).Trim()));
        return Path.GetDirectoryName(common.TrimEnd(Path.DirectorySeparatorChar, Path.AltDirectorySeparatorChar));
    }
}
