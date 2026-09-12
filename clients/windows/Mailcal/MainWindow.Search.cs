// The search field in the window's top row: how wide it is, what typing in it does, and when the
// core hears about it. Split out of MainWindow.xaml.cs so the shell file stays about the shell.
//
// The field is the WINDOW's, not the message list's. Its default scope is every account and every
// folder (docs/search.md, rule 2), so a control drawn over one column was acting on all of them;
// the caption is the only row here whose centre is not a pane's.

using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;

namespace Allodia.Mailcal;

public sealed partial class MainWindow
{
    // How long the field stays quiet before the core is asked, matching macOS, Android and Linux
    // so a search costs the same on every platform.
    private static readonly TimeSpan SearchDebounce = TimeSpan.FromMilliseconds(250);

    private DispatcherTimer? _searchDebounce;
    private string _pendingSearch = string.Empty;

    // What the caption's leading cluster takes: the pane toggle, the app icon and the app's name.
    // The name is localised and injected (docs/branding.md), so this is a floor rather than a
    // measurement, and it is the field that yields when the two would meet.
    private const double CaptionLeadingClusterDip = 240;

    // The field never shrinks past the point where a subject line is unreadable in it, and never
    // grows into the full-width bar that reads as a browser's address bar rather than a search.
    private const double SearchFieldMinDip = 280;
    private const double SearchFieldMaxDip = 560;

    /// <summary>
    /// Sizes the caption's search field, from the window it is in.
    /// </summary>
    /// <remarks>
    /// Set rather than bound, because there is nothing here to bind against: the field is centred
    /// over the whole caption, so what bounds it is what sits beside it, and the budget is what is
    /// left once the leading cluster is cleared on BOTH sides. Twice the nearer neighbour, not the
    /// sum of the two, because a centred control cannot take more room on one side than the other.
    /// </remarks>
    private void OnTitleBarSizeChanged(object sender, SizeChangedEventArgs e) =>
        SearchBox.Width = Math.Clamp(
            e.NewSize.Width - (2 * CaptionLeadingClusterDip),
            SearchFieldMinDip,
            SearchFieldMaxDip);

    private void OnSearchChanged(AutoSuggestBox sender, AutoSuggestBoxTextChangedEventArgs args)
    {
        // Act on typed input AND the built-in clear (X) button, which raises a ProgrammaticChange,
        // otherwise clearing the box leaves search mode stuck on. We never set the text in code
        // apart from that, and show no suggestions.
        if (args.Reason == AutoSuggestionBoxTextChangeReason.SuggestionChosen)
        {
            return;
        }
        var query = sender.Text;
        // The list's header re-labels itself for the search context immediately, whatever the
        // query costs: the header describes the field, not the results.
        Model.SearchQuery = query;
        if (string.IsNullOrEmpty(query))
        {
            // Leaving search is a navigation, not a query: never made to wait.
            _searchDebounce?.Stop();
            Model.Search(query);
            return;
        }
        ScheduleSearch(query);
    }

    // Restart the debounce; the tick fires on the UI thread, where a dispatched intent belongs.
    // A search is a full-text query per account plus a store read per hit (docs/search.md,
    // "Typing does not mean searching"), so the core is asked once, when the typing stops, rather
    // than once per keystroke.
    private void ScheduleSearch(string query)
    {
        _pendingSearch = query;
        if (_searchDebounce is null)
        {
            _searchDebounce = new DispatcherTimer { Interval = SearchDebounce };
            _searchDebounce.Tick += (_, _) =>
            {
                _searchDebounce!.Stop();
                Model.Search(_pendingSearch);
            };
        }
        _searchDebounce.Stop();
        _searchDebounce.Start();
    }
}
