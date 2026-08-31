// Where the Windows client keeps its writable state. The shared root holds the engine store,
// the rotating log, and the credential index; the one-line preference files (language, window
// placement, pane width, diagnostics log level) resolve through PrefsDir, which a debug harness
// run (MAILCAL_DEV_ACCOUNT=stalwart / stalwart-imap) redirects into the same isolated dev
// subdir as that run's engine store, so a test that resizes the window, switches the language,
// or flips the Diagnostics DEBUG toggle never rewrites the developer's real preferences. The
// mapping is the single source the engine store, the credential namespace
// (CredentialStore.UseDevNamespace), and the preference stores all share. WinUI-free on
// purpose, so the resolution is unit-tested in Mailcal.Tests (AppPathsTests).

using System.IO;

namespace Allodia.Mailcal.Services;

/// <summary>The client's writable data locations: the shared root, and the (dev-isolated) preferences dir.</summary>
internal static class AppPaths
{
    /// <summary>
    /// The shared app-data root (<c>%LOCALAPPDATA%\Allodia\MailCalendar</c>), engine store,
    /// rotating log, and credential index. The log deliberately stays here even under a dev run,
    /// so one file diagnoses whatever ran last on this machine.
    /// </summary>
    internal static string Root => Path.Combine(
        Environment.GetFolderPath(Environment.SpecialFolder.LocalApplicationData),
        "Allodia", "MailCalendar");

    /// <summary>
    /// Where the one-line preference files live: the root normally, the harness run's dev store
    /// subdir under <c>MAILCAL_DEV_ACCOUNT</c>, throwaway, wiped with the rest of the dev store.
    /// A release build never consults the environment variable.
    /// </summary>
    internal static string PrefsDir
    {
        get
        {
#if DEBUG
            return ResolvePrefsDir(Root, Environment.GetEnvironmentVariable("MAILCAL_DEV_ACCOUNT"));
#else
            return Root;
#endif
        }
    }

    /// <summary>Pure form of <see cref="PrefsDir"/> (testable): the root, or its dev subdir.</summary>
    internal static string ResolvePrefsDir(string root, string? devAccountRaw) =>
        DevStoreSubdir(devAccountRaw) is { } subdir ? Path.Combine(root, subdir) : root;

    /// <summary>
    /// The isolated store subdirectory for a dev mode (<c>dev</c> / <c>dev-multi</c> /
    /// <c>dev-imap</c> / <c>dev-first-run</c>), or <c>null</c> for a normal launch (unset,
    /// <c>personal</c>, or an unsupported value, which must fall back to the real paths exactly
    /// like the account resolution it mirrors).
    /// </summary>
    /// <remarks>
    /// Every mode gets its OWN subdir, never a shared one: two modes on one SQLite store means a
    /// two-account harness run leaves accounts behind in the single-account one, which then boots
    /// showing mail it was never given.
    ///
    /// <c>dev-first-run</c> is the one nothing injects into: it exists so the screen somebody sees
    /// once can be seen again. Delete the directory to get it back.
    /// </remarks>
    internal static string? DevStoreSubdir(string? raw) => raw?.Trim().ToLowerInvariant() switch
    {
        "stalwart" => "dev",
        "stalwart-multi" => "dev-multi",
        "stalwart-imap" => "dev-imap",
        "first-run" => "dev-first-run",
        _ => null,
    };
}
