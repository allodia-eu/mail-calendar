// Which key chords undo and redo while writing (docs/undo.md rule 2). Plain BCL, so Mailcal.Tests
// holds it; Views/HistoryKeys.cs reads the keyboard into it.

namespace Allodia.Mailcal.Services;

/// <summary>What an undo chord, or a menu's Undo or Redo, asks for.</summary>
internal enum HistoryStep
{
    /// <summary>Take back the newest change.</summary>
    Undo,

    /// <summary>Put back what the newest undo took back.</summary>
    Redo,
}

/// <summary>The undo and redo chords, by virtual key.</summary>
internal static class HistoryChord
{
    private const int KeyY = 0x59;
    private const int KeyZ = 0x5A;

    /// <summary>Ctrl+Z is undo, Ctrl+Shift+Z and Ctrl+Y redo.</summary>
    /// <remarks>
    /// Read by virtual key, as a TextBox reads its own. A layout with no Latin letter on the key (a
    /// Cyrillic one types <c>я</c> where Z is) still reports the key as Z, so it still undoes.
    /// </remarks>
    internal static HistoryStep? Read(int virtualKey, bool ctrl, bool shift, bool alt)
    {
        if (!ctrl || alt)
        {
            return null;
        }
        return virtualKey switch
        {
            KeyZ => shift ? HistoryStep.Redo : HistoryStep.Undo,
            KeyY when !shift => HistoryStep.Redo,
            _ => null,
        };
    }
}
