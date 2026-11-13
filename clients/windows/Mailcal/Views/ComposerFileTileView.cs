// One attached file in the composer, as its tile shows it (docs/composer-layout.md, rule 4): the
// name, truncated in the middle so its extension stays, the size in the app's own words, and the
// Remove control drawn from the tile's own state.

using Allodia.Mailcal.Controls;
using Allodia.Mailcal.Services;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Automation;
using Microsoft.UI.Xaml.Automation.Peers;
using Microsoft.UI.Xaml.Controls;
using uniffi.mailcal_bindings;

namespace Allodia.Mailcal.Views;

/// <summary>A file's tile. A list item to assistive technology, named by the file.</summary>
internal sealed partial class ComposerFileTileView : Grid
{
    internal ComposerFileTileView(ComposerFileTile tile, Action remove)
    {
        Padding = new Thickness(12, 6, 4, 6);
        ColumnSpacing = 4;
        CornerRadius = new CornerRadius(4);
        BorderThickness = new Thickness(1);
        Background = Brush("CardBackgroundFillColorDefaultBrush");
        BorderBrush = Brush("CardStrokeColorDefaultBrush");
        ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(1, GridUnitType.Star) });
        ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });
        AutomationProperties.SetName(this, tile.FileName);
        AutomationProperties.SetAutomationId(this, "ComposerFileTile");
        ToolTipService.SetToolTip(this, tile.FileName);

        var text = new StackPanel { VerticalAlignment = VerticalAlignment.Center };
        text.Children.Add(NameLine(tile.FileName));
        if (tile.Size is { } size)
        {
            text.Children.Add(new TextBlock
            {
                Text = ByteSize.Format(size),
                Style = (Style)Application.Current.Resources["CaptionTextBlockStyle"],
                Foreground = Brush("TextFillColorSecondaryBrush"),
            });
        }
        Children.Add(text);

        var button = ControlComponents.IconButton(
            "",
            // The tile around it names the file, so the button is named by its verb alone rather
            // than by a sentence assembled from two catalog strings.
            L10n.ActionRemove(),
            tile.Remove,
            "ComposerFileRemove",
            remove);
        SetColumn(button, 1);
        Children.Add(button);
    }

    /// <summary>The name in two parts: the stem, which gives way, and the extension, which never
    /// does. WinUI trims only at the end of a line, so this is how a middle truncation is drawn.</summary>
    private static Grid NameLine(string fileName)
    {
        var dot = fileName.LastIndexOf('.');
        var stem = dot > 0 ? fileName[..dot] : fileName;
        var extension = dot > 0 ? fileName[dot..] : string.Empty;
        var line = new Grid();
        line.ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(1, GridUnitType.Auto) });
        line.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });
        var head = new TextBlock { Text = stem, TextTrimming = TextTrimming.CharacterEllipsis, TextWrapping = TextWrapping.NoWrap };
        var tail = new TextBlock { Text = extension, TextWrapping = TextWrapping.NoWrap };
        SetColumn(tail, 1);
        line.Children.Add(head);
        line.Children.Add(tail);
        // The stem may take what the extension leaves, and no more: an Auto column would let a long
        // stem push the extension out of the tile.
        line.SizeChanged += (_, e) =>
        {
            tail.Measure(new Windows.Foundation.Size(double.PositiveInfinity, double.PositiveInfinity));
            head.MaxWidth = Math.Max(0, e.NewSize.Width - tail.DesiredSize.Width);
        };
        return line;
    }

    private static Microsoft.UI.Xaml.Media.Brush Brush(string key) =>
        (Microsoft.UI.Xaml.Media.Brush)Application.Current.Resources[key];

    protected override AutomationPeer OnCreateAutomationPeer() => new TilePeer(this);

    private sealed partial class TilePeer(ComposerFileTileView owner) : FrameworkElementAutomationPeer(owner)
    {
        protected override AutomationControlType GetAutomationControlTypeCore() => AutomationControlType.ListItem;

        protected override string GetClassNameCore() => nameof(ComposerFileTileView);
    }
}
