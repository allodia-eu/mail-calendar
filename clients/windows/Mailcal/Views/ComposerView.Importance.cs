// The composer's importance picker (docs/importance.md), in the action row beside Signature.
//
// The importance is the core's, held with the composition: the menu reads it there, a pick writes it
// there, and every save and the send take it from there, so the composer sends nothing of its own. A
// composer popped out into a window of its own reads the same composition and so opens with it.

using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Automation;
using Microsoft.UI.Xaml.Controls;
using uniffi.mailcal_bindings;

namespace Allodia.Mailcal.Views;

public sealed partial class ComposerView
{
    /// <summary>The picker as the core last answered it, and the importance's half of whether
    /// anything was written (<see cref="ComposerImportance.Changed"/>).</summary>
    private ComposerImportance? _importance;

    /// <summary>Draws the picker the core answered with. A call that failed answers <c>null</c>
    /// and leaves what is drawn; with no core at all the button stays hidden.</summary>
    private void ShowImportance(ComposerImportance? importance)
    {
        if (importance is null)
        {
            return;
        }
        _importance = importance;
        ImportanceButton.Visibility = Visibility.Visible;
        // The glyph a list row draws for what is in force, and none for normal, which a row
        // marks with nothing either.
        ImportanceIcon.Visibility = importance.Selected == MessageImportance.Normal
            ? Visibility.Collapsed
            : Visibility.Visible;
        ImportanceIcon.Glyph = importance.Selected == MessageImportance.Low ? "\uE74B" : "\uE8C9";
        AutomationProperties.SetItemStatus(ImportanceButton, ImportanceLabel(importance.Selected));
        ImportanceMenu.Items.Clear();
        foreach (var option in importance.Options)
        {
            var item = new RadioMenuFlyoutItem
            {
                Text = ImportanceLabel(option),
                GroupName = "composer-importance",
                IsChecked = option == importance.Selected,
            };
            item.Click += (_, _) => PickImportance(option);
            ImportanceMenu.Items.Add(item);
        }
    }

    private void PickImportance(MessageImportance importance)
    {
        if (_model is null)
        {
            return;
        }
        ShowImportance(_model.SetComposerImportance(_composition, importance));
        // A pick is a change to the draft, so the autosave interval starts again.
        NoteDraftChange();
    }

    private static string ImportanceLabel(MessageImportance importance) => importance switch
    {
        MessageImportance.High => L10n.ComposeImportanceHigh(),
        MessageImportance.Low => L10n.ComposeImportanceLow(),
        _ => L10n.ComposeImportanceNormal(),
    };
}
