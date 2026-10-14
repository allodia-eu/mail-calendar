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
    /// Persists the appearance. The core signals Settings only, nothing it computes depends on
    /// this, so repainting the window is the caller's job (<c>MainWindow.ApplyAppearance</c>).
    /// Without a core the pick applies to this session and is not written: the only launches with
    /// no core are the showcase, which has no store to write to, and the moments before the first
    /// connect returns.
    /// </summary>
    internal void SetAppearance(Appearance appearance)
    {
        Log.Info($"settings: set appearance={appearance}");
        _app?.SetAppearance(appearance);
    }

    /// <summary>
    /// Whether messages are drawn on a dark background while the app is dark: the reading view's
    /// sun/moon button and Settings → Reading both show it (docs/reading-dark-mode.md). On while
    /// the core is still connecting, which is the core's own default.
    /// </summary>
    internal bool DarkMessageBackground => _app?.DisplaySettings().DarkMessageBackground ?? true;

    /// <summary>
    /// Persists the message background, and bumps <see cref="DisplaySettingsVersion"/> now rather
    /// than on the Settings signal that follows, so every open reading view re-renders on the press.
    /// </summary>
    internal void SetDarkMessageBackground(bool dark)
    {
        Log.Info($"settings: set dark message background={dark}");
        _app?.SetDarkMessageBackground(dark);
        BumpDisplaySettings();
    }
}
