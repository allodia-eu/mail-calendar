// A recipient field's undo and redo (docs/undo.md rule 9).
//
// The field's own, not the TextBox's. The input holds only the address being typed, and the field
// rewrites it whenever an address is finished, so the input's history would type a finished
// address back in and the field would finish it a second time. A step here is the whole field
// before a change: typing one address, finishing it, accepting a suggestion, removing a pill.
//
// Plain BCL with no WinUI, so Mailcal.Tests drives it directly.

using System.Collections.Generic;
using System.Linq;

namespace Allodia.Mailcal.Services;

/// <summary>The values a field held before each change, and after each undo.</summary>
internal sealed class RecipientHistory
{
    /// <summary>The most steps kept, oldest dropped first.</summary>
    private const int MaxSteps = 100;

    private readonly LinkedList<string> _undone = new();
    private readonly Stack<string> _redone = new();

    /// <summary>Whether the newest step is an address still being typed, which the next keystroke joins.</summary>
    private bool _typing;

    /// <summary>The value the field last held as far as this history knows.</summary>
    internal string Current { get; private set; } = string.Empty;

    /// <summary>Whether there is a change to take back.</summary>
    internal bool CanUndo => _undone.Count > 0;

    /// <summary>Whether there is an undo to put back.</summary>
    internal bool CanRedo => _redone.Count > 0;

    /// <summary>
    /// Files a change to <paramref name="after"/>. <paramref name="typed"/> is a keystroke in the
    /// input, which joins the step before it while the same address is being typed; finishing it is
    /// a step of its own.
    /// </summary>
    internal void Record(string after, bool typed)
    {
        if (after == Current)
        {
            return;
        }
        var sameRecipients = RecipientTokens.Committed(Current).SequenceEqual(RecipientTokens.Committed(after));
        if (!(typed && _typing && sameRecipients))
        {
            _undone.AddLast(Current);
            if (_undone.Count > MaxSteps)
            {
                _undone.RemoveFirst();
            }
        }
        _redone.Clear();
        _typing = typed && sameRecipients;
        Current = after;
    }

    /// <summary>The value to go back to, if there is one.</summary>
    internal string? Undo()
    {
        if (_undone.Last is not { } last)
        {
            return null;
        }
        _undone.RemoveLast();
        _redone.Push(Current);
        return MoveTo(last.Value);
    }

    /// <summary>The value an undo left behind, if nothing has changed since.</summary>
    internal string? Redo()
    {
        if (!_redone.TryPop(out var value))
        {
            return null;
        }
        _undone.AddLast(Current);
        return MoveTo(value);
    }

    /// <summary>Forgets everything: the field was given <paramref name="value"/> from outside it.</summary>
    internal void Restart(string value)
    {
        _undone.Clear();
        _redone.Clear();
        _typing = false;
        Current = value;
    }

    private string MoveTo(string value)
    {
        _typing = false;
        Current = value;
        return value;
    }
}
