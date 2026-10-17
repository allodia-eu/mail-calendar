// One recipient field (To / Cc / Bcc) as pills plus the token being typed, with autosuggest.
//
// The suggestion list is a POPUP, not a sibling in the layout. Inline it took ~160px the moment a
// second character was typed and gave them back on the third, so Cc, Bcc, the editor and the Send
// row jumped down and back while the user was still on the first recipient. A popup takes no layout
// space, so nothing below the field moves at all.
//
// Two things the pills buy over the plain TextBox they replace:
//
//   * **Each address is visibly one thing.** In a bare field, `a@x.com, b@y.com` is a wall of text
//     whose only boundary is a comma the reader has to find; a duplicated or wrong address is easy
//     to miss, and there is nothing to click to remove one.
//   * **The caret ends up where you would put it.** Accepting a suggestion turns that address into a
//     pill and empties the input, so the caret has nowhere to be but the end, the structural fix
//     for "the next keystroke lands inside the address just inserted", rather than a correction
//     applied after the fact.
//
// The value stays ONE comma-separated string (`Text`), which is what the composer's send path
// parses. Everything here is a rendering of it: Services/RecipientTokens.cs owns the split, and is
// tested directly because both of its failure modes are silent (query the whole field and nothing
// matches once a first recipient is entered; replace the whole field on selection and every
// recipient already typed is destroyed).

using System;
using System.Collections.Generic;
using System.Collections.ObjectModel;
using System.Threading.Tasks;
using Allodia.Mailcal.Services;
using Allodia.Mailcal.ViewModels;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Automation;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Input;
using Microsoft.UI.Xaml.Media;
using uniffi.mailcal_bindings;
using Windows.System;

namespace Allodia.Mailcal.Views;

/// <summary>A To/Cc/Bcc field: finished recipients as pills, the one in progress as text.</summary>
public sealed partial class RecipientField : UserControl
{
    /// <summary>
    /// How long the field waits after the last keystroke before asking the core.
    /// </summary>
    /// <remarks>
    /// Long enough that typing a word costs one query rather than one per character, short enough
    /// that the list still arrives while the user is looking at the field.
    /// </remarks>
    private const int SuggestionDebounceMs = 120;

    private readonly ObservableCollection<RecipientPillItem> _pills = new();
    private readonly ObservableCollection<RecipientSuggestionItem> _matches = new();

    /// <summary>The whole field, comma-separated, the composer's source of truth.</summary>
    private string _text = string.Empty;

    /// <summary>What the input holds as far as the field has accounted for it.</summary>
    /// <remarks>
    /// <c>TextChanged</c> is raised asynchronously, after the write has returned, and two writes in
    /// quick succession (a held Ctrl+Z) can both report after the second. So a change is the user's
    /// only when the input holds something other than this. Left unrecognised, the field's own write
    /// reads as a keystroke: the field files it in its history, which clears what there was to redo,
    /// and costs an undo that changes nothing on screen.
    /// </remarks>
    private string _accounted = string.Empty;

    /// <summary>
    /// Bumped on every change of the token, so a debounced query whose token has been superseded
    /// can never land, the counterpart of the Apple field's `.task(id: token)`.
    /// </summary>
    private int _generation;

    /// <summary>Initialises the control.</summary>
    public RecipientField()
    {
        this.InitializeComponent();
        Pills.ItemsSource = _pills;
        SuggestionList.ItemsSource = _matches;
        InstallHistory();
        // A Popup lives in its own layer, rooted at the XamlRoot rather than at this control, so
        // it does NOT go away with the composer that owned it. Close it explicitly, or a draft
        // cancelled with the list open leaves it floating over the reading pane.
        Unloaded += (_, _) => SuggestionPopup.IsOpen = false;
    }

    /// <summary>The field's label ("To", "Cc", "Bcc"), set by the composer from the catalog.</summary>
    /// <remarks>
    /// It also names the input for accessibility. The label is a sibling <c>TextBlock</c>, which is
    /// not a programmatic association, without this the three fields are three unnamed text boxes
    /// and a screen reader cannot tell To from Bcc.
    /// </remarks>
    public string Label
    {
        get => LabelText.Text;
        set
        {
            LabelText.Text = value;
            AutomationProperties.SetName(Input, value);
        }
    }

    /// <summary>
    /// The hint inside an empty input, for a field with no label above it (the event editor's
    /// invitee field). It names the input for accessibility too, as <see cref="Label"/> does.
    /// </summary>
    public string Placeholder
    {
        get => Input.PlaceholderText;
        set
        {
            Input.PlaceholderText = value;
            LabelText.Visibility = Visibility.Collapsed;
            AutomationProperties.SetName(Input, value);
        }
    }

    /// <summary>A control drawn at the trailing edge of the input, the To row's Cc/Bcc chevron.</summary>
    /// <remarks>
    /// A plain CLR property, which XAML property-element syntax sets just as well as a dependency
    /// one; nothing binds to it. It lands in the input's own grid row, so the two stay aligned
    /// however tall the field grows, see the note in the XAML.
    /// </remarks>
    public object? Trailing
    {
        get => TrailingHost.Content;
        set => TrailingHost.Content = value;
    }

