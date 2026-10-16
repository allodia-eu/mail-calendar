// The field's undo and redo (docs/undo.md rule 9): the chords, and Undo and Redo in the input's
// right-click menu, both over Services/RecipientHistory.cs rather than the TextBox's own history.

using Allodia.Mailcal.Services;
using Microsoft.UI.Xaml.Automation;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Controls.Primitives;
using Microsoft.UI.Xaml.Input;
using Windows.Foundation.Collections;

namespace Allodia.Mailcal.Views;

public sealed partial class RecipientField
{
    private readonly RecipientHistory _history = new();

    private void InstallHistory()
    {
        // Preview, ahead of the TextBox, which answers the same chords from its own history.
        Input.PreviewKeyDown += OnInputPreviewKeyDown;
        // Menus of this input's own, the right-click one and the one touch or a pen opens over a
        // selection: the defaults are flyouts shared by every text box in the app, and this one's
        // Undo and Redo are this field's.
        var menu = new TextCommandBarFlyout { Placement = FlyoutPlacementMode.BottomEdgeAlignedLeft };
        menu.Opening += OnInputMenuOpening;
        Input.ContextFlyout = menu;
        var selection = new TextCommandBarFlyout();
        selection.Opening += OnInputMenuOpening;
        Input.SelectionFlyout = selection;
    }

    private void OnInputPreviewKeyDown(object sender, KeyRoutedEventArgs e)
    {
        if (HistoryKeys.Of(e.Key) is { } step)
        {
            e.Handled = true;
            Step(step);
        }
    }

    // Takes the field back, or forward again, one step.
    private void Step(HistoryStep step)
    {
        if ((step == HistoryStep.Undo ? _history.Undo() : _history.Redo()) is { } value)
        {
            Show(value, suggest: false);
        }
    }

    // The flyout fills itself from the TextBox as it opens. The field's Undo and Redo go where a
    // TextBox puts its own, ahead of Select All, under the platform's own labels, and as a TextBox
    // offers its own: only when there is a step to take. The TextBox's own never appear, because
    // the field empties its history (OnInputChanged), and are taken out here should they ever.
    private void OnInputMenuOpening(object? sender, object e)
    {
        if (sender is not TextCommandBarFlyout menu)
        {
            return;
        }
        RemoveHistoryButtons(menu.PrimaryCommands);
        var commands = menu.SecondaryCommands;
        RemoveHistoryButtons(commands);
        var at = commands.Count;
        for (var index = 0; index < commands.Count; index++)
        {
            if (Kind(commands[index]) == StandardUICommandKind.SelectAll)
            {
                at = index;
                break;
            }
        }
        if (_history.CanRedo)
        {
            commands.Insert(at, HistoryButton(StandardUICommandKind.Redo, HistoryStep.Redo));
        }
        if (_history.CanUndo)
        {
            commands.Insert(at, HistoryButton(StandardUICommandKind.Undo, HistoryStep.Undo));
        }
    }

    private static void RemoveHistoryButtons(IObservableVector<ICommandBarElement> commands)
    {
        for (var index = commands.Count - 1; index >= 0; index--)
        {
            if (Kind(commands[index]) is StandardUICommandKind.Undo or StandardUICommandKind.Redo)
            {
                commands.RemoveAt(index);
            }
        }
    }

    private static StandardUICommandKind? Kind(ICommandBarElement element) =>
        element is AppBarButton { Command: StandardUICommand command } ? command.Kind : null;

    private AppBarButton HistoryButton(StandardUICommandKind kind, HistoryStep step)
    {
        var command = new StandardUICommand(kind);
        command.ExecuteRequested += (_, _) => Step(step);
        var button = new AppBarButton { Command = command };
        // The label is the platform's, in the system's language, so a UI test finds the item by id.
        AutomationProperties.SetAutomationId(button, step == HistoryStep.Undo ? "RecipientUndo" : "RecipientRedo");
        return button;
    }
}
