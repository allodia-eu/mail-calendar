// The circle a row draws beside a person: their photo when a synced address book has one, else
// their initials on the colour the core picked for them (docs/avatars.md).
//
// **Only the shape is decided here.** The letters and the colour come from the core, for the same
// reason a calendar chip's contrast does: resolved per client, four clients disagree about whether
// a white letter is legible on a mid-green fill. So this reuses CalendarColors.Parse rather than
// growing a second hex reader, the two surfaces draw from one palette on purpose.
//
// A Grid rather than a templated Control, like ColumnSplitter: an Ellipse with one centred child
// needs no control template, and filling the Ellipse with an ImageBrush clips a photo to the circle
// by construction instead of depending on how a Border treats its corner radius.

using System;
using System.Collections.Generic;
using System.IO;
using System.Threading.Tasks;
using Allodia.Mailcal.Calendar;
using Allodia.Mailcal.Services;
using Allodia.Mailcal.ViewModels;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Automation;
using Microsoft.UI.Xaml.Automation.Peers;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Media;
using Microsoft.UI.Xaml.Media.Imaging;
using Microsoft.UI.Xaml.Shapes;

namespace Allodia.Mailcal.Controls;

/// <summary>The avatar circle: a person's photo, else their monogram on their own colour.</summary>
public sealed class AvatarView : Grid
{
    // Segoe Fluent Icons "Contact", the platform's own person glyph, for a row that names nobody.
    private const string PersonGlyph = "";

    // Photos decode once at this height and every size draws from that one bitmap: the largest
    // avatar on screen is 56 effective pixels, which is 112 physical at 200% scale.
    private const int PhotoDecodePixels = 128;

    // A photo's file name is a hash of its own contents, so an entry can never go stale and there
    // is nothing to invalidate. Cleared wholesale at the cap rather than aged out, an LRU would be
    // more machinery than a few hundred small bitmaps deserve. UI-thread only, like every control.
    private const int PhotoCacheCap = 256;

    private static readonly Dictionary<string, BitmapImage> Photos = new();

    // Paths that failed to decode, so a recycled row does not retry the same broken file, and the
    // log records it once rather than once per scroll.
    private static readonly HashSet<string> Unreadable = new();

    // The theme the circle was last drawn in, see the Loaded handler below.
    private bool _drawnDark;

    /// <summary>Identifies the <see cref="Avatar"/> property.</summary>
    public static readonly DependencyProperty AvatarProperty = DependencyProperty.Register(
        nameof(Avatar), typeof(AvatarItem), typeof(AvatarView), new PropertyMetadata(null, OnChanged));

    /// <summary>Identifies the <see cref="Diameter"/> property.</summary>
    public static readonly DependencyProperty DiameterProperty = DependencyProperty.Register(
        nameof(Diameter), typeof(double), typeof(AvatarView), new PropertyMetadata(34.0, OnChanged));

    /// <summary>Creates the circle. It draws nothing until an <see cref="Avatar"/> is set.</summary>
    public AvatarView()
    {
        // Decoration, on every platform. The row already announces the person's name and the
        // monogram restates its first letters, so announcing it would make Narrator read a letter
        // before every sender; the person glyph says nothing at all.
        AutomationProperties.SetAccessibilityView(this, AccessibilityView.Raw);
        // The core resolved both themes up front, so a light/dark switch is a repaint here and
        // never a trip back through the FFI.
        ActualThemeChanged += (_, _) => Apply();
        // A row's template fills its bindings before the container joins the tree, where
        // ActualTheme is still the default rather than the window's. Redrawing on Loaded catches
        // that; the guard keeps it to the rows that were actually drawn in the wrong theme, so it
        // is not a second rebuild per row.
        Loaded += (_, _) =>
        {
            if (_drawnDark != (ActualTheme == ElementTheme.Dark))
            {
                Apply();
            }
        };
    }

    /// <summary>The person this circle is of.</summary>
    public AvatarItem? Avatar
    {
        get => (AvatarItem?)GetValue(AvatarProperty);
        set => SetValue(AvatarProperty, value);
    }

