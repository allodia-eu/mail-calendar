// The frame the two Writing style sheets share (docs/ai.md, "Learning"): a page, a row above it for
// the reveal's language control, and a footer with Cancel, the pips, Back and Next. It renders
// inside the Settings dialog's detail panel, because WinUI forbids a nested ContentDialog.
//
// A page arrives from the side the person moved towards, and nothing moves while Windows' own
// animation setting is off. Back keeps its place while hidden, so Next does not move under the
// pointer. Where the frame stands is WizardPager, which Mailcal.Tests pins.

using System.Globalization;
using Allodia.Mailcal.Services;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Automation;
using Microsoft.UI.Xaml.Automation.Peers;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Media.Animation;
using Windows.UI.ViewManagement;

namespace Allodia.Mailcal.Dialogs;

/// <summary>A stepped sheet: one page at a time, with the footer every step shares.</summary>
internal sealed class WizardFrame
{
    // How far a page travels on its way in: a hint of direction, not a slide across the panel.
    private const double Travel = 40;

    private readonly WizardPager _pager;
    private readonly Func<WizardFrame, int, UIElement> _page;
    private readonly bool _allowsJump;
    private readonly bool _motion = new UISettings().AnimationsEnabled;
    private readonly ContentControl _stage = new()
    {
        IsTabStop = false,
        HorizontalContentAlignment = HorizontalAlignment.Stretch,
        VerticalContentAlignment = VerticalAlignment.Stretch,
    };
    private readonly Grid _top = new() { Visibility = Visibility.Collapsed };
    private readonly Button _cancel = new();
    private readonly Button _back = new() { Content = L10n.WizardBack() };
    private readonly Button _primary = new();
    private readonly PipsPager _pips;
    private Action? _cancelAction;
    private Action? _primaryAction;
    private UIElement? _topControl;
    private bool _syncing;

    /// <summary>A frame over <paramref name="pager"/>'s pages, each built by <paramref name="page"/>
    /// as it comes on screen.</summary>
    /// <param name="pager">Where the frame stands; the caller keeps it, so a rebuild keeps the page.</param>
    /// <param name="height">The height of the panel the frame fills.</param>
    /// <param name="allowsJump">Whether a pip moves to its page: only where every page can be
    /// reached in any order.</param>
    /// <param name="page">Builds the page at an index, in this frame.</param>
    internal WizardFrame(WizardPager pager, double height, bool allowsJump, Func<WizardFrame, int, UIElement> page)
    {
        _pager = pager;
        _page = page;
        _allowsJump = allowsJump;
        _pips = new PipsPager
        {
            NumberOfPages = pager.Count,
            MaxVisiblePips = pager.Count,
            PreviousButtonVisibility = PipsPagerButtonVisibility.Collapsed,
            NextButtonVisibility = PipsPagerButtonVisibility.Collapsed,
            HorizontalAlignment = HorizontalAlignment.Center,
            VerticalAlignment = VerticalAlignment.Center,
            IsHitTestVisible = allowsJump,
        };
        _pips.SelectedIndexChanged += (_, _) => OnPip();
        // The pager's template names it "Pager" when applied, over a name set before it loaded,
        // so the first page's step is named again once it has.
        _pips.Loaded += (_, _) => NamePips();
        _cancel.Click += (_, _) => _cancelAction?.Invoke();
        _back.Click += (_, _) => Go(_pager.Index - 1);
        _primary.Click += (_, _) =>
        {
            if (_primaryAction is { } action)
            {
                action();
            }
            else
            {
                Go(_pager.Index + 1);
            }
        };
        _cancel.Visibility = Visibility.Collapsed;

        Root = new Grid { Height = height };
        Root.RowDefinitions.Add(new RowDefinition { Height = GridLength.Auto });
        Root.RowDefinitions.Add(new RowDefinition { Height = new GridLength(1, GridUnitType.Star) });
        Root.RowDefinitions.Add(new RowDefinition { Height = GridLength.Auto });
        Grid.SetRow(_stage, 1);
        Root.Children.Add(_top);
        Root.Children.Add(_stage);
        var footer = Footer();
        Grid.SetRow(footer, 2);
        Root.Children.Add(footer);
    }

    /// <summary>The frame, to put in the panel.</summary>
    internal Grid Root { get; }

    /// <summary>Called whenever a page comes on screen, after the footer has been put back to Back
    /// and Next, so the owner can set it for that page.</summary>
    internal Action? Moved { get; set; }

    /// <summary>A page: its heading, then whatever the caller adds beneath it.</summary>
    internal static StackPanel Page(string title)
    {
        var page = new StackPanel { Spacing = 14 };
        var heading = new TextBlock
        {
            Text = title,
            Style = (Style)Application.Current.Resources["SubtitleTextBlockStyle"],
            TextWrapping = TextWrapping.Wrap,
        };
        AutomationProperties.SetHeadingLevel(heading, AutomationHeadingLevel.Level2);
        page.Children.Add(heading);
        return page;
    }

    /// <summary>Draws the page the pager is on, without motion.</summary>
    internal void Show() => Present(motion: false);

    /// <summary>Moves to <paramref name="target"/>, held to the pages there are.</summary>
    internal void Go(int target)
    {
        if (_pager.Go(target))
        {
            Present(_motion);
        }
    }

