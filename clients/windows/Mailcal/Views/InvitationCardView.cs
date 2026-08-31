// The meeting-invitation card, drawn above the message body, the Windows twin of
// InvitationCardView.swift and InvitationCard.kt.
//
// Everything on it was decided by the core (docs/invitations.md): whether there is a card at all, the
// organiser line, the attendee tally, the conflict count, and the meeting-day preview's geometry.
// This view localises and arranges; it computes no counts of its own, so this client and the next
// cannot disagree about whether a meeting clashes.
//
// SECURITY (Gate 8, docs/rendering-security.md), the summary, location, description and organizer
// name are attacker-controlled sender content, and they reach the screen without passing the HTML
// sanitiser, the CSP or a WebView2. Every one of them is assigned to `TextBlock.Text`, which WinUI
// renders as text and nothing else: markup on this platform needs either a `RichTextBlock` with
// authored `Inline`s or an explicit `XamlReader.Load`, neither of which a plain string can become by
// accident. So there is no markup path to fall into here, unlike GTK, where a libadwaita row parses
// its title as Pango markup by *default* (AGENTS.md), or SwiftUI, where a string literal selects the
// markdown-parsing overload. Nothing on this card reaches XamlReader, a WebView2, or the composer
// bridge.
//
// The conflict count is stated in WORDS beside the preview grid, always, docs/calendar.md §4: a
// picture the user has to read carefully is not a disclosure.
//
// The respond half is InvitationCardView.Respond.cs: it is the only part of the card that *writes*,
// and everything in it is conditional on what the account can actually do.
using System.Globalization;
using Allodia.Mailcal.Calendar;
using Microsoft.UI;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Automation;
using Microsoft.UI.Xaml.Automation.Peers;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Media;
using Windows.UI;
using uniffi.mailcal_bindings;

namespace Allodia.Mailcal.Views;

/// <summary>The invitation card for the open message: who is asking, when, and what it clashes with.</summary>
internal sealed partial class InvitationCardView : UserControl
{
    private const double CardCorner = 8;

    /// <summary>The label column of the organiser / when / where rows.</summary>
    private const double LabelWidth = 76;

    private readonly Border _frame = new()
    {
        CornerRadius = new CornerRadius(CardCorner),
        BorderThickness = new Thickness(1),
        Padding = new Thickness(10),
    };

    private readonly StackPanel _stack = new() { Spacing = 4 };

    private CultureInfo _culture = CultureInfo.CurrentCulture;

    /// <summary>
    /// Answer the invitation: the response, a note for the organiser (<c>null</c> where the transport
    /// carries none), and whether to tell them.
    /// </summary>
    /// <remarks>
    /// The host names the <b>message</b> when it dispatches this, never the event, the answer goes
    /// out as the address the invitation matched, and only the core knows the address set
    /// (docs/invitations.md §4).
    /// </remarks>
    internal Action<InvitationResponse, string?, bool, string>? Respond { get; set; }

    internal InvitationCardView()
    {
        _frame.Child = _stack;
        Content = _frame;
    }

    /// <summary>Draws <paramref name="card"/>, the whole card, rebuilt.</summary>
    /// <param name="card">The core's decided card for the open message.</param>
    /// <param name="zone">The display zone: the card's instants are UTC and the host localises them.</param>
    /// <param name="use24Hour">The app's clock setting, so mail and calendar cannot disagree.</param>
    /// <param name="culture">The formatting culture the app's language choice pinned.</param>
    /// <param name="status">The calendar write currently settling, if any.</param>
    internal void Apply(
        InvitationCard card,
        string zone,
        bool use24Hour,
        CultureInfo culture,
        CalendarWriteStatus status)
    {
        _culture = culture;
        _stack.Children.Clear();
        ResetRespondRow();
        var tint = TintOf(card.Kind);
        _frame.Background = new SolidColorBrush(tint) { Opacity = 0.08 };
        _frame.BorderBrush = new SolidColorBrush(tint) { Opacity = 0.3 };

        _stack.Children.Add(Header(card.Kind, tint));
        if (InvitationText.Notice(card.Kind) is { } notice)
        {
            _stack.Children.Add(Caption(notice));
        }
        _stack.Children.Add(new TextBlock
        {
            Text = string.IsNullOrEmpty(card.Summary) ? L10n.InvitationNoTitle() : card.Summary,
            Style = Res("BodyStrongTextBlockStyle"),
            MaxLines = 2,
            TextWrapping = TextWrapping.Wrap,
            TextTrimming = TextTrimming.CharacterEllipsis,
        });
        _stack.Children.Add(Detail(L10n.InvitationOrganizer(), card.Organizer));
        _stack.Children.Add(Detail(
            L10n.InvitationWhen(),
            InvitationFormat.When(card.StartsAt, card.EndsAt, card.AllDay, zone, use24Hour, culture)));
        if (!string.IsNullOrEmpty(card.Location))
        {
            _stack.Children.Add(Detail(L10n.InvitationWhere(), card.Location));
        }
        if (card.Recurring)
        {
            _stack.Children.Add(Caption(L10n.InvitationRepeats()));
        }
        AddDescription(card);
        AddAnswer(card, status);
        AddConflicts(card, zone, use24Hour);
    }

