// The reveal's fourth and fifth pages: tone and approach as three cards, and the person's phrases
// as chips, with what they avoid struck through (docs/ai.md, "Learning" step 5). Also RevealInk,
// the colours every page draws with.

using Allodia.Mailcal.Controls;
using Allodia.Mailcal.Services;
using Microsoft.UI.Text;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Automation;
using Microsoft.UI.Xaml.Automation.Peers;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Media;
using uniffi.mailcal_bindings;
using Windows.UI;

namespace Allodia.Mailcal.Dialogs;

public sealed partial class SettingsDialog
{
    // Step 4: register, structure, and how the person declines and chases, one card each. A card
    // with nothing to say is left out.
    private static UIElement VoicePage(LanguageStyleRow style, RevealInk ink)
    {
        var page = WizardFrame.Page(L10n.RevealStepVoice());
        foreach (var (glyph, label, headline, description) in new[]
                 {
                     ("\uE9E9", L10n.RevealRegister(), style.RegisterHeadline, style.Register),
                     ("\uE8CA", L10n.RevealStructure(), style.StructureHeadline, style.Structure),
                     ("\uE8F2", L10n.RevealMoves(), style.MovesHeadline, style.Moves),
                 })
        {
            var (heading, body) = StyleReveal.CardText(headline, description);
            if (heading.Length > 0 || body.Length > 0)
            {
                page.Children.Add(VoiceCard(glyph, label, heading, body, ink));
            }
        }
        return page;
    }

