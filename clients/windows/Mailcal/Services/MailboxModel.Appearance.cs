// The light/dark appearance half of MailboxModel: reading the core's persisted choice and writing
// a new one. Its own partial, like the other settings families, so no file carries the whole model.

using uniffi.mailcal_bindings;

namespace Allodia.Mailcal.Services;

public sealed partial class MailboxModel
{
    /// <summary>
    /// The persisted appearance. Falls back to what this launch is painted in while the core is
    /// still connecting, so the Settings picker never shows a selection the window contradicts.
    /// </summary>
    internal Appearance CurrentAppearance =>
        _app?.DisplaySettings().Appearance ?? AppearanceChoice.AtLaunch;

    /// <summary>
    /// Changes the appearance. The core signals Settings only, nothing it computes depends on
    /// this, so repainting the window from <see cref="CurrentAppearance"/> is the caller's job
    /// (<c>MainWindow.ApplyAppearance</c>).
    /// </summary>
    internal void SetAppearance(Appearance appearance)
    {
        Log.Info($"settings: set appearance={appearance}");
        DispatchPreference(new PreferenceIntent.SetAppearance(appearance));
    }

    /// <summary>
    /// Whether messages are drawn on a dark background while the app is dark: the reading view's
    /// sun/moon button and Settings → Reading both show it (docs/reading-dark-mode.md). Off while no
    /// core is open, when there is no message to draw; the first reload after one opens bumps
    /// <see cref="DisplaySettingsVersion"/>, which draws the core's.
    /// </summary>
    internal bool DarkMessageBackground => _app?.DisplaySettings().DarkMessageBackground == true;

    /// <summary>
    /// The app's 12/24-hour setting, which every mail timestamp follows as the calendar does
    /// (docs/timestamps.md). 24-hour while the core is still connecting, the core's own default.
    /// </summary>
    internal bool Use24Hour => _app?.DisplaySettings().TimeFormat != TimeFormat.TwelveHour;

    /// <summary>
    /// Changes the message background. Every open reading view draws it again on the Settings
    /// signal that follows, through <see cref="DisplaySettingsVersion"/>.
    /// </summary>
    internal void SetDarkMessageBackground(bool dark)
    {
        Log.Info($"settings: set dark message background={dark}");
        DispatchPreference(new PreferenceIntent.SetDarkMessageBackground(dark));
    }

    /// <summary>
    /// Hands the core a message the shared editor posted, and answers whether it was the toolbar's
    /// sun or moon, which sets the same preference as <see cref="SetDarkMessageBackground"/>. False
    /// while no core is open.
    /// </summary>
    internal bool ComposerBackgroundChoice(string message) => _app?.ComposerBackgroundChoice(message) == true;
}
