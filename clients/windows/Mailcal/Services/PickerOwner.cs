// Which window a file picker belongs to. The Windows App SDK pickers take the owner's window id, and
// a picker opened from a message in a window of its own belongs to that window: owned by the
// mailbox, it opens over the mailbox, behind the window the reader pressed Save in.

using Microsoft.UI;
using Microsoft.UI.Xaml;

namespace Allodia.Mailcal.Services;

/// <summary>The window a picker opened from a given element belongs to.</summary>
internal static class PickerOwner
{
    /// <summary>
    /// The window <paramref name="origin"/> is drawn in, or the mailbox when there is no element
    /// (a dialog's own handler) or it is not in a window yet.
    /// </summary>
    public static WindowId For(UIElement? origin)
    {
        if (origin?.XamlRoot?.ContentIslandEnvironment is { } island)
        {
            return island.AppWindowId;
        }
        return App.MainWindow?.AppWindow.Id ?? default;
    }
}
