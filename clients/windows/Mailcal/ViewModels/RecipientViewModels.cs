// The composer recipient field's render-ready types: one finished recipient (a pill) and one ranked
// suggestion. Public POCOs for the same reason as RowViewModels.cs, the generated `RecipientMatch`
// is `internal`, so the FFI type stays confined to the service layer and the templates bind to these.

using Microsoft.UI.Xaml;

namespace Allodia.Mailcal.ViewModels;

/// <summary>One finished recipient, drawn as a pill with its own remove control.</summary>
public sealed class RecipientPillItem
{
    /// <summary>The address (or whatever the user typed) this pill stands for.</summary>
    public required string Text { get; init; }

    /// <summary>
    /// Its position among the finished recipients, what a remove takes out. Rebuilt with the
    /// collection on every change, so it is never stale.
    /// </summary>
    public required int Index { get; init; }

    /// <summary>
    /// The remove button's spoken name. It names the recipient rather than repeating a bare
    /// "Remove", so the control is distinguishable when a screen reader reaches the third otherwise
    /// identical button (docs/contacts.md §4).
    /// </summary>
    public string RemoveLabel => $"{Text}, {L10n.ComposeRemoveRecipient()}";
}

/// <summary>One ranked recipient suggestion under the field.</summary>
public sealed class RecipientSuggestionItem
{
    /// <summary>The address that gets inserted, bare, when this is picked.</summary>
    public required string Email { get; init; }

    /// <summary>The display name; empty when only the address is known.</summary>
    public required string DisplayName { get; init; }

    /// <summary>
    /// Show the name line only when there is one. A suggestion that came only from sent mail carries
    /// no name, it is as valid as one from a saved card, and usually the more useful, so it shows
    /// its address alone rather than being hidden (docs/contacts.md §4).
    /// </summary>
    public Visibility NameVisibility =>
        string.IsNullOrEmpty(DisplayName) ? Visibility.Collapsed : Visibility.Visible;

    /// <summary>
    /// What a screen reader announces for the suggestion. A WinUI <c>ListViewItem</c> with no
    /// accessible name of its own falls back to its data item's <c>ToString()</c>, which is the
    /// type name, so a list of suggestions would otherwise read as several identical class names.
    /// </summary>
    public override string ToString() =>
        string.IsNullOrEmpty(DisplayName) ? Email : $"{DisplayName}, {Email}";
}
