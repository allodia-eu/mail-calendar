// Settings → Reading → Swipe actions: the two per-direction pickers, as on macOS and Android
// Split into its own partial to keep SettingsDialog.cs under the 500-line limit.
//
// The two directions are set independently and both default to Move to Trash; the values are
// persisted in Rust (`swipe_settings` / `set_swipe_action`, over the FFI since #67), so there is no
// client-side state to keep in step here, the picker reads the core and writes back to it.

using System.Linq;
using Allodia.Mailcal.Views;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using uniffi.mailcal_bindings;

namespace Allodia.Mailcal.Dialogs;

public sealed partial class SettingsDialog
{
    // The actions a direction can be bound to, in the order the picker lists them. The generated
    // enum carries no ordering of its own, so it lives here (as it does on Apple).
    private static readonly SwipeActionKind[] SwipeActionKinds =
    [
        SwipeActionKind.Delete,
        SwipeActionKind.Archive,
        SwipeActionKind.Star,
    ];

    private UIElement SwipeActionControls()
    {
        var settings = _model.SwipeActions;
        var panel = new StackPanel { Spacing = 12 };
        panel.Children.Add(SwipePicker(L10n.SettingsSwipeLeft(), SwipeDirection.Left, settings.Left));
        panel.Children.Add(SwipePicker(L10n.SettingsSwipeRight(), SwipeDirection.Right, settings.Right));
        return panel;
    }

    // One direction's picker. The icon beside each entry is the same symbol the swipe button shows,
    // so the setting and the gesture read as the one feature.
    private UIElement SwipePicker(string header, SwipeDirection direction, SwipeActionKind selected)
    {
        var box = new ComboBox { MinWidth = 260, Header = header };
        foreach (var action in SwipeActionKinds)
        {
            var content = new StackPanel { Orientation = Orientation.Horizontal, Spacing = 8 };
            content.Children.Add(new SymbolIcon { Symbol = MailListView.SwipeActionSymbol(action) });
            content.Children.Add(new TextBlock
            {
                Text = MailListView.SwipeActionLabel(action),
                VerticalAlignment = VerticalAlignment.Center,
            });
            box.Items.Add(new ComboBoxItem { Content = content, Tag = action });
        }
        box.SelectedIndex = Array.IndexOf(SwipeActionKinds, selected);
        box.SelectionChanged += (_, _) =>
        {
            // The dialog rebuilds its detail panel on a category change, which re-selects each
            // picker programmatically; _rebuilding suppresses that so it isn't written back as a
            // user edit.
            if (_rebuilding || (box.SelectedItem as ComboBoxItem)?.Tag is not SwipeActionKind action)
            {
                return;
            }
            _model.SetSwipeAction(direction, action);
        };
        return box;
    }
}