    /// <summary>Puts <paramref name="title"/> at the start of the footer, doing <paramref name="action"/>.</summary>
    internal void SetCancel(string title, Action action)
    {
        _cancel.Content = title;
        _cancel.Visibility = Visibility.Visible;
        _cancelAction = action;
    }

    /// <summary>Takes Cancel away, for a page whose only way out is its own button.</summary>
    internal void HideCancel()
    {
        _cancel.Visibility = Visibility.Collapsed;
        _cancelAction = null;
    }

    /// <summary>Shows or hides Back, which keeps its place either way.</summary>
    internal void SetBack(bool shown)
    {
        if (!shown && _back.FocusState != FocusState.Unfocused)
        {
            _primary.Focus(FocusState.Programmatic);
        }
        _back.Opacity = shown ? 1 : 0;
        _back.IsEnabled = shown;
        AutomationProperties.SetAccessibilityView(_back, shown ? AccessibilityView.Content : AccessibilityView.Raw);
    }

    /// <summary>The caller's own action where Next stands: Save, Learn, Stop, Close.</summary>
    internal void SetAction(string title, bool enabled, bool accent, Action action)
    {
        SetPrimary(title, enabled, accent);
        _primaryAction = action;
    }

    /// <summary>Enables or disables whatever stands where Next does.</summary>
    internal void EnablePrimary(bool enabled) => _primary.IsEnabled = enabled;

    /// <summary>Nothing where Next stands. Focus on it moves to Back, rather than out of the frame.</summary>
    internal void HidePrimary()
    {
        if (_primary.FocusState != FocusState.Unfocused && _back.IsEnabled)
        {
            _back.Focus(FocusState.Programmatic);
        }
        _primary.Visibility = Visibility.Collapsed;
        _primaryAction = null;
    }

    /// <summary>
    /// The control above the page: the reveal's language choice. The row keeps its height whether
    /// or not the control is shown, so the page below does not jump as the steps change.
    /// </summary>
    internal void SetTop(FrameworkElement control)
    {
        control.HorizontalAlignment = HorizontalAlignment.Center;
        _top.Children.Clear();
        _top.Children.Add(control);
        _top.Visibility = Visibility.Visible;
        _top.Margin = new Thickness(0, 0, 0, 8);
        _top.MinHeight = 40;
        _topControl = control;
    }

    /// <summary>Shows or hides the control above the page.</summary>
    internal void ShowTop(bool shown)
    {
        if (_topControl is { } control)
        {
            control.Visibility = shown ? Visibility.Visible : Visibility.Collapsed;
        }
    }

    private Grid Footer()
    {
        var footer = new Grid { ColumnSpacing = 8, Margin = new Thickness(0, 12, 0, 0) };
        footer.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });
        footer.ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(1, GridUnitType.Star) });
        footer.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });
        var moves = new StackPanel { Orientation = Orientation.Horizontal, Spacing = 8 };
        moves.Children.Add(_back);
        moves.Children.Add(_primary);
        Grid.SetColumn(_pips, 1);
        Grid.SetColumn(moves, 2);
        footer.Children.Add(_cancel);
        footer.Children.Add(_pips);
        footer.Children.Add(moves);
        return footer;
    }

    private void Present(bool motion)
    {
        var transitions = new TransitionCollection();
        if (motion)
        {
            transitions.Add(new ContentThemeTransition
            {
                HorizontalOffset = _pager.Forward ? Travel : -Travel,
                VerticalOffset = 0,
            });
        }
        _stage.ContentTransitions = transitions;
        _stage.Content = new ScrollViewer
        {
            Content = _page(this, _pager.Index),
            Padding = new Thickness(0, 4, 16, 12),
        };
        _syncing = true;
        _pips.SelectedPageIndex = _pager.Index;
        _syncing = false;
        NamePips();
        SetBack(!_pager.IsFirst);
        SetPrimary(L10n.WizardNext(), enabled: true, accent: true);
        _primaryAction = null;
        Moved?.Invoke();
    }

    private void NamePips() => AutomationProperties.SetName(_pips, L10n.A11yWizardStep(
        (_pager.Index + 1).ToString(CultureInfo.CurrentCulture),
        _pager.Count.ToString(CultureInfo.CurrentCulture)));

    // A pip moves to its page where the frame allows it; elsewhere the selection goes back to the
    // page on screen, which a keyboard can still reach the pips to change.
    private void OnPip()
    {
        if (_syncing || _pips.SelectedPageIndex == _pager.Index)
        {
            return;
        }
        if (_allowsJump)
        {
            Go(_pips.SelectedPageIndex);
            return;
        }
        _syncing = true;
        _pips.SelectedPageIndex = _pager.Index;
        _syncing = false;
    }

    private void SetPrimary(string title, bool enabled, bool accent)
    {
        _primary.Content = title;
        _primary.IsEnabled = enabled;
        _primary.Visibility = Visibility.Visible;
        if (accent)
        {
            _primary.Style = (Style)Application.Current.Resources["AccentButtonStyle"];
        }
        else
        {
            _primary.ClearValue(FrameworkElement.StyleProperty);
        }
    }
}
