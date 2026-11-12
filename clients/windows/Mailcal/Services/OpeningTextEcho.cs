// Telling a text field's own opening value apart from an edit (docs/drafts.md, "When a save
// happens"). WinUI-free so Mailcal.Tests can link it.

namespace Allodia.Mailcal.Services;

/// <summary>
/// Recognises the <c>TextChanged</c> a field raises for the value the composer opened it with.
/// </summary>
/// <remarks>
/// WinUI raises <c>TextChanged</c> for a value assigned in code, and raises it asynchronously, so
/// it reaches the handler after the composer has armed its draft tracking. Counted as an edit, it
/// would mark every reply, forward and resumed draft as written in and save it once the idle
/// interval passed. Only the first change can be that echo, and only when it carries the opened
/// value; a change after it is the user's, even one that puts the opened value back.
/// </remarks>
internal sealed class OpeningTextEcho(string opened)
{
    private bool _pending = true;

    /// <summary>Whether the field's change to <paramref name="now"/> is an edit.</summary>
    internal bool IsEdit(string now)
    {
        if (!_pending)
        {
            return true;
        }
        _pending = false;
        return now != opened;
    }
}
