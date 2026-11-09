// An account's own folders take the account's columns (docs/folder-pane.md, rule 32): every row
// leads with its disclosure control, a top-level folder's icon stands under its account's and only
// a folder inside a folder steps in, one indent per level.
//
// The NavigationView indents each level itself, one step per level of the tree, by setting the left
// margin of its item presenter's "PresenterContentRootGrid" (which holds the selection indicator,
// the icon and the label) from code whenever an item is realised at a depth.
// Neither a style nor the item's padding reaches that margin, so it is watched: each time the
// framework sets it, the item gets one step less. The step is read off the framework's own margin
// rather than written here, so a WinUI that changes it moves this with it.
//
// The presenter draws its chevron in the last column of its "ContentGrid", and loads it only once
// the item has children. Every row gets a leading column of its own, so a row without a chevron
// keeps the blank and the icons line up down the pane, and the chevron is moved into it whenever
// the presenter's "ChevronStates" bring it in.

using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Media;

namespace Allodia.Mailcal.Views;

internal static class SidebarAccountColumns
{
    // The leading column the disclosure control stands in, on every row whether it has one or not.
    private const double DisclosureWidth = 24;

    // The framework's indent per level, as each presenter's grid first showed it.
    private static readonly System.Runtime.CompilerServices.ConditionalWeakTable<FrameworkElement, StepHolder> Steps = new();

    // The content grids already given their leading column; a re-templated presenter is a new grid.
    private static readonly System.Runtime.CompilerServices.ConditionalWeakTable<Grid, object> Led = new();

    private sealed class StepHolder
    {
        internal double? Step;
    }

    /// <summary>Leads <paramref name="item"/> with its disclosure column and takes one level of
    /// indent off it, now and whenever the framework indents it again.</summary>
    internal static void Apply(NavigationViewItem item)
    {
        LeadWithDisclosure(item);
        if (Find(item, "PresenterContentRootGrid") is not { } grid)
        {
            return;
        }
        if (!Steps.TryGetValue(grid, out var holder))
        {
            holder = new StepHolder();
            Steps.Add(grid, holder);
            grid.RegisterPropertyChangedCallback(FrameworkElement.MarginProperty, (_, _) => Shift(item, grid, holder));
        }
        Shift(item, grid, holder);
    }

    private static void LeadWithDisclosure(NavigationViewItem item)
    {
        if (Find(item, "ContentGrid") is not Grid grid)
        {
            return;
        }
        if (!Led.TryGetValue(grid, out _))
        {
            Led.Add(grid, new object());
            grid.ColumnDefinitions.Insert(0, new ColumnDefinition { Width = new GridLength(DisclosureWidth) });
            foreach (var child in grid.Children)
            {
                if (!IsChevron(child))
                {
                    Grid.SetColumn((FrameworkElement)child, Grid.GetColumn((FrameworkElement)child) + 1);
                }
            }
            if (ChevronStates(grid) is { } states)
            {
                states.CurrentStateChanged += (_, _) => PlaceChevron(grid);
            }
        }
        PlaceChevron(grid);
    }

    // The chevron, once loaded, into the leading column, filling it so the whole column is the target.
    private static void PlaceChevron(Grid grid)
    {
        foreach (var child in grid.Children)
        {
            if (IsChevron(child) && child is FrameworkElement chevron && Grid.GetColumn(chevron) != 0)
            {
                Grid.SetColumn(chevron, 0);
                chevron.Width = DisclosureWidth;
                chevron.Margin = default;
                chevron.HorizontalAlignment = HorizontalAlignment.Stretch;
            }
        }
    }

    private static bool IsChevron(UIElement element) =>
        element is FrameworkElement { Name: "ExpandCollapseChevron" };

    // The presenter's visual state group that loads and shows the chevron, on its template root.
    private static VisualStateGroup? ChevronStates(DependencyObject from)
    {
        for (var node = VisualTreeHelper.GetParent(from); node is not null and not NavigationViewItem; node = VisualTreeHelper.GetParent(node))
        {
            if (node is FrameworkElement element)
            {
                foreach (var group in VisualStateManager.GetVisualStateGroups(element))
                {
                    if (group.Name == "ChevronStates")
                    {
                        return group;
                    }
                }
            }
        }
        return null;
    }

    private static void Shift(NavigationViewItem item, FrameworkElement grid, StepHolder holder)
    {
        var depth = Depth(item);
        var margin = grid.Margin;
        if (depth == 0)
        {
            return;
        }
        // An item at depth d is indented d steps; what is there before the first shift is the
        // framework's own.
        holder.Step ??= margin.Left > 0 ? margin.Left / depth : null;
        if (holder.Step is not { } step)
        {
            return;
        }
        var left = step * (depth - 1);
        if (margin.Left != left)
        {
            grid.Margin = new Thickness(left, margin.Top, margin.Right, margin.Bottom);
        }
    }

    // How many items this one is nested under: its children are drawn inside its own template.
    private static int Depth(DependencyObject item)
    {
        var depth = 0;
        for (var parent = VisualTreeHelper.GetParent(item); parent is not null and not NavigationView; parent = VisualTreeHelper.GetParent(parent))
        {
            if (parent is NavigationViewItem)
            {
                depth++;
            }
        }
        return depth;
    }

    // The named part of this item's own template, never of an item nested inside it.
    private static FrameworkElement? Find(DependencyObject root, string name)
    {
        for (var i = 0; i < VisualTreeHelper.GetChildrenCount(root); i++)
        {
            var child = VisualTreeHelper.GetChild(root, i);
            if (child is NavigationViewItem)
            {
                continue;
            }
            if (child is FrameworkElement { Name: var found } element && found == name)
            {
                return element;
            }
            if (Find(child, name) is { } deeper)
            {
                return deeper;
            }
        }
        return null;
    }
}
