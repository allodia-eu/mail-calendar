// The dev-isolation path resolution behind every one-line preference store (language, window
// placement, pane width, diagnostics log level): a MAILCAL_DEV_ACCOUNT harness run must read and
// write its preferences inside the dev store subdir, the same throwaway directory as its engine
// store, so driving the app in a test never rewrites the developer's real preferences. The
// mapping here is the single source the engine store, the credential namespace, and the
// preference files all share; if it drifts, a dev run half-isolates, which is worse than not
// isolating at all.

using System.Collections.Generic;
using System.IO;
using Allodia.Mailcal.Services;
using Xunit;

namespace Allodia.Mailcal.Tests;

public class AppPathsTests
{
    [Theory]
    [InlineData("stalwart", "dev")]
    [InlineData("stalwart-multi", "dev-multi")]
    [InlineData("stalwart-imap", "dev-imap")]
    [InlineData(" STALWART ", "dev")]           // trimmed + case-insensitive, like the account switch
    [InlineData("Stalwart-Imap", "dev-imap")]
    [InlineData("Stalwart-Multi", "dev-multi")]
    [InlineData("first-run", "dev-first-run")]  // injects nothing, but is isolated like the rest
    [InlineData("First-Run", "dev-first-run")]
    public void DevModesMapToTheirStoreSubdir(string raw, string expected) =>
        Assert.Equal(expected, AppPaths.DevStoreSubdir(raw));

    // No two dev modes may share a store. They would share a SQLite database, so the accounts a
    // two-account run connected would linger in the single-account one, which then boots showing
    // mail it was never given. For first-run the same collision is what makes the screen
    // unreachable: any account left by another mode is an account, so the form never comes up.
    // None may resolve to the real (null) paths either.
    [Fact]
    public void NoTwoDevModesShareAStore()
    {
        string?[] subdirs =
        [
            AppPaths.DevStoreSubdir("stalwart"),
            AppPaths.DevStoreSubdir("stalwart-multi"),
            AppPaths.DevStoreSubdir("stalwart-imap"),
            AppPaths.DevStoreSubdir("first-run"),
        ];
        Assert.All(subdirs, Assert.NotNull);
        Assert.Equal(subdirs.Length, new HashSet<string?>(subdirs).Count);
    }

    [Theory]
    [InlineData(null)]
    [InlineData("")]
    [InlineData("   ")]
    [InlineData("personal")]   // explicit real-accounts mode
    [InlineData("demo")]       // recognized elsewhere, unsupported on Windows, falls back to real
    [InlineData("stalwart2")]  // junk never silently isolates
    public void EverythingElseIsANormalLaunch(string? raw) =>
        Assert.Null(AppPaths.DevStoreSubdir(raw));

    [Fact]
    public void PrefsResolveIntoTheDevSubdirForAHarnessRun()
    {
        var root = Path.Combine("C:", "data");
        Assert.Equal(Path.Combine(root, "dev"), AppPaths.ResolvePrefsDir(root, "stalwart"));
        Assert.Equal(Path.Combine(root, "dev-imap"), AppPaths.ResolvePrefsDir(root, "stalwart-imap"));
    }

    [Fact]
    public void PrefsStayAtTheRootForANormalLaunch()
    {
        var root = Path.Combine("C:", "data");
        Assert.Equal(root, AppPaths.ResolvePrefsDir(root, null));
        Assert.Equal(root, AppPaths.ResolvePrefsDir(root, "personal"));
    }
}
