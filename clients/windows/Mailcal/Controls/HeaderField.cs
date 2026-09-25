// A composer header field's input (To, Cc, Bcc, Subject): a TextBox that draws no box of its own,
// under an underline that turns to the accent while it has focus, so a row reads as one line of the
// header the way a mail client draws it, and the recipient pills and the text typed after them read
// as one field.
//
// The TextBox template paints its background and border from theme keys in its pointer-over and
// focused states, whatever the control's own Background and BorderThickness say, so those keys are
// overridden in the box's own resources, where the template looks first. They are transparent in
// every theme, which is what lets them be set here rather than per theme.

using Microsoft.UI;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Media;

namespace Allodia.Mailcal.Controls;

/// <summary>Turns a TextBox into a composer header field's input.</summary>
internal static class HeaderField
{
    private static readonly string[] ClearedBrushes =
    [
        "TextControlBackground",
        "TextControlBackgroundPointerOver",
        "TextControlBackgroundFocused",
        "TextControlBorderBrush",
        "TextControlBorderBrushPointerOver",
        "TextControlBorderBrushFocused",
    ];

    /// <summary>
    /// Strips <paramref name="box"/>'s own chrome and shows <paramref name="focusLine"/>, the
    /// accent underline drawn by the row, while the box has focus. Call before the box loads.
    /// </summary>
    internal static void Attach(TextBox box, UIElement focusLine)
    {
        foreach (var key in ClearedBrushes)
        {
            box.Resources[key] = new SolidColorBrush(Colors.Transparent);
        }
        box.Resources["TextControlBorderThemeThicknessFocused"] = new Thickness(0);
        box.BorderThickness = new Thickness(0);
        box.Background = new SolidColorBrush(Colors.Transparent);
        // No leading padding, so the text starts where a pill would.
        box.Padding = new Thickness(0, 5, 6, 6);
        box.GotFocus += (_, _) => focusLine.Visibility = Visibility.Visible;
        box.LostFocus += (_, _) => focusLine.Visibility = Visibility.Collapsed;
    }
}
