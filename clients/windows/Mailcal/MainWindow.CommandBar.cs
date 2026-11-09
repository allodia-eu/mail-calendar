// The mail command bar under the caption (docs/command-bar.md): New Mail in a slot as wide as the
// folder pane, then the core's answer over the selection, drawn into a WinUI CommandBar whose
// dynamic overflow moves items from the end into its "More" menu when the row cannot hold them.
// The selection the bar acts on is the list's own (docs/list-selection.md, rule 1), and a pick runs
// through the row menu's path over it (MailListView.RowMenu.cs).

using Allodia.Mailcal.Services;
using Allodia.Mailcal.Views;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Automation;
using Microsoft.UI.Xaml.Automation.Peers;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Markup;
using Microsoft.UI.Xaml.Media;
using uniffi.mailcal_bindings;
using Windows.Foundation;

namespace Allodia.Mailcal;

public sealed partial class MainWindow
{
    private bool _commandsWired;

    // The bar as last drawn: each slot's button and the item it stands for, and the slots' ids in
    // order with the dividers, which decides whether an answer can be drawn into the buttons there.
    private readonly List<(AppBarButton Button, CommandItem Item)> _commandButtons = [];
    private string _commandShape = string.Empty;

    /// <summary>
    /// New Mail: the bar's head, and the one item on it that is not the core's: it names no
    /// selection.
    /// </summary>
    /// <remarks>
    /// The draft already open is left first, as every other route into the composer leaves it, so
    /// it is kept in Drafts. The composer takes the reading pane's place, which exists only on the
    /// mail surface, so the mailbox is shown before it opens.
    /// </remarks>
    private async void OnNewMail(object sender, RoutedEventArgs e)
    {
        await LeaveComposerAsync();
        Model.ShowMail();
        ComposeNew();
    }

    private void OnMailCommandsLoaded(object sender, RoutedEventArgs e)
    {
        NameMoreButton();
        if (_commandsWired)
        {
            return;
        }
        _commandsWired = true;
        Model.BarChanged += DrawCommandBar;
        Nav.RegisterPropertyChangedCallback(NavigationView.IsPaneOpenProperty, (_, _) => SizeNewMailSlot());
        Nav.RegisterPropertyChangedCallback(NavigationView.OpenPaneLengthProperty, (_, _) => SizeNewMailSlot());
        ComposeButton.SizeChanged += (_, _) => SizeNewMailSlot();
        MailCommands.DynamicOverflowItemsChanging += OnMailCommandsOverflowChanging;
        SizeNewMailSlot();
        DrawCommandBar();
    }

    /// <summary>
    /// New Mail's slot: as wide as the folder pane, so the actions start at the message list's
    /// leading edge and follow the pane's divider, and never narrower than New Mail itself, so a
    /// compact pane narrows it to the button and the actions follow (docs/command-bar.md, rule 2).
    /// </summary>
    private void SizeNewMailSlot()
    {
        var pane = Nav.IsPaneOpen ? Nav.OpenPaneLength : Nav.CompactPaneLength;
        ComposeButton.Measure(new Size(double.PositiveInfinity, double.PositiveInfinity));
        NewMailColumn.Width = new GridLength(Math.Max(pane, ComposeButton.DesiredSize.Width));
    }

    /// <summary>
    /// Draws the core's bar. An answer of the same shape as the last (the same slots between the
    /// same dividers) is drawn into the buttons already there, so a selection moving relabels and
    /// enables them without the overflow measuring a new set; any other shape is drawn afresh.
    /// </summary>
    private void DrawCommandBar()
    {
        var bar = Model.Bar;
        var shape = string.Join("|", bar.Sections.Select(section =>
            string.Join(",", section.Items.Select(item => CommandBarPresentation.AutomationId(item.Command)))))
            + "#" + string.Join(",", bar.Menu.Select(item => CommandBarPresentation.AutomationId(item.Command)));
        var items = bar.Sections.SelectMany(section => section.Items).Concat(bar.Menu).ToList();
        if (shape == _commandShape)
        {
            for (var i = 0; i < items.Count; i++)
            {
                _commandButtons[i] = (_commandButtons[i].Button, items[i]);
                DrawCommand(_commandButtons[i].Button, items[i]);
            }
            return;
        }
        _commandShape = shape;
        _commandButtons.Clear();
        MailCommands.PrimaryCommands.Clear();
        MailCommands.SecondaryCommands.Clear();
        foreach (var section in bar.Sections)
        {
            if (MailCommands.PrimaryCommands.Count > 0)
            {
                MailCommands.PrimaryCommands.Add(GroupDivider());
            }
            foreach (var item in section.Items)
            {
                MailCommands.PrimaryCommands.Add(CommandButton(item));
            }
        }
        OrderOverflow();
        // What always stands in the overflow follows whatever overflowed; the CommandBar draws the
        // separator between the two itself.
        foreach (var item in bar.Menu)
        {
            MailCommands.SecondaryCommands.Add(CommandButton(item));
        }
    }

    /// <summary>
    /// Items leave the bar from its end, and a divider leaves with the last item of the group before
    /// it, so the overflow keeps the bar's dividers between groups (docs/command-bar.md, rule 4).
    /// </summary>
    private void OrderOverflow()
    {
        var order = 0;
        for (var i = MailCommands.PrimaryCommands.Count - 1; i >= 0; i--)
        {
            if (MailCommands.PrimaryCommands[i] is AppBarButton button)
            {
                button.DynamicOverflowOrder = ++order;
            }
        }
        for (var i = 1; i < MailCommands.PrimaryCommands.Count; i++)
        {
            if (MailCommands.PrimaryCommands[i] is AppBarElementContainer divider
                && MailCommands.PrimaryCommands[i - 1] is AppBarButton before)
            {
                divider.DynamicOverflowOrder = before.DynamicOverflowOrder;
            }
        }
    }