    // A cancellation is the one kind that has to be unmissable, a stale hold otherwise sits in the
    // calendar looking like a commitment.
    private static Color TintOf(InvitationKind kind) => kind switch
    {
        InvitationKind.Cancelled => BrushColor("SystemFillColorCriticalBrush"),
        InvitationKind.Informational => BrushColor("TextFillColorSecondaryBrush"),
        // Caution, not critical: nothing was lost, there is simply a newer copy to open.
        InvitationKind.Superseded => BrushColor("SystemFillColorCautionBrush"),
        _ => BrushColor("AccentTextFillColorPrimaryBrush"),
    };

    private static UIElement Header(InvitationKind kind, Color tint)
    {
        var row = new StackPanel { Orientation = Orientation.Horizontal, Spacing = 6 };
        row.Children.Add(new FontIcon
        {
            // The Segoe Fluent glyphs the rest of the app already uses: Cancel, Info, Warning,
            // Calendar.
            Glyph = kind switch
            {
                InvitationKind.Cancelled => "\uE711",
                InvitationKind.Informational => "\uE946",
                InvitationKind.Superseded => "\uE7BA",
                _ => "\uE787",
            },
            FontSize = 14,
            Foreground = new SolidColorBrush(tint),
            VerticalAlignment = VerticalAlignment.Center,
        });
        var title = new TextBlock
        {
            Text = InvitationText.Title(kind),
            Style = Res("CaptionTextBlockStyle"),
            Foreground = new SolidColorBrush(tint),
            VerticalAlignment = VerticalAlignment.Center,
        };
        // A heading, so a screen reader's heading navigation lands on the card in one hop. The card
        // itself carries NO container label: naming the container is exactly what collapsed the whole
        // card into a single node on iOS and put the three buttons out of reach of VoiceOver
        // (InvitationCardView.swift). Every line here is reachable on its own instead.
        AutomationProperties.SetHeadingLevel(title, AutomationHeadingLevel.Level3);
        row.Children.Add(title);
        return row;
    }

