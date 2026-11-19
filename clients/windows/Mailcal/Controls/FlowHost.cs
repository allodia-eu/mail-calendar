// The one host for a multi-step flow on Windows (docs/ai.md, "The flow host"): a title, the stage
// and a footer of Close, Back and the primary in the accent style, with a PipsPager between them
// when the record gives a step count. It renders inside the Settings dialog's detail panel,
// because WinUI forbids a nested ContentDialog.
//
// It draws the frame it is given (Services/FlowFrame.cs) and keeps no step of its own: a stage is
// the caller's element, and the same element handed back is left where it is, so a redraw for a
// control's state does not take what is typed in it. A stage arrives from the side the flow moved
// towards, and nothing moves while Windows' own animation setting is off.

using Allodia.Mailcal.Services;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Automation;
using Microsoft.UI.Xaml.Automation.Peers;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Media.Animation;
using Windows.UI.ViewManagement;

namespace Allodia.Mailcal.Controls;

/// <summary>A flow's stage under its title, over the footer every stage shares.</summary>
internal sealed class FlowHost
{
    // How far a stage travels on its way in: a hint of direction, not a slide across the panel.
    private const double Travel = 40;

    // A footer label wider than this wraps between words, so the largest text size still fits.
    private const double LabelWidth = 200;

    private readonly string _automationId;
    private readonly bool _motion = new UISettings().AnimationsEnabled;
    private readonly TextBlock _title = new()
    {
        Style = (Style)Application.Current.Resources["SubtitleTextBlockStyle"],
        TextWrapping = TextWrapping.Wrap,
    };
    private readonly ContentControl _stage = new()
    {
        IsTabStop = false,
        HorizontalContentAlignment = HorizontalAlignment.Stretch,
        VerticalContentAlignment = VerticalAlignment.Stretch,
    };
    private readonly Border _close = new();
    private readonly StackPanel _moves = new() { Orientation = Orientation.Horizontal, Spacing = 8 };
    private readonly PipsPager _pips = new()
    {
        PreviousButtonVisibility = PipsPagerButtonVisibility.Collapsed,
        NextButtonVisibility = PipsPagerButtonVisibility.Collapsed,
        HorizontalAlignment = HorizontalAlignment.Center,
        VerticalAlignment = VerticalAlignment.Center,
    };
    private FlowSteps? _steps;
    private bool _drawingPips;

    /// <summary>A host whose parts are named after <paramref name="automationId"/>.</summary>
    /// <param name="height">The height of the panel the host fills.</param>
    internal FlowHost(string automationId, double height)
    {
        _automationId = automationId;
        AutomationProperties.SetHeadingLevel(_title, AutomationHeadingLevel.Level2);
        AutomationProperties.SetAutomationId(_title, automationId + "Title");
        AutomationProperties.SetAutomationId(_stage, automationId + "Stage");
        AutomationProperties.SetAutomationId(_pips, automationId + "Steps");
        _pips.SelectedIndexChanged += (_, _) => OnPip();
        // The pager's template names it when applied, over a name set before it loaded.
        _pips.Loaded += (_, _) => NamePips();

        Root = new Grid { Height = height, RowSpacing = 12 };
        Root.RowDefinitions.Add(new RowDefinition { Height = GridLength.Auto });
        Root.RowDefinitions.Add(new RowDefinition { Height = new GridLength(1, GridUnitType.Star) });
        Root.RowDefinitions.Add(new RowDefinition { Height = GridLength.Auto });
        Grid.SetRow(_stage, 1);
        Root.Children.Add(_title);
        Root.Children.Add(_stage);
        var footer = new Grid { ColumnSpacing = 8 };
        footer.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });
        footer.ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(1, GridUnitType.Star) });
        footer.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });
        Grid.SetColumn(_pips, 1);
        Grid.SetColumn(_moves, 2);
        footer.Children.Add(_close);
        footer.Children.Add(_pips);
        footer.Children.Add(_moves);
        Grid.SetRow(footer, 2);
        Root.Children.Add(footer);
    }

    /// <summary>The host, to put in the panel.</summary>
    internal Grid Root { get; }

    /// <summary>
    /// Draws <paramref name="frame"/> over <paramref name="stage"/>. The stage on screen stays
    /// when it is the same element, and otherwise comes in from the side the flow moved towards.
    /// </summary>
    internal void Draw(FlowFrame frame, UIElement stage)
    {
        _title.Text = frame.Title;
        var arrival = FlowFooter.Arrival(_steps, frame.Steps);
        _steps = frame.Steps;
        if (_stage.Content is not ScrollViewer { Content: var shown } || !ReferenceEquals(shown, stage))
        {
            Present(stage, _motion ? arrival : FlowArrival.InPlace);
        }
        DrawFooter(frame);
        DrawPips();
    }

    private void Present(UIElement stage, FlowArrival arrival)
    {
        var transitions = new TransitionCollection();
        if (arrival != FlowArrival.InPlace)
        {
            transitions.Add(new ContentThemeTransition
            {
                HorizontalOffset = arrival == FlowArrival.FromEnd ? Travel : -Travel,
                VerticalOffset = 0,
            });
        }
        _stage.ContentTransitions = transitions;
        // A stage handed back later, as a page whose fields keep what is typed, may have only one
        // parent.
        if (_stage.Content is ScrollViewer previous)
        {
            previous.Content = null;
        }
        _stage.Content = new ScrollViewer { Content = stage, Padding = new Thickness(0, 4, 16, 12) };
    }

    private void DrawFooter(FlowFrame frame)
    {
        _close.Child = null;
        _moves.Children.Clear();
        foreach (var (role, action) in FlowFooter.Controls(frame))
        {
            var control = ControlComponents.Button(
                action.Label,
                action.State,
                $"{_automationId}{role}",
                action.Press,
                role == FlowRole.Primary ? (Style)Application.Current.Resources["AccentButtonStyle"] : null,
                LabelWidth);
            if (role == FlowRole.Close)
            {
                _close.Child = control;
            }
            else
            {
                _moves.Children.Add(control);
            }
        }
    }

    private void DrawPips()
    {
        _drawingPips = true;
        if (_steps is { } steps)
        {
            _pips.NumberOfPages = steps.Count;
            _pips.MaxVisiblePips = steps.Count;
            _pips.SelectedPageIndex = steps.Current;
            _pips.IsHitTestVisible = steps.Go is not null;
            _pips.Visibility = Visibility.Visible;
        }
        else
        {
            _pips.Visibility = Visibility.Collapsed;
        }
        _drawingPips = false;
        NamePips();
        // A pager made visible applies its template on the next layout, and the template names it
        // over a name set before, so it is named again once that has happened.
        _pips.DispatcherQueue?.TryEnqueue(Microsoft.UI.Dispatching.DispatcherQueuePriority.Low, NamePips);
    }

    private void NamePips()
    {
        if (_steps is { } steps)
        {
            AutomationProperties.SetName(_pips, steps.Name);
        }
    }

    // A pip moves to its page where the record allows it; elsewhere the selection goes back to the
    // step on screen, which a keyboard can still reach the pips to change.
    private void OnPip()
    {
        if (_drawingPips || _steps is not { } steps || _pips.SelectedPageIndex == steps.Current)
        {
            return;
        }
        if (steps.Go is { } go)
        {
            go(_pips.SelectedPageIndex);
            return;
        }
        _drawingPips = true;
        _pips.SelectedPageIndex = steps.Current;
        _drawingPips = false;
    }
}
