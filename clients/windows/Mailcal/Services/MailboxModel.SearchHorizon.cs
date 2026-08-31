// The search horizon the list header states: how far back the active search actually looked.
//
// Its own partial rather than a few more lines in MailboxModel.cs, which is close to the 500-line
// limit. The mapping from the core's enum to words is SearchHorizonLine, pure, and therefore
// gated by Mailcal.Tests; this half is only the property change notification the view binds to.

using uniffi.mailcal_bindings;

namespace Allodia.Mailcal.Services;

public sealed partial class MailboxModel
{
    private SearchHorizon? _searchHorizon;

    /// <summary>How far back the active search looked, or <c>null</c> when the list is not a
    /// search, the sync depth of the accounts its scope covered.</summary>
    internal SearchHorizon? SearchHorizon
    {
        get => _searchHorizon;
        private set
        {
            if (Set(ref _searchHorizon, value))
            {
                Raise(nameof(HasSearchHorizon));
                Raise(nameof(SearchHorizonText));
            }
        }
    }

    /// <summary>Whether to show the horizon line at all, false for every list nobody searched.</summary>
    public bool HasSearchHorizon => _searchHorizon is not null;

    /// <summary>The horizon line's text (empty when there is nothing to state).</summary>
    public string SearchHorizonText => SearchHorizonLine.For(
        _searchHorizon,
        L10n.SearchHorizonAll(),
        L10n.SearchHorizonMonths);
}
