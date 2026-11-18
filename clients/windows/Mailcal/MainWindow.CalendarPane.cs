// The calendar pane in the folder pane's column (docs/calendar-pane.md, rule 1). While the calendar
// is showing, the NavigationView's menu of accounts and folders steps aside and the pane takes its
// height, above the destinations it pins (rule 10), so the column is still the one pane with the
// splitter and the width the folder pane has.
//
// The pane is the NavigationView's PaneFooter, which the template places between the menu and the
// footer items, sized to an Auto row; the menu's own row takes whatever is left. So the pane is
// given that whole height by hand, measured from the template, and the menu is hidden while it
// shows. The accounts and folders are untouched underneath, expanded as the person left them, for
// mail to bring back.

using Allodia.Mailcal.Services;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Media;

namespace Allodia.Mailcal;

public sealed partial class MainWindow
{
    // The template parts the pane is measured against, found once the template is applied.
    private FrameworkElement? _paneItems;
    private FrameworkElement? _paneMenu;
    private FrameworkElement? _paneFooterItems;
    private FrameworkElement? _paneSeparator;

    // The bottom margin the template puts on the PaneFooter's presenter.
    private const double PaneFooterMargin = 4;

    /// <summary>Wires the pane to the calendar and finds where it is to fit.</summary>
    private void InitCalendarPane()
    {
        CalendarPaneView.Init(Model, CalendarDetail);
        Nav.Loaded += (_, _) => FitCalendarPane();
        Nav.SizeChanged += (_, _) => FitCalendarPane();
        Model.PropertyChanged += (_, e) =>
        {
            if (e.PropertyName == nameof(MailboxModel.Destination))
            {
                FitCalendarPane();
            }
        };
    }

    /// <summary>Shows the pane in place of the menu on the calendar, the menu everywhere else, and
    /// gives the pane every pixel between the top of the menu and the destinations.</summary>
    private void FitCalendarPane()
    {
        FindPaneParts();
        var calendar = Model.Destination == AppDestination.Calendar;
        CalendarPaneView.Visibility = calendar ? Visibility.Visible : Visibility.Collapsed;
        if (_paneMenu is not null)
        {
            _paneMenu.Visibility = calendar ? Visibility.Collapsed : Visibility.Visible;
        }
        if (!calendar || _paneItems is null || _paneFooterItems is null)
        {
            return;
        }
        var separator = _paneSeparator is { Visibility: Visibility.Visible } line
            ? line.ActualHeight + line.Margin.Top + line.Margin.Bottom
            : 0;
        var height = _paneItems.ActualHeight - _paneFooterItems.ActualHeight - separator - PaneFooterMargin;
        // Height starts as NaN, which compares unequal to nothing, so it is tested for first.
        if (height > 0 && (double.IsNaN(CalendarPaneView.Height) || Math.Abs(CalendarPaneView.Height - height) > 0.5))
        {
            CalendarPaneView.Height = height;
        }
    }

    // The template parts, looked for until they are there: a launch that boots the core first shows
    // the NavigationView only afterwards, and until then its template has drawn nothing to find.
    private void FindPaneParts()
    {
        if (_paneItems is not null)
        {
            return;
        }
        if (Part(Nav, "ItemsContainerGrid") is not { } items
            || Part(Nav, "MenuItemsScrollViewer") is not { } menu
            || Part(Nav, "FooterItemsScrollViewer") is not { } footer)
        {
            return;
        }
        _paneItems = items;
        _paneMenu = VisualTreeHelper.GetParent(menu) as FrameworkElement ?? menu;
        _paneFooterItems = VisualTreeHelper.GetParent(footer) as FrameworkElement ?? footer;
        _paneSeparator = Part(Nav, "VisualItemsSeparator");
        _paneItems.SizeChanged += (_, _) => FitCalendarPane();
        _paneFooterItems.SizeChanged += (_, _) => FitCalendarPane();
    }

    // The first element called `name` under `root`, or null.
    private static FrameworkElement? Part(DependencyObject root, string name)
    {
        var count = VisualTreeHelper.GetChildrenCount(root);
        for (var i = 0; i < count; i++)
        {
            var child = VisualTreeHelper.GetChild(root, i);
            if (child is FrameworkElement { Name: var found } element && found == name)
            {
                return element;
            }
            if (Part(child, name) is { } deeper)
            {
                return deeper;
            }
        }
        return null;
    }
}
