// Why the list has no rows, for the panel the view draws over it.
//
// Its own partial rather than a few more lines in MailboxModel.cs, which is close to the 500-line
// limit. The mapping from the core's enum to words is EmptyMailboxLine, pure, and therefore gated
// by Mailcal.Tests; this half is only the property change notification the view binds to.

using uniffi.mailcal_bindings;

namespace Allodia.Mailcal.Services;

public sealed partial class MailboxModel
{
    private EmptyReason? _emptyReason;

    /// <summary>Why the mail list has no rows, or <c>null</c> whenever it has some. A search says
    /// how far it looked through <see cref="SearchHorizon"/> instead, so the two are never both
    /// up.</summary>
    internal EmptyReason? EmptyReason
    {
        get => _emptyReason;
        private set
        {
            if (Set(ref _emptyReason, value))
            {
                Raise(nameof(HasEmptyReason));
                Raise(nameof(EmptyMailboxText));
                Raise(nameof(EmptyMailboxOffersSyncDepth));
            }
        }
    }

    /// <summary>Whether to draw the panel at all, false for every list that has rows.</summary>
    public bool HasEmptyReason => _emptyReason is not null;

    /// <summary>The panel's headline (empty when there is nothing to state).</summary>
    public string EmptyMailboxText => EmptyMailboxLine.For(
        _emptyReason,
        L10n.MailboxEmptyNoMail(),
        L10n.MailboxEmptyWindowed);

    /// <summary>The sentence under the headline; only the depth-bounded case has one.</summary>
    public string EmptyMailboxBody =>
        EmptyMailboxOffersSyncDepth ? L10n.MailboxEmptyWindowedBody() : string.Empty;

    /// <summary>Whether the panel offers the depth setting: only where widening would find
    /// something.</summary>
    public bool EmptyMailboxOffersSyncDepth => EmptyMailboxLine.OffersSyncDepth(_emptyReason);
}