    private static UIElement Detail(string label, string value)
    {
        var row = new Grid { ColumnSpacing = 6 };
        row.ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(LabelWidth) });
        row.ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(1, GridUnitType.Star) });
        var name = Caption(label);
        Grid.SetColumn(name, 0);
        row.Children.Add(name);
        // Selectable: an organiser's address and a meeting room are things people copy out.
        var text = new TextBlock
        {
            Text = value,
            Style = Res("CaptionTextBlockStyle"),
            TextWrapping = TextWrapping.Wrap,
            MaxLines = 3,
            TextTrimming = TextTrimming.CharacterEllipsis,
            IsTextSelectionEnabled = true,
        };
        Grid.SetColumn(text, 1);
        row.Children.Add(text);
        return row;
    }

    // The organiser's notes. Already truncated by the core (Gmail sends a wall of filler), and the
    // card says so rather than implying the text ends there.
    private void AddDescription(InvitationCard card)
    {
        if (string.IsNullOrEmpty(card.Description))
        {
            return;
        }
        _stack.Children.Add(new TextBlock
        {
            Text = card.Description,
            Style = Res("CaptionTextBlockStyle"),
            Foreground = ThemeBrush("TextFillColorSecondaryBrush"),
            TextWrapping = TextWrapping.Wrap,
            MaxLines = 4,
            TextTrimming = TextTrimming.CharacterEllipsis,
            IsTextSelectionEnabled = true,
        });
        if (card.DescriptionTruncated)
        {
            _stack.Children.Add(Caption(L10n.InvitationDescriptionShortened()));
        }
    }

    /// <summary>
    /// This account's own answer, how everyone else answered, and the buttons to change it.
    /// </summary>
    /// <remarks>
    /// Both lines read the <b>calendar's</b> copy, not the email's, the mail is frozen at the moment
    /// it was sent, so a card built from it would still say "you haven't answered" after you had, and
    /// would go on counting you among the people yet to reply. Only a card carrying an RSVP shows
    /// buttons: a cancellation has nothing to answer.
    /// </remarks>
    private void AddAnswer(InvitationCard card, CalendarWriteStatus status)
    {
        _stack.Children.Add(new Border
        {
            Height = 1,
            Margin = new Thickness(0, 4, 0, 4),
            Background = ThemeBrush("DividerStrokeColorDefaultBrush"),
        });
        _stack.Children.Add(new TextBlock
        {
            Text = InvitationText.Response(card.MyResponse),
            Style = Res("CaptionTextBlockStyle"),
            TextWrapping = TextWrapping.Wrap,
        });
        var attendees = InvitationText.Attendees(card.Attendees);
        if (!string.IsNullOrEmpty(attendees))
        {
            _stack.Children.Add(Caption(attendees));
        }
        if (card.Kind == InvitationKind.Rsvp)
        {
            _stack.Children.Add(RespondRow(card, status));
        }
    }

    /// <summary>
    /// What else is in the calendar then, stated in words, then shown.
    /// </summary>
    /// <remarks>
    /// The preview is offered only when the calendar was actually read. An empty grid drawn over an
    /// unread calendar looks exactly like a free day, which is the whole failure this guards.
    /// </remarks>
    private void AddConflicts(InvitationCard card, string zone, bool use24Hour)
    {
        var clashes = card.ConflictsKnown && card.ConflictCount > 0;
        _stack.Children.Add(new TextBlock
        {
            Text = InvitationText.Conflicts(card.ConflictCount, card.ConflictsKnown),
            Style = Res("CaptionTextBlockStyle"),
            Margin = new Thickness(0, 4, 0, 0),
            TextWrapping = TextWrapping.Wrap,
            Foreground = ThemeBrush(clashes ? "TextFillColorPrimaryBrush" : "TextFillColorSecondaryBrush"),
        });
        if (!card.ConflictsKnown)
        {
            return;
        }
        // Built per card rather than kept as a field: the Expander below is new on every Apply, and a
        // WinUI element may have only one parent, a reused grid would still be owned by the discarded
        // Expander when the new one claimed it.
        var preview = new InvitationPreviewGrid();
        preview.Apply(
            card.Preview,
            InvitationFormat.MeetingMinuteSpan(
                card.StartsAt,
                card.EndsAt,
                // The layout zone the core solved the day in; the display zone only when it did not
                // say. Reading the meeting in a different zone from the blocks beside it would put it
                // in the wrong row of its own preview.
                string.IsNullOrEmpty(card.Preview.Timezone) ? zone : card.Preview.Timezone),
            use24Hour,
            _culture);
        _stack.Children.Add(new Expander
        {
            Header = L10n.InvitationConflictsPreview(),
            Content = preview,
            HorizontalAlignment = HorizontalAlignment.Stretch,
            HorizontalContentAlignment = HorizontalAlignment.Stretch,
            Margin = new Thickness(0, 4, 0, 0),
            // Open whenever the calendar was actually read, which the early return above has
            // already established, so this is unconditional.
            //
            // It used to be `clashes`: open only when the count was non-zero. That is wrong about
            // what the grid is FOR, the question a person answering an invitation is asking is
            // "what does my day look like", and the answer is the picture, not the number. "Nothing
            // else in your calendar then" over a drawn, visibly empty day is a STRONGER answer than
            // the same words over a collapsed row.
            IsExpanded = true,
        });
    }

    private static TextBlock Caption(string text) => new()
    {
        Text = text,
        Style = Res("CaptionTextBlockStyle"),
        Foreground = ThemeBrush("TextFillColorSecondaryBrush"),
        TextWrapping = TextWrapping.Wrap,
    };

    private static Style Res(string key) => (Style)Application.Current.Resources[key];

    private static Brush ThemeBrush(string key) => (Brush)Application.Current.Resources[key];

    // The colour behind a theme brush. Every key used here ships with WinUI, but a missing one must
    // tint the card grey rather than take the reading pane down with it.
    private static Color BrushColor(string key) =>
        Application.Current.Resources.TryGetValue(key, out var value) && value is SolidColorBrush brush
            ? brush.Color
            : Colors.Gray;
}
