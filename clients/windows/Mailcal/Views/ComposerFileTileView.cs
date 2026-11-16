// One attached file in the composer, as its tile shows it (docs/composer-layout.md, rule 4): a
// picture of it when it is one, the name cut in the middle so its end and its extension stay, the
// size in the app's own words, and the Remove control drawn from the tile's own state. One line,
// so a forward's twenty files take a few rows rather than the window.

using Allodia.Mailcal.Controls;
using Allodia.Mailcal.Services;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Automation;
using Microsoft.UI.Xaml.Automation.Peers;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Media;
using Microsoft.UI.Xaml.Media.Imaging;
using uniffi.mailcal_bindings;

namespace Allodia.Mailcal.Views;

/// <summary>A file's tile. A list item to assistive technology, named by the file.</summary>
internal sealed partial class ComposerFileTileView : Grid
{
    /// <summary>The side of the picture or glyph at the start of the tile, in epx.</summary>
    private const double PreviewSide = 28;

    internal ComposerFileTileView(ComposerFileTile tile, Action remove)
    {
        Padding = new Thickness(6, 4, 2, 4);
        MinHeight = 40;
        ColumnSpacing = 8;
        CornerRadius = new CornerRadius(4);
        BorderThickness = new Thickness(1);
        Background = Brush("CardBackgroundFillColorDefaultBrush");
        BorderBrush = Brush("CardStrokeColorDefaultBrush");
        ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });
        ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(1, GridUnitType.Star) });
        ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });
        ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });
        AutomationProperties.SetName(this, tile.FileName);
        AutomationProperties.SetAutomationId(this, "ComposerFileTile");
        ToolTipService.SetToolTip(this, tile.FileName);

        Children.Add(Preview(tile.Preview));

        var name = new MiddleTrimmedText(tile.FileName) { VerticalAlignment = VerticalAlignment.Center };
        SetColumn(name, 1);
        Children.Add(name);

        if (tile.Size is { } size)
        {
            var words = new TextBlock
            {
                Text = ByteSize.Format(size),
                VerticalAlignment = VerticalAlignment.Center,
                Style = (Style)Application.Current.Resources["CaptionTextBlockStyle"],
                Foreground = Brush("TextFillColorSecondaryBrush"),
            };
            SetColumn(words, 2);
            Children.Add(words);
        }

        var button = ControlComponents.IconButton(
            "",
            // The tile around it names the file, so the button is named by its verb alone rather
            // than by a sentence assembled from two catalog strings.
            L10n.ActionRemove(),
            tile.Remove,
            "ComposerFileRemove",
            remove);
        SetColumn(button, 3);
        Children.Add(button);
    }

    /// <summary>The picture, shrunk, when the core found the file to be one; the file glyph until
    /// it has loaded, and for good when it cannot be (docs/icons.md, "An attached file with no
    /// picture to show"). Decorative: the tile is named by the file.</summary>
    private static Border Preview(string? path)
    {
        var frame = new Border
        {
            Width = PreviewSide,
            Height = PreviewSide,
            CornerRadius = new CornerRadius(4),
            VerticalAlignment = VerticalAlignment.Center,
            Background = Brush("SubtleFillColorSecondaryBrush"),
            Child = new FontIcon { Glyph = "", FontSize = 14 },
        };
        AutomationProperties.SetAccessibilityView(frame, AccessibilityView.Raw);
        if (path is not null)
        {
            LoadPicture(frame, path);
        }
        return frame;
    }

    /// <summary>Decodes the picture at a few times the tile's size, never at its own: a forward of
    /// twenty photos would otherwise hold twenty full images for twenty small squares.</summary>
    private static async void LoadPicture(Border frame, string path)
    {
        try
        {
            var bitmap = new BitmapImage
            {
                DecodePixelType = DecodePixelType.Logical,
                DecodePixelWidth = (int)(PreviewSide * 2),
            };
            // Opened off the UI thread: the file may sit on a network share.
            await using var file = await Task.Run(() => new FileStream(
                path, FileMode.Open, FileAccess.Read, FileShare.ReadWrite, 4096, useAsync: true));
            using var stream = file.AsRandomAccessStream();
            await bitmap.SetSourceAsync(stream);
            frame.Child = new Image { Source = bitmap, Stretch = Stretch.UniformToFill };
        }
        catch (Exception ex)
        {
            // An async void method: a throw here would take the app down with it. The glyph stays.
            Log.Warn($"composer: a file's picture could not be shown ({ex.GetType().Name})");
        }
    }

    private static Brush Brush(string key) => (Brush)Application.Current.Resources[key];

    protected override AutomationPeer OnCreateAutomationPeer() => new TilePeer(this);

    private sealed partial class TilePeer(ComposerFileTileView owner) : FrameworkElementAutomationPeer(owner)
    {
        protected override AutomationControlType GetAutomationControlTypeCore() => AutomationControlType.ListItem;

        protected override string GetClassNameCore() => nameof(ComposerFileTileView);
    }
}
