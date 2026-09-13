// Theme brushes for surfaces that are built in code rather than in XAML.
//
// `Application.Current.Resources["…Brush"]` answers with the theme the APPLICATION is in, which is
// the desktop's and is fixed before any window exists. The appearance setting is applied to the
// content root instead (MainWindow.Theme.cs), so whenever the two differ, a light app on a dark
// desktop or the reverse, every brush read that way comes back inverted: near-white label text on a
// light card, and the reverse on a dark one. It renders perfectly and is simply unreadable, so
// nothing downstream can tell; the appearance the capture set is pinned to is exactly where it
// bites, because a screenshot run deliberately overrides the desktop.
//
// XAML is unaffected: `{ThemeResource}` resolves against the element, which is why the same text in
// AccountSetupView.xaml is right while the panel built beside it in code is not.
//
// The neutral tokens carry their values here because the framework offers no way to read them: a
// ResourceDictionary's indexer resolves against the active theme whichever of its theme
// dictionaries is asked, so "Light" and "Default" both answer with whatever is on screen now. The
// accent ramp needs no copy. SystemAccentColorDark2 and SystemAccentColorLight3 are theme-
// independent, and are the two colours AccentTextFillColorPrimaryBrush is itself built from.
using System;
using System.Collections.Generic;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Media;
using Windows.UI;

namespace Allodia.Mailcal.Services;

/// <summary>The Fluent colours a code-built surface needs, for the theme an element is actually in.</summary>
internal static class ThemePalette
{
    /// <summary>Body text.</summary>
    internal static Color PrimaryText(bool dark) =>
        dark ? Argb(0xFF, 0xFF, 0xFF, 0xFF) : Argb(0xE4, 0x00, 0x00, 0x00);

    /// <summary>A label, a caption, anything subordinate to the line beside it.</summary>
    internal static Color SecondaryText(bool dark) =>
        dark ? Argb(0xC5, 0xFF, 0xFF, 0xFF) : Argb(0x9E, 0x00, 0x00, 0x00);

    /// <summary>The hairline between two blocks of content.</summary>
    internal static Color Divider(bool dark) =>
        dark ? Argb(0x15, 0xFF, 0xFF, 0xFF) : Argb(0x0F, 0x00, 0x00, 0x00);

    /// <summary>A control's outline.</summary>
    internal static Color ControlStroke(bool dark) =>
        dark ? Argb(0x12, 0xFF, 0xFF, 0xFF) : Argb(0x0F, 0x00, 0x00, 0x00);

    /// <summary>A panel set slightly off the page it sits on, a list or a preview.</summary>
    internal static Color LayerFill(bool dark) =>
        dark ? Argb(0x4C, 0x3A, 0x3A, 0x3A) : Argb(0x80, 0xFF, 0xFF, 0xFF);

    /// <summary>A card's fill, over the page behind it.</summary>
    internal static Color CardBackground(bool dark) =>
        dark ? Argb(0x0D, 0xFF, 0xFF, 0xFF) : Argb(0xB3, 0xFF, 0xFF, 0xFF);

    /// <summary>Something went wrong and the user has to act.</summary>
    internal static Color Critical(bool dark) =>
        dark ? Argb(0xFF, 0xFF, 0x99, 0xA4) : Argb(0xFF, 0xC4, 0x2B, 0x1C);

    /// <summary>Worth noticing, nothing lost.</summary>
    internal static Color Caution(bool dark) =>
        dark ? Argb(0xFF, 0xFC, 0xE1, 0x00) : Argb(0xFF, 0x9D, 0x5D, 0x00);

    /// <summary>
    /// Accent text, the desktop's own accent colour stepped for legibility on this theme's ground.
    /// Read from the app rather than stated here: it is the user's choice, not Fluent's.
    /// </summary>
    internal static Color AccentText(bool dark) =>
        Resource(dark ? "SystemAccentColorLight3" : "SystemAccentColorDark2", SecondaryText(dark));

    /// <summary>Whether <paramref name="element"/> is showing dark, whatever the application is in.</summary>
    internal static bool IsDark(this FrameworkElement element) =>
        element.ActualTheme == ElementTheme.Dark;

    /// <summary>A brush of <paramref name="color"/>, the shape every caller wants.</summary>
    internal static SolidColorBrush Brush(Color color) => new(color);

    private static Color Argb(byte a, byte r, byte g, byte b) => Color.FromArgb(a, r, g, b);

    // A key that ships with WinUI, so a miss means the resource set was not merged. Falling back
    // keeps a card readable rather than taking the surface down with it.
    private static Color Resource(string key, Color fallback) =>
        Application.Current.Resources.TryGetValue(key, out var value) && value is Color color
            ? color
            : fallback;
}

/// <summary>
/// The brushes one code-built surface draws with, repainted in place when its theme changes.
/// </summary>
/// <remarks>
/// For a surface that is built once and then left alone, which is what every Settings panel and
/// both calendar dialogs are. A colour read once at build time is wrong there twice over, and a
/// dialog is where both show:
///
/// <list type="bullet">
/// <item>its content is built before <c>DialogHelper</c> mirrors the window's theme onto it, so
/// the read happens while the dialog still carries the application's;</item>
/// <item>the appearance picker lives inside Settings, so picking one has to reach the panels of
/// the very dialog it was picked in.</item>
/// </list>
///
/// Handing out a brush and repainting <b>it</b> answers both: every element that took the brush
/// follows without being found again, so nothing has to be tracked but the brush.
///
/// A surface that rebuilds anyway needs none of this and does not use it: the invitation card
/// redraws per message and simply re-reads <see cref="ThemePalette"/>.
/// </remarks>
internal sealed class ThemeBrushes
{
    private readonly FrameworkElement _owner;
    private readonly List<(SolidColorBrush Brush, Func<bool, Color> Role)> _tracked = [];

    /// <summary>Binds to <paramref name="owner"/>, whose theme every brush handed out follows.</summary>
    internal ThemeBrushes(FrameworkElement owner)
    {
        _owner = owner;
        owner.ActualThemeChanged += (_, _) => Repaint();
    }

    /// <summary>A brush for <paramref name="role"/>, in the owner's theme now and after a change.</summary>
    internal Brush Of(Func<bool, Color> role)
    {
        var brush = new SolidColorBrush(role(_owner.IsDark()));
        _tracked.Add((brush, role));
        return brush;
    }

    private void Repaint()
    {
        var dark = _owner.IsDark();
        foreach (var (brush, role) in _tracked)
        {
            brush.Color = role(dark);
        }
    }
}
