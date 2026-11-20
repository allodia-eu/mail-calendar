// Carries a new-mail notifications choice kept in notifications.txt beside the other preference
// files into the core, which holds it from then on. Run once a launch, as soon as the core exists
// and before anything asks it for new mail, so a person who turned notifications off is not
// notified on the first launch that reads the choice from the core.
//
// The file goes once read, so the choice is carried exactly once and a later change in Settings
// is never overwritten by it. A file that cannot be removed is left for the next launch, and its
// choice with it, for the same reason.

using System.IO;
using uniffi.mailcal_bindings;

namespace Allodia.Mailcal.Services;

/// <summary>Moves a stored new-mail notifications choice into the core, once.</summary>
internal static class NotificationsImport
{
    /// <summary>Where the choice was kept: <c>notifications.txt</c> in
    /// <paramref name="prefsDir"/>.</summary>
    internal static string FileIn(string prefsDir) => Path.Combine(prefsDir, "notifications.txt");

    /// <summary>
    /// Removes the file at <paramref name="path"/> and, when it said <c>off</c>, sends the intent
    /// that turns new-mail notifications off. Anything else it said was the default, which the
    /// core already holds. Sends nothing when there is no file, or when it could not be read or
    /// removed.
    /// </summary>
    /// <returns>Whether the choice was carried over as off.</returns>
    internal static bool Run(string path, Action<PreferenceIntent> dispatch)
    {
        string text;
        try
        {
            text = File.ReadAllText(path);
            File.Delete(path);
        }
        catch (Exception ex) when (ex is IOException or UnauthorizedAccessException)
        {
            return false;
        }
        if (text.Trim() != "off")
        {
            return false;
        }
        dispatch(new PreferenceIntent.SetNewMailNotifications(false));
        return true;
    }
}