    /// <summary>The circle's diameter in effective pixels.</summary>
    public double Diameter
    {
        get => (double)GetValue(DiameterProperty);
        set => SetValue(DiameterProperty, value);
    }

    private static void OnChanged(DependencyObject element, DependencyPropertyChangedEventArgs args) =>
        ((AvatarView)element).Apply();

    private void Apply()
    {
        Children.Clear();
        Width = Diameter;
        Height = Diameter;
        if (Avatar is not { } avatar)
        {
            return;
        }

        var dark = ActualTheme == ElementTheme.Dark;
        _drawnDark = dark;
        var circle = new Ellipse
        {
            Width = Diameter,
            Height = Diameter,
            Fill = new SolidColorBrush(CalendarColors.Parse(avatar.Background(dark))),
        };
        Add(circle);

        var ink = new SolidColorBrush(CalendarColors.Parse(avatar.TextColor(dark)));
        // A photo is drawn OVER the monogram rather than instead of it: decoding is asynchronous,
        // and the contract says an avatar is never blank, so the letters hold the circle until the
        // face arrives, and stay if it never does.
        var monogram = avatar.Content == AvatarContent.PersonGlyph
            ? Glyph(ink)
            : Letters(avatar.Initials, ink);
        Add(monogram);

        if (avatar.Content == AvatarContent.Photo)
        {
            _ = ShowPhotoAsync(circle, monogram, avatar.ImagePath!);
        }
    }

    private FrameworkElement Letters(string initials, Brush ink) => new TextBlock
    {
        Text = initials,
        FontSize = Diameter * 0.4,
        FontWeight = Microsoft.UI.Text.FontWeights.Medium,
        Foreground = ink,
        HorizontalAlignment = HorizontalAlignment.Center,
        VerticalAlignment = VerticalAlignment.Center,
    };

    private FrameworkElement Glyph(Brush ink) => new FontIcon
    {
        Glyph = PersonGlyph,
        FontSize = Diameter * 0.5,
        Foreground = ink,
        HorizontalAlignment = HorizontalAlignment.Center,
        VerticalAlignment = VerticalAlignment.Center,
    };

    private void Add(FrameworkElement child)
    {
        // Raw on the child too: the flag does not inherit, and it is the TextBlock, not the panel
        // around it, that a screen reader would otherwise land on and read out.
        AutomationProperties.SetAccessibilityView(child, AccessibilityView.Raw);
        Children.Add(child);
    }

    // Swap the fill for the photo once it has decoded. Nothing waits on a face, so a row that
    // scrolled away before it landed simply drops the result.
    private async Task ShowPhotoAsync(Ellipse circle, FrameworkElement monogram, string path)
    {
        var photo = await PhotoAsync(path);
        if (photo is null)
        {
            return; // The monogram stands. Never a blank circle.
        }
        // The container may have been recycled onto another row while this decoded, which would
        // otherwise put one person's face on someone else's mail.
        if (Avatar?.ImagePath != path)
        {
            return;
        }
        circle.Fill = new ImageBrush { ImageSource = photo, Stretch = Stretch.UniformToFill };
        monogram.Visibility = Visibility.Collapsed;
    }

    private static async Task<BitmapImage?> PhotoAsync(string path)
    {
        if (Photos.TryGetValue(path, out var cached))
        {
            return cached;
        }
        if (Unreadable.Contains(path))
        {
            return null;
        }
        try
        {
            var photo = new BitmapImage { DecodePixelHeight = PhotoDecodePixels };
            using var file = File.OpenRead(path);
            await photo.SetSourceAsync(file.AsRandomAccessStream());
            if (photo.PixelHeight == 0)
            {
                // A decode that fails raises ImageFailed rather than throwing, so the awaited call
                // can return having produced nothing at all.
                throw new InvalidOperationException("the file decoded to no pixels");
            }
            if (Photos.Count >= PhotoCacheCap)
            {
                Photos.Clear();
            }
            Photos[path] = photo;
            return photo;
        }
        catch (Exception error)
        {
            Unreadable.Add(path);
            Log.Warn($"a contact photo could not be drawn: {error.GetType().Name}");
            return null;
        }
    }
}