    // One card: a glyph, what it is about, a heading, and two lines of the description that open
    // to the rest. More is offered only when the two lines cut something off.
    private static Border VoiceCard(string glyph, string label, string heading, string body, RevealInk ink)
    {
        var card = new StackPanel { Spacing = 4 };
        var top = new Grid { ColumnSpacing = 10 };
        top.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });
        top.ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(1, GridUnitType.Star) });
        top.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });
        var symbol = new FontIcon { Glyph = glyph, FontSize = 14, Foreground = ink.Accent };
        AutomationProperties.SetAccessibilityView(symbol, AccessibilityView.Raw);
        var icon = new Border
        {
            Width = 28,
            Height = 28,
            CornerRadius = new CornerRadius(6),
            Background = ink.AccentFaint,
            Child = symbol,
        };
        var name = Label(label, ink);
        name.VerticalAlignment = VerticalAlignment.Center;
        Grid.SetColumn(name, 1);
        top.Children.Add(icon);
        top.Children.Add(name);
        card.Children.Add(top);
        if (heading.Length > 0)
        {
            card.Children.Add(new TextBlock
            {
                Text = heading,
                FontSize = 16,
                FontWeight = FontWeights.SemiBold,
                TextWrapping = TextWrapping.Wrap,
                Margin = new Thickness(0, 6, 0, 0),
            });
        }
        if (body.Length > 0)
        {
            var description = new TextBlock
            {
                Text = body,
                TextWrapping = TextWrapping.Wrap,
                TextTrimming = TextTrimming.CharacterEllipsis,
                MaxLines = 2,
            };
            var more = new HyperlinkButton
            {
                Content = L10n.RevealCardMore(),
                Padding = new Thickness(6, 2, 6, 2),
                VerticalAlignment = VerticalAlignment.Center,
                Visibility = Visibility.Collapsed,
            };
            var expanded = false;
            description.IsTextTrimmedChanged += (_, _) =>
                more.Visibility = expanded || description.IsTextTrimmed ? Visibility.Visible : Visibility.Collapsed;
            more.Click += (_, _) =>
            {
                expanded = !expanded;
                description.MaxLines = expanded ? 0 : 2;
                more.Content = expanded ? L10n.RevealCardLess() : L10n.RevealCardMore();
            };
            Grid.SetColumn(more, 2);
            top.Children.Add(more);
            card.Children.Add(description);
        }
        return Card(card, ink);
    }

    // Step 5: the person's phrases, and what they avoid, struck through.
    private static UIElement PhrasesPage(LanguageStyleRow style, RevealInk ink)
    {
        var page = WizardFrame.Page(L10n.RevealPhrases());
        var phrases = style.Phrases.Where(phrase => !string.IsNullOrWhiteSpace(phrase)).ToArray();
        if (phrases.Length > 0)
        {
            var cloud = new FlowPanel { Spacing = 8 };
            foreach (var phrase in phrases)
            {
                cloud.Children.Add(Chip(phrase, avoided: false, ink));
            }
            page.Children.Add(cloud);
        }
        var avoid = style.Avoid.Where(phrase => !string.IsNullOrWhiteSpace(phrase)).ToArray();
        if (avoid.Length > 0)
        {
            var heading = Label(L10n.RevealAvoid(), ink);
            heading.Margin = new Thickness(0, 10, 0, 0);
            AutomationProperties.SetHeadingLevel(heading, AutomationHeadingLevel.Level3);
            page.Children.Add(heading);
            var cloud = new FlowPanel { Spacing = 8 };
            foreach (var phrase in avoid)
            {
                cloud.Children.Add(Chip(phrase, avoided: true, ink));
            }
            page.Children.Add(cloud);
        }
        return page;
    }

    private static Border Chip(string phrase, bool avoided, RevealInk ink)
    {
        var chip = new Border
        {
            BorderBrush = avoided ? ink.Separator : ink.Stroke,
            BorderThickness = new Thickness(1),
            CornerRadius = new CornerRadius(16),
            Padding = new Thickness(14, 5, 14, 6),
            Child = new TextBlock
            {
                Text = phrase,
                TextWrapping = TextWrapping.Wrap,
                TextDecorations = avoided
                    ? Windows.UI.Text.TextDecorations.Strikethrough
                    : Windows.UI.Text.TextDecorations.None,
                Foreground = avoided ? ink.Secondary : ink.Primary,
            },
        };
        if (!avoided)
        {
            chip.Background = ink.Card;
        }
        return chip;
    }

    // The colours the reveal draws with, handed out once per reveal as brushes that follow the
    // dialog's theme. The neutral fills are Fluent's subtle and strong fills; the accent ones are
    // the desktop's accent, lightened as a fill.
    private sealed class RevealInk
    {
        internal RevealInk(ThemeBrushes brushes)
        {
            Primary = brushes.Of(ThemePalette.PrimaryText);
            Secondary = brushes.Of(ThemePalette.SecondaryText);
            Separator = brushes.Of(ThemePalette.Divider);
            Stroke = brushes.Of(ThemePalette.ControlStroke);
            Card = brushes.Of(ThemePalette.CardBackground);
            Track = brushes.Of(dark => Neutral(dark, light: 0x08, night: 0x0F));
            Fill = brushes.Of(dark => Neutral(dark, light: 0x0F, night: 0x15));
            FillStrong = brushes.Of(dark => Neutral(dark, light: 0x1A, night: 0x28));
            Ink = brushes.Of(dark => Neutral(dark, light: 0x24, night: 0x29));
            Accent = brushes.Of(ThemePalette.AccentText);
            AccentSoft = brushes.Of(dark => Accented(dark, light: 0x2E, night: 0x52));
            AccentFaint = brushes.Of(dark => Accented(dark, light: 0x17, night: 0x29));
            AccentLine = brushes.Of(dark => Accented(dark, light: 0x66, night: 0x66));
        }

        internal Brush Primary { get; }

        internal Brush Secondary { get; }

        internal Brush Separator { get; }

        internal Brush Stroke { get; }

        internal Brush Card { get; }

        /// <summary>The ground a greeting's bar grows over.</summary>
        internal Brush Track { get; }

        /// <summary>A language chip.</summary>
        internal Brush Fill { get; }

        /// <summary>Every greeting's bar but the most used.</summary>
        internal Brush FillStrong { get; }

        /// <summary>The miniature letter's grey lines.</summary>
        internal Brush Ink { get; }

        internal Brush Accent { get; }

        /// <summary>The most used greeting's bar.</summary>
        internal Brush AccentSoft { get; }

        /// <summary>A placeholder pill, a card's glyph.</summary>
        internal Brush AccentFaint { get; }

        internal Brush AccentLine { get; }

        private static Color Neutral(bool dark, byte light, byte night) =>
            dark ? Color.FromArgb(night, 0xFF, 0xFF, 0xFF) : Color.FromArgb(light, 0x00, 0x00, 0x00);

        private static Color Accented(bool dark, byte light, byte night)
        {
            var accent = ThemePalette.AccentText(dark);
            return Color.FromArgb(dark ? night : light, accent.R, accent.G, accent.B);
        }
    }
}
