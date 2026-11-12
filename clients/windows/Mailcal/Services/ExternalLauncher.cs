// Every page, app or system setting this client hands to the OS opens through here: a browser
// sign-in, a meeting's Join, a link in a message, Windows' default-apps page.
//
// A debug build launched with MAILCAL_NO_EXTERNAL_OPEN=1 opens nothing and answers as though the
// OS had opened it, so a run of the UI suites, which complete a browser sign-in with the requests
// the page would make rather than through the page, leaves no browser tabs behind. The log says
// that a launch was held back and never what it was: a URL can carry a sign-in's state and the
// account it is for (docs/logging.md).

namespace Allodia.Mailcal.Services;

internal static class ExternalLauncher
{
    /// <summary>Opens <paramref name="uri"/> in whatever the OS assigns to it; whether it did.</summary>
    public static async Task<bool> Open(Uri uri)
    {
#if DEBUG
        if (Environment.GetEnvironmentVariable("MAILCAL_NO_EXTERNAL_OPEN")?.Trim() == "1")
        {
            Log.Info($"launch hook: MAILCAL_NO_EXTERNAL_OPEN -> a {uri.Scheme} launch was not handed to the OS");
            return true;
        }
#endif
        return await Windows.System.Launcher.LaunchUriAsync(uri);
    }
}
