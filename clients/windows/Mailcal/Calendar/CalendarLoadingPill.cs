// The pill a calendar surface lays over its grid while the period is loading (docs/calendar.md,
// "Showing a period that is loading"): a spinner beside "loading this period…", floating over the
// grid it leaves drawn, pinned to what is on screen. The caller places it; it never moves with the
// scrolled day.
using System.Numerics;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Automation;
using Microsoft.UI.Xaml.Automation.Peers;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Media;
using Windows.UI;

namespace Allodia.Mailcal.Calendar;

internal sealed class CalendarLoadingPill : UserControl
{
    private readonly Border _pill = new()
    {
        CornerRadius = new CornerRadius(16),
        BorderThickness = new Thickness(1),
        Padding = new Thickness(14, 6, 14, 6),
        Shadow = new ThemeShadow(),
        Translation = new Vector3(0, 0, 16),
    };

    private readonly ProgressRing _ring = new()
    {
        IsActive = false,
        Width = 16,
        Height = 16,
        VerticalAlignment = VerticalAlignment.Center,
    };

    /// <summary>The pill, collapsed until <see cref="Show"/> says the period is loading.</summary>
    /// <param name="text">"Loading this period…".</param>
    /// <param name="spoken">Whether the pill is what a screen reader hears. The time grid's spoken
    /// summary already says its week is loading, so its pill stays silent rather than say it twice;
    /// the month has no such node.</param>
    /// <param name="automationId">The id a UI suite finds it by.</param>
    internal CalendarLoadingPill(string text, bool spoken, string automationId)
    {
        HorizontalAlignment = HorizontalAlignment.Center;
        VerticalAlignment = VerticalAlignment.Top;
        Visibility = Visibility.Collapsed;
        // The grid under it stays usable: a press goes through to the day it lands on.
        IsHitTestVisible = false;

        // The words are what is spoken, and what a suite finds: neither a Border nor a UserControl
        // has an automation peer of its own.
        var words = new TextBlock
        {
            Text = text,
            VerticalAlignment = VerticalAlignment.Center,
            Style = (Style)Application.Current.Resources["CaptionTextBlockStyle"],
        };
        AutomationProperties.SetAutomationId(words, automationId);
        if (!spoken)
        {
            AutomationProperties.SetAccessibilityView(words, AccessibilityView.Raw);
        }
        AutomationProperties.SetAccessibilityView(_ring, AccessibilityView.Raw);

        var line = new StackPanel { Orientation = Orientation.Horizontal, Spacing = 8 };
        line.Children.Add(_ring);
        line.Children.Add(words);
        _pill.Child = line;
        Content = _pill;

        ActualThemeChanged += (_, _) => Paint();
        Paint();
    }

    /// <summary>Shows the pill while <paramref name="loading"/>, and hides it once the period is
    /// materialized.</summary>
    internal void Show(bool loading)
    {
        _ring.IsActive = loading;
        Visibility = loading ? Visibility.Visible : Visibility.Collapsed;
    }

    // The flyout's own neutrals, so the pill reads as raised over the grid in either theme.
    private void Paint()
    {
        var dark = ActualTheme == ElementTheme.Dark;
        _pill.Background = new SolidColorBrush(dark ? Color.FromArgb(255, 44, 44, 44) : Color.FromArgb(255, 252, 252, 252));
        _pill.BorderBrush = new SolidColorBrush(dark ? Color.FromArgb(255, 58, 58, 58) : Color.FromArgb(255, 229, 229, 229));
    }
}