    /// <summary>The automation id of the field's input, set by the composer.</summary>
    /// <remarks>
    /// It goes on the <c>TextBox</c> rather than on this control, because a <c>UserControl</c> gets
    /// no automation peer, an id on one is unreachable, and a test waiting for it can only time
    /// out. With it, "Cc is not on screen" is something a UI test can fail on: a collapsed field is
    /// absent from the automation tree, and its three siblings are otherwise indistinguishable.
    /// </remarks>
    public string InputAutomationId
    {
        get => AutomationProperties.GetAutomationId(Input);
        set => AutomationProperties.SetAutomationId(Input, value);
    }

    /// <summary>
    /// The whole field as one comma-separated string, what the send path parses. Setting it
    /// re-renders the pills and the token; reading it gives exactly what is on screen.
    /// </summary>
    /// <remarks>
    /// A value set here comes from outside the field (a reply's recipients), so the field's history
    /// starts again from it: it is where undo stops, not a step.
    /// </remarks>
    public string Text
    {
        get => _text;
        set
        {
            Show(value);
            _history.Restart(_text);
        }
    }

    /// <summary>
    /// The core lookup for a partially-typed recipient; <c>null</c> disables autosuggest.
    /// </summary>
    /// <remarks>
    /// Supplied by the composer as <see cref="MailboxModel.RecipientSuggestionsAsync"/>, which hops
    /// off the UI thread, the call is network-free but blocks on the core's runtime and reaches the
    /// store's connection thread three times.
    /// </remarks>
    internal Func<string, Task<IReadOnlyList<RecipientMatch>>>? SuggestionsFor { get; set; }

    /// <summary>Raised whenever the field's value changes, however it changed.</summary>
    public event EventHandler? RecipientsChanged;

    /// <summary>
    /// Raised on Enter over typed text, for a host that takes what is typed itself (the event
    /// editor's Invite). With no handler, Enter and leaving the input finish the typed address as a
    /// comma would (docs/contacts.md). Either way Enter over typed text goes nowhere else, so a
    /// dialog's default button does not fire under an address being added, and Enter in an empty
    /// input still reaches it.
    /// </summary>
    public event EventHandler? Submitted;

    // The one place the user changes the field. A keystroke, an accepted suggestion and a removed
    // pill all compute the new string with RecipientTokens and come through here, and each is filed
    // in the history as it goes; `typed` is a keystroke in the input.
    private void Change(string value, bool typed)
    {
        if (_text == value)
        {
            return;
        }
        _history.Record(value, typed);
        Show(value);
    }

    // Puts `value` on screen and tells the composer. A history step does not `suggest`: it is not
    // the user typing, and a list opening over Cc for a token they did not just type covers it for
    // nothing.
    private void Show(string value, bool suggest = true)
    {
        if (_text == value)
        {
            return;
        }
        _text = value;
        RebuildPills();

        // Re-seed the input only when the TOKEN actually changed, and compare TRIMMED, that is the
        // whole point of the comparison. `CurrentToken` trims, so the token derived from the field
        // has lost any space the user just typed; compare raw and typing "John " re-seeds the input
        // as "John", eating the space. Every space then goes silently: "John Smith" arrives as
        // "JohnSmith", and a name-based autosuggest query can never match anything.
        var token = RecipientTokens.CurrentToken(_text);
        if (Input.Text.Trim() != token)
        {
            _accounted = token;
            Input.Text = token;
            // The caret goes to the end after any programmatic change (docs/contacts.md §4).
            Input.SelectionStart = Input.Text.Length;
            Input.ClearUndoRedoHistory();
        }

        RecipientsChanged?.Invoke(this, EventArgs.Empty);
        if (suggest)
        {
            _ = QuerySuggestionsAsync(token, ++_generation);
        }
        else
        {
            _generation++;
            SuggestionPopup.IsOpen = false;
        }
    }

    private void OnInputChanged(object sender, TextChangedEventArgs e)
    {
        if (Input.Text == _accounted)
        {
            return;
        }
        _accounted = Input.Text;
        // The field keeps the history, so the input's own is emptied as it fills: what it holds is
        // only the address being typed, and its undo would type a finished one back in.
        Input.ClearUndoRedoHistory();
        // Only the trailing token is the user's to edit; the recipients already finished are carried
        // over verbatim rather than re-parsed out of the box.
        Change(RecipientTokens.FieldText(RecipientTokens.Committed(_text), Input.Text), typed: true);
    }

    // The caret goes back to the input: the button that was pressed is gone with its pill, and the
    // chord that brings the recipient back is heard only there.
    private void OnRemoveRecipient(object sender, RoutedEventArgs e)
    {
        if (sender is FrameworkElement { Tag: int index })
        {
            Change(RecipientTokens.Remove(_text, index), typed: false);
            Input.Focus(FocusState.Programmatic);
        }
    }