    /// <summary>
    /// The divider between two groups: a line upright on the bar and across the menu. Not an
    /// AppBarSeparator, which the CommandBar never moves into the overflow: it keeps it on the bar
    /// and hides it there, and the groups would run together in the menu.
    /// </summary>
    private static AppBarElementContainer GroupDivider()
    {
        // Loaded from markup, so the brush is a theme resource that follows the window's appearance.
        var line = (Border)XamlReader.Load(
            "<Border xmlns=\"http://schemas.microsoft.com/winfx/2006/xaml/presentation\" "
            + "Background=\"{ThemeResource DividerStrokeColorDefaultBrush}\" />");
        var divider = new AppBarElementContainer
        {
            Content = line,
            IsTabStop = false,
            VerticalContentAlignment = VerticalAlignment.Center,
            HorizontalContentAlignment = HorizontalAlignment.Stretch,
        };
        AutomationProperties.SetAccessibilityView(divider, AccessibilityView.Raw);
        // Moving into the overflow re-parents it, so it loads again there.
        divider.Loaded += (_, _) => ShapeDivider(divider);
        ShapeDivider(divider);
        return divider;
    }

    // Upright on the bar, across the menu. IsInOverflow raises no change of its own when the
    // CommandBar moves an item, so this runs as the item loads in its new place and as the bar's
    // overflow changes.
    private static void ShapeDivider(AppBarElementContainer divider)
    {
        if (divider.Content is not Border line)
        {
            return;
        }
        var across = divider.IsInOverflow;
        divider.Height = across ? 9 : double.NaN;
        divider.MinHeight = 0;
        line.Width = across ? double.NaN : 1;
        line.Height = across ? 1 : 20;
        line.Margin = across ? new Thickness(0, 4, 0, 4) : new Thickness(4, 0, 4, 0);
    }

    private void OnMailCommandsOverflowChanging(Microsoft.UI.Xaml.Controls.CommandBar sender, DynamicOverflowItemsChangingEventArgs args) =>
        DispatcherQueue.TryEnqueue(() =>
        {
            foreach (var divider in MailCommands.PrimaryCommands.OfType<AppBarElementContainer>())
            {
                ShapeDivider(divider);
            }
        });
    private AppBarButton CommandButton(CommandItem item)
    {
        var button = new AppBarButton { Icon = new FontIcon() };
        AutomationProperties.SetAutomationId(button, CommandBarPresentation.AutomationId(item.Command));
        button.Click += OnCommandClick;
        _commandButtons.Add((button, item));
        DrawCommand(button, item);
        return button;
    }

    private void DrawCommand(AppBarButton button, CommandItem item)
    {
        button.Label = CommandLabel(item.Command);
        var icon = (FontIcon)button.Icon;
        icon.Glyph = CommandBarPresentation.Glyph(item.Command);
        // The one irreversible item, in the critical colour as it is in the row menu.
        if (item.Command is Command.Act act && RowMenuPresentation.Critical(act.Action)
            && Application.Current.Resources.TryGetValue("SystemFillColorCriticalBrush", out var critical))
        {
            icon.Foreground = (Brush)critical;
        }
        else
        {
            icon.ClearValue(IconElement.ForegroundProperty);
        }
        button.IsEnabled = MailView.CanRun(item);
    }

    /// <summary>An item's label: the row menu's words for its action, except Move, which on a bar
    /// opens the picker rather than reading as a menu entry.</summary>
    private static string CommandLabel(Command command) => command switch
    {
        Command.Sync => L10n.ActionRefresh(),
        Command.Act { Action: RowAction.MoveToFolder } => L10n.ActionMove(),
        Command.Act act => MailListView.Label(act.Action),
        _ => string.Empty,
    };

    // Sync acts on the mailbox rather than on what is picked, so it never reaches the selection.
    private async void OnCommandClick(object sender, RoutedEventArgs e)
    {
        if (_commandButtons.FirstOrDefault(entry => ReferenceEquals(entry.Button, sender)) is not ({ } _, { } item))
        {
            return;
        }
        if (item.Command is Command.Sync)
        {
            Model.Refresh();
            return;
        }
        await MailView.RunCommandAsync(item);
    }

    /// <summary>
    /// Names the overflow button "More actions", the name every client gives it
    /// (docs/command-bar.md, rule 4). It is the CommandBar's own template part, so it is reached
    /// once the template has been applied.
    /// </summary>
    private void NameMoreButton()
    {
        if (Descendant(MailCommands, "MoreButton") is { } more)
        {
            AutomationProperties.SetName(more, L10n.A11yMoreActions());
            ToolTipService.SetToolTip(more, L10n.A11yMoreActions());
        }
    }

    private static FrameworkElement? Descendant(DependencyObject root, string name)
    {
        for (var i = 0; i < VisualTreeHelper.GetChildrenCount(root); i++)
        {
            var child = VisualTreeHelper.GetChild(root, i);
            if (child is FrameworkElement { Name: var found } element && found == name)
            {
                return element;
            }
            if (Descendant(child, name) is { } deeper)
            {
                return deeper;
            }
        }
        return null;
    }
}
