// The two ways out of a composer (docs/drafts.md, "Leaving a composer"), as plain rules the WinUI
// half applies. WinUI-free so Mailcal.Tests can link it: the composer view itself cannot be built
// there.

namespace Allodia.Mailcal.Services;

/// <summary>What leaving a composer does with its composition.</summary>
internal enum ComposerLeave
{
    /// <summary>Forget the composition and leave Drafts as it is.</summary>
    Close,

    /// <summary>Store what the composer holds over its own copy, then forget the composition, in
    /// one call to the core.</summary>
    SaveAndClose,
}

/// <summary>The decisions behind leaving a composer and discarding one.</summary>
internal static class ComposerExit
{
    /// <summary>
    /// What leaving does: a click on another message, a different compose, a link, a share, an
    /// assistant's draft or the composer window's own close.
    /// </summary>
    /// <remarks>
    /// Never a question. A composer something was <paramref name="written"/> in is saved; one
    /// nobody touched, a reply holding only its quote and a resumed draft only looked at are
    /// closed, because saving them would put nothing new in Drafts.
    /// </remarks>
    internal static ComposerLeave OnLeave(bool keepsDraft, bool written) =>
        keepsDraft && written ? ComposerLeave.SaveAndClose : ComposerLeave.Close;

    /// <summary>
    /// Whether Discard asks "Discard draft?" before it removes anything: only when something would
    /// be lost, words <paramref name="written"/> in this composer or a copy
    /// <paramref name="stored"/> in Drafts.
    /// </summary>
    internal static bool AsksBeforeDiscard(bool written, bool stored) => written || stored;
}