    // Escape dismisses the list without touching what has been typed, an overlay with no keyboard
    // way out is a trap, and the composer binds Escape to nothing else (it is a pane, not a dialog).
    private void OnInputKeyDown(object sender, KeyRoutedEventArgs e)
    {
        if (e.Key == VirtualKey.Escape && SuggestionPopup.IsOpen)
        {
            SuggestionPopup.IsOpen = false;
            e.Handled = true;
        }
        else if (e.Key == VirtualKey.Enter && Input.Text.Trim().Length > 0)
        {
            SuggestionPopup.IsOpen = false;
            e.Handled = true;
            if (Submitted is { } submitted)
            {
                submitted(this, EventArgs.Empty);
            }
            else
            {
                FinishTyped();
            }
        }
    }

    // Focus is leaving the input: close the list unless it is going INTO the list. Without the
    // exception, clicking a suggestion would dismiss the popup before the click landed on it; and
    // without the rule, tabbing from To to Cc would leave To's suggestions floating over Cc.
    //
    // Leaving the FIELD also finishes the typed address. Neither the list nor a pill's remove
    // button is leaving it: finished on the way to a suggestion, the half-typed token becomes a pill
    // beside the address chosen, and the pills rebuilt under a press swallow its click.
    private void OnInputLosingFocus(object sender, LosingFocusEventArgs e)
    {
        var next = e.NewFocusedElement as DependencyObject;
        if (IsInsideSuggestions(next))
        {
            return;
        }
        SuggestionPopup.IsOpen = false;
        if (Submitted is null && !IsInside(next, this))
        {
            FinishTyped();
        }
    }

    // The address being typed becomes a pill, as a comma would make it.
    private void FinishTyped()
    {
        if (RecipientTokens.CurrentToken(_text).Length > 0)
        {
            Change(RecipientTokens.Seeded(_text), typed: false);
        }
    }

    private bool IsInsideSuggestions(DependencyObject? element) =>
        IsInside(element, SuggestionSurface) || IsInside(element, SuggestionPopup);

    private static bool IsInside(DependencyObject? element, DependencyObject root)
    {
        for (var node = element; node is not null; node = VisualTreeHelper.GetParent(node))
        {
            if (ReferenceEquals(node, root))
            {
                return true;
            }
        }
        return false;
    }

    // The popup is placed against the input but not sized by it, so a list as wide as its longest
    // address would hang off the edge of a narrow composer pane. Match the input's width instead.
    private void OnInputSizeChanged(object sender, SizeChangedEventArgs e) =>
        SuggestionSurface.Width = Input.ActualWidth;

    private void OnSuggestionPicked(object sender, ItemClickEventArgs e)
    {
        if (e.ClickedItem is not RecipientSuggestionItem match)
        {
            return;
        }
        // The address goes in BARE, not as `Name <address>`: the core parses addresses, a display
        // name adds nothing it uses, and a name containing a comma would split into two invalid
        // recipients, which the user would not discover until the send failed.
        Change(RecipientTokens.Accept(_text, match.Email), typed: false);
        Input.Focus(FocusState.Programmatic);
    }

    /// <summary>
    /// Puts the caret in this field's input, the composer opens with it in To on a new message
    /// the caller did not address.
    /// </summary>
    public void FocusInput() => Input.Focus(FocusState.Programmatic);

    private void RebuildPills()
    {
        var committed = RecipientTokens.Committed(_text);
        _pills.Clear();
        for (var index = 0; index < committed.Count; index++)
        {
            _pills.Add(new RecipientPillItem { Text = committed[index], Index = index });
        }
        Pills.Visibility = _pills.Count == 0 ? Visibility.Collapsed : Visibility.Visible;
    }

    // Debounced, off-the-UI-thread lookup. A per-keystroke call would stall the composer whenever a
    // sync held the store's connection; the generation guard means a burst of keystrokes costs one
    // query and a superseded result is dropped rather than overwriting a newer one.
    private async Task QuerySuggestionsAsync(string token, int generation)
    {
        if (SuggestionsFor is null || token.Length == 0)
        {
            ShowSuggestions([]);
            return;
        }
        await Task.Delay(SuggestionDebounceMs);
        if (generation != _generation)
        {
            return;
        }
        var matches = await SuggestionsFor(token);
        if (generation != _generation)
        {
            return;
        }
        ShowSuggestions(matches);
    }

    private void ShowSuggestions(IReadOnlyList<RecipientMatch> matches)
    {
        _matches.Clear();
        var emails = new List<string>(matches.Count);
        foreach (var match in matches)
        {
            _matches.Add(new RecipientSuggestionItem
            {
                Email = match.Email,
                DisplayName = match.DisplayName,
            });
            emails.Add(match.Email);
        }
        // Closed once the current token is exactly one of them: the user has finished that
        // recipient, and a list offering what is already typed covers the next field for nothing.
        var show = RecipientTokens.ShouldShowSuggestions(_text, emails);
        if (show)
        {
            // Width is set here as well as on SizeChanged: the first open usually happens without
            // the input ever having resized, so the handler alone would leave the surface at its
            // content width.
            SuggestionSurface.Width = Input.ActualWidth;
        }
        SuggestionPopup.IsOpen = show;
    }
}
