// Where Move to… sends a folder and Move to folder… sends mail (docs/folder-pane.md, rule 24):
// the account's tree drawn as the pane draws it, each folder by its own name beside its role's
// glyph and indented under the folder it sits in. Which rows are offered is FolderActions'.

using Allodia.Mailcal.Services;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Automation;
using Microsoft.UI.Xaml.Controls;

namespace Allodia.Mailcal.Dialogs;

internal static partial class FolderDialogs
{
    /// <summary>How far one level of the tree steps in: the glyph's width and the gap after it, so
    /// a folder's glyph sits under its parent's name.</summary>
    private const double IndentStep = GlyphSize + GlyphGap;

    private const double GlyphSize = 16;
    private const double GlyphGap = 12;

    /// <summary>
    /// Asks where a folder or a message goes and returns the chosen destination, or <c>null</c>
    /// when the user cancelled. Choosing a row answers at once: there is nothing else to decide,
    /// and a separate confirm button would need a verb the list already is.
    /// </summary>
    public static async Task<MoveTarget?> AskMoveTargetAsync(
        XamlRoot root,
        string title,
        IReadOnlyList<MoveTarget> targets)
    {
        MoveTarget? chosen = null;
        var list = new ListView
        {
            SelectionMode = ListViewSelectionMode.None,
            IsItemClickEnabled = true,
            MaxHeight = 420,
            MinWidth = 360,
        };
        foreach (var target in targets)
        {
            list.Items.Add(TargetRow(target));
        }
        AutomationProperties.SetName(list, title);
        AutomationProperties.SetAutomationId(list, "FolderMoveTargets");
        var dialog = new ContentDialog
        {
            XamlRoot = root,
            Title = title,
            Content = list,
            CloseButtonText = L10n.ActionCancel(),
            // Choosing a row is the answer, so Cancel is not the one to draw the eye.
            DefaultButton = ContentDialogButton.None,
        };
        list.ItemClick += (_, args) =>
        {
            // Matched by reference, never by the words: two destinations can read alike (a folder
            // called "Top level" at the top of the tree), and a match on the words would move it
            // wrong.
            if (args.ClickedItem is FrameworkElement { Tag: MoveTarget { Enabled: true } target })
            {
                chosen = target;
                dialog.Hide();
            }
        };
        _ = await DialogHelper.ShowAsync(dialog);
        return chosen;
    }

    /// <summary>One row: the account glyph for the top level, the role's glyph for a folder, as the
    /// pane draws them. A folder that only holds a destination is disabled, which
    /// keeps it out of the click and the tab order, and drawn at half strength.</summary>
    private static ListViewItem TargetRow(MoveTarget target)
    {
        var content = new StackPanel
        {
            Orientation = Orientation.Horizontal,
            Spacing = GlyphGap,
            Margin = new Thickness(target.Indent * IndentStep, 0, 0, 0),
            VerticalAlignment = VerticalAlignment.Center,
            // The pane's pending row is drawn at half strength too; a disabled item's own style
            // leaves custom content at full strength.
            Opacity = target.Enabled ? 1 : 0.5,
            Tag = target,
        };
        content.Children.Add(new FontIcon
        {
            Glyph = target.Key is null ? MainWindow.Glyphs.Account : MainWindow.RoleGlyph(target.Role),
            FontSize = GlyphSize,
            VerticalAlignment = VerticalAlignment.Center,
        });
        content.Children.Add(new TextBlock
        {
            Text = target.Name,
            TextTrimming = TextTrimming.CharacterEllipsis,
            VerticalAlignment = VerticalAlignment.Center,
        });
        var row = new ListViewItem
        {
            Content = content,
            Tag = target,
            IsEnabled = target.Enabled,
            MinHeight = 36,
            HorizontalContentAlignment = HorizontalAlignment.Stretch,
        };
        AutomationProperties.SetName(row, target.Label);
        return row;
    }
}
