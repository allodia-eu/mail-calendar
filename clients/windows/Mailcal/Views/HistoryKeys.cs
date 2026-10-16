// The undo and redo chords read off the keyboard (docs/undo.md rule 2), for the fields around the
// composer's body. The body answers them in the shared bundle and no host binds them there.

using Allodia.Mailcal.Services;
using Microsoft.UI.Input;
using Microsoft.UI.Xaml.Controls;
using Windows.System;
using Windows.UI.Core;

namespace Allodia.Mailcal.Views;

/// <summary>The undo and redo chords, as the key being pressed and the modifiers held with it.</summary>
internal static class HistoryKeys
{
    /// <summary>What <paramref name="key"/>, pressed now, asks of a history, if anything.</summary>
    internal static HistoryStep? Of(VirtualKey key) =>
        HistoryChord.Read((int)key, IsDown(VirtualKey.Control), IsDown(VirtualKey.Shift), IsDown(VirtualKey.Menu));

    /// <summary>
    /// Has <paramref name="box"/> redo on Shift+Ctrl+Z, over its own history.
    /// </summary>
    /// <remarks>
    /// A plain field keeps the TextBox's own history (docs/undo.md rule 10), which answers Ctrl+Z
    /// and Ctrl+Y and nothing on Shift+Ctrl+Z, the chord the body and the recipient fields beside
    /// it redo with as well.
    /// </remarks>
    internal static void RedoOnShiftChord(TextBox box) =>
        box.PreviewKeyDown += (_, e) =>
        {
            if (e.Key == VirtualKey.Z && Of(e.Key) == HistoryStep.Redo)
            {
                e.Handled = true;
                if (box.CanRedo)
                {
                    box.Redo();
                }
            }
        };

    private static bool IsDown(VirtualKey key) =>
        InputKeyboardSource.GetKeyStateForCurrentThread(key).HasFlag(CoreVirtualKeyStates.Down);
}
