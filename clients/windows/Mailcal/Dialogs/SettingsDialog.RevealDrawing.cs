// How a style is drawn, on the reveal's pages and on the detail (docs/ai.md rules 15 and 16): the
// facts as large figures, a typical reply as a miniature letter, greetings and sign-offs over bars,
// tone and approach as cards, and phrases as chips with what is avoided struck through. Every value
// is the core's (the line widths, the placeholder runs, the shares, the first sentence a card is
// headed by); this lays them out and colours them, with brushes that follow the dialog's theme.

using Allodia.Mailcal.Controls;
using Allodia.Mailcal.Services;
using Microsoft.UI.Text;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Automation;
using Microsoft.UI.Xaml.Automation.Peers;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Documents;
using Microsoft.UI.Xaml.Media;
using uniffi.mailcal_bindings;
using Windows.UI;

namespace Allodia.Mailcal.Dialogs;

public sealed partial class SettingsDialog
{
    // What was read: the address, then how many messages, in how many languages, since when, the
    // figures large and read as one sentence.
    private UIElement FactsSection(WritingStyleFacts facts, RevealInk ink)
    {
        var panel = new StackPanel { Spacing = 12 };
        if (facts.LearnedFrom.Length > 0)
        {
            panel.Children.Add(Muted(L10n.RevealBasedOn(facts.LearnedFrom), ink));
        }
        var figures = new List<(string Label, string Value)>
        {
            (L10n.RevealStatMessages(), Number((int)facts.Messages)),
            (L10n.RevealStatLanguages(), Number(facts.Languages.Length)),
        };
        if (facts.Since is { } since)
        {
            figures.Add((L10n.RevealStatSince(), StyleDate(since)));
        }
        var row = new StackPanel { Orientation = Orientation.Horizontal };
        var texts = new List<TextBlock>();
        for (var index = 0; index < figures.Count; index++)
        {
            var (label, value) = figures[index];
            var stat = new StackPanel { Spacing = 2, Padding = new Thickness(index == 0 ? 0 : 20, 0, 20, 0) };
            var caption = new TextBlock { Text = label, FontWeight = FontWeights.SemiBold, Foreground = ink.Secondary };
            var figure = new TextBlock { Text = value, FontSize = 30, FontWeight = FontWeights.SemiBold };
            stat.Children.Add(caption);
            stat.Children.Add(figure);
            texts.Add(caption);
            texts.Add(figure);
            row.Children.Add(new Border
            {
                BorderBrush = ink.Separator,
                BorderThickness = new Thickness(index == 0 ? 0 : 1, 0, 0, 0),
                Child = stat,
            });
        }
        if (facts.Since is { } from)
        {
            // A panel has no automation peer, so the sentence rides on the first text and the rest
            // is left out of the tree.
            AutomationProperties.SetName(texts[0], L10n.RevealStatsA11y((int)facts.Messages, StyleDate(from), StyleLanguages(facts.Languages)));
            foreach (var text in texts.Skip(1))
            {
                AutomationProperties.SetAccessibilityView(text, AccessibilityView.Raw);
            }
        }
        panel.Children.Add(new Viewbox
        {
            Child = row,
            Stretch = Stretch.Uniform,
            StretchDirection = StretchDirection.DownOnly,
            HorizontalAlignment = HorizontalAlignment.Left,
        });
        var chips = new FlowPanel { Spacing = 8 };
        foreach (var code in facts.Languages)
        {
            chips.Children.Add(new Border
            {
                Background = ink.Fill,
                CornerRadius = new CornerRadius(14),
                Padding = new Thickness(12, 4, 12, 5),
                Child = new TextBlock { Text = StyleLanguage(code) },
            });
        }
        panel.Children.Add(chips);
        return panel;
    }

    // A typical reply: the usual greeting, grey lines in the usual length and paragraphs, the usual
    // sign-off and the name signed with; then its length and shape in words. The lines are a
    // picture, so they are not read out.
    private UIElement LetterSection(WritingStyleLetter letter, RevealInk ink)
    {
        var panel = new StackPanel { Spacing = 10 };
        var body = new StackPanel();
        if (letter.Opening.Length > 0)
        {
            var line = RunsLine(letter.Opening, 20, ink);
            line.Margin = new Thickness(0, 0, 0, 12);
            body.Children.Add(line);
        }
        var paragraphs = new StackPanel { Spacing = 12 };
        foreach (var paragraph in letter.Paragraphs)
        {
            var lines = new StackPanel { Spacing = 7 };
            foreach (var width in paragraph.Lines)
            {
                var bar = Bar(width, ink.Ink, 3.5);
                bar.Height = 7;
                lines.Children.Add(bar);
            }
            paragraphs.Children.Add(lines);
        }
        AutomationProperties.SetAccessibilityView(paragraphs, AccessibilityView.Raw);
        body.Children.Add(paragraphs);
        var close = new StackPanel { Spacing = 2, Margin = new Thickness(0, 18, 0, 0) };
        if (letter.Closing.Length > 0)
        {
            close.Children.Add(RunsLine(letter.Closing, 15, ink));
        }
        if (letter.SignsAs.Length > 0)
        {
            close.Children.Add(new TextBlock { Text = letter.SignsAs, FontSize = 15, TextWrapping = TextWrapping.Wrap });
        }
        if (close.Children.Count > 0)
        {
            body.Children.Add(close);
        }
        panel.Children.Add(Card(body, ink));
        var caption = Muted(L10n.RevealLetterCaption(), ink);
        caption.Style = TextStyle("CaptionTextBlockStyle");
        panel.Children.Add(caption);
        if (letter.Words > 0)
        {
            panel.Children.Add(LengthLine((int)letter.Words, ink));
        }
        AddFact(panel, L10n.RevealShape(), letter.Shape, ink);
        return panel;
    }

    // "Usually about 70 words", the number drawn large in the sentence the catalog words.
    private static TextBlock LengthLine(int words, RevealInk ink)
    {
        var sentence = L10n.RevealLength(words);
        var number = Number(words);
        var line = new TextBlock { TextWrapping = TextWrapping.Wrap, Foreground = ink.Secondary };
        var at = sentence.IndexOf(number, StringComparison.Ordinal);
        if (at < 0)
        {
            line.Text = sentence;
            return line;
        }
        line.Inlines.Add(new Run { Text = sentence[..at] });
        line.Inlines.Add(new Run { Text = number, FontSize = 26, FontWeight = FontWeights.SemiBold, Foreground = ink.Primary });
        line.Inlines.Add(new Run { Text = sentence[(at + number.Length)..] });
        return line;
    }

    // Greetings and sign-offs, each over a bar as long as its part of its list, with the word for
    // how often beside it; then the name signed with.
    private UIElement HabitsSection(WritingStyleHabits habits, RevealInk ink)
    {
        var panel = new StackPanel { Spacing = 16 };
        AddHabits(panel, L10n.RevealGreetings(), habits.Greetings, ink);
        AddHabits(panel, L10n.RevealSignOffs(), habits.SignOffs, ink);
        if (habits.SignsAs.Length > 0)
        {
            var signs = new StackPanel { Orientation = Orientation.Horizontal, Spacing = 12 };
            var label = Label(L10n.RevealSignsAs(), ink);
            label.VerticalAlignment = VerticalAlignment.Center;
            signs.Children.Add(label);
            signs.Children.Add(new TextBlock { Text = habits.SignsAs, FontSize = 18, FontWeight = FontWeights.SemiBold });
            panel.Children.Add(signs);
        }
        return panel;
    }

    private static void AddHabits(StackPanel page, string label, WritingStyleHabit[] habits, RevealInk ink)
    {
        if (habits.Length == 0)
        {
            return;
        }
        var group = new StackPanel { Spacing = 5 };
        var heading = Label(label, ink);
        AutomationProperties.SetHeadingLevel(heading, AutomationHeadingLevel.Level3);
        group.Children.Add(heading);
        for (var index = 0; index < habits.Length; index++)
        {
            var habit = habits[index];
            var row = new Grid { MinHeight = 34 };
            row.Children.Add(new Border { Background = ink.Track, CornerRadius = new CornerRadius(6) });
            row.Children.Add(Bar(habit.Relative / 100.0, index == 0 ? ink.AccentSoft : ink.FillStrong, 6));
            var text = new Grid { Padding = new Thickness(12, 4, 12, 4), ColumnSpacing = 12 };
            text.ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(1, GridUnitType.Star) });
            text.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });
            var words = RunsLine(habit.Runs, 14, ink);
            words.VerticalAlignment = VerticalAlignment.Center;
            var frequency = new TextBlock
            {
                Text = WritingStyleRules.Frequency(habit.Frequency, StyleWords()),
                Style = TextStyle("CaptionTextBlockStyle"),
                Foreground = ink.Secondary,
                VerticalAlignment = VerticalAlignment.Center,
            };
            Grid.SetColumn(frequency, 1);
            text.Children.Add(words);
            text.Children.Add(frequency);
            row.Children.Add(text);
            group.Children.Add(row);
        }
        page.Children.Add(group);
    }

    // Tone and approach: a card per topic, its heading over the text beneath it.
    private static UIElement CardsSection(WritingStyleCard[] cards, RevealInk ink)
    {
        var panel = new StackPanel { Spacing = 10 };
        foreach (var card in cards)
        {
            var content = new StackPanel { Spacing = 4 };
            content.Children.Add(Label(card.Topic switch
            {
                WritingStyleTopic.Register => L10n.RevealRegister(),
                WritingStyleTopic.Structure => L10n.RevealStructure(),
                WritingStyleTopic.Moves => L10n.RevealMoves(),
                _ => L10n.RevealPunctuation(),
            }, ink));
            content.Children.Add(new TextBlock { Text = card.Heading, FontSize = 16, FontWeight = FontWeights.SemiBold, TextWrapping = TextWrapping.Wrap });
            if (card.Body.Length > 0)
            {
                content.Children.Add(new TextBlock { Text = card.Body, TextWrapping = TextWrapping.Wrap });
            }
            panel.Children.Add(Card(new Border
            {
                BorderBrush = ink.Accent,
                BorderThickness = new Thickness(3, 0, 0, 0),
                Padding = new Thickness(12, 0, 0, 0),
                Child = content,
            }, ink));
        }
        return panel;
    }

    // Phrases the person uses, as chips, then what they avoid, struck through.
    private static UIElement PhrasesSection(WritingStylePhrases phrases, RevealInk ink)
    {
        var panel = new StackPanel { Spacing = 10 };
        void Cloud(string? heading, string[] list, bool avoided)
        {
            if (list.Length == 0)
            {
                return;
            }
            if (heading is not null)
            {
                var label = Label(heading, ink);
                AutomationProperties.SetHeadingLevel(label, AutomationHeadingLevel.Level3);
                panel.Children.Add(label);
            }
            var cloud = new FlowPanel { Spacing = 8 };
            foreach (var phrase in list)
            {
                cloud.Children.Add(new Border
                {
                    BorderBrush = avoided ? ink.Separator : ink.Stroke,
                    Background = avoided ? null : ink.Card,
                    BorderThickness = new Thickness(1),
                    CornerRadius = new CornerRadius(16),
                    Padding = new Thickness(14, 5, 14, 6),
                    Child = new TextBlock
                    {
                        Text = phrase,
                        TextWrapping = TextWrapping.Wrap,
                        TextDecorations = avoided ? Windows.UI.Text.TextDecorations.Strikethrough : Windows.UI.Text.TextDecorations.None,
                        Foreground = avoided ? ink.Secondary : ink.Primary,
                    },
                });
            }
            panel.Children.Add(cloud);
        }
        Cloud(null, phrases.Used, avoided: false);
        Cloud(L10n.RevealAvoid(), phrases.Avoided, avoided: true);
        return panel;
    }

    // A greeting or sign-off, a placeholder such as [Name] drawn as a small pill in the accent.
    private static FrameworkElement RunsLine(WritingStyleSpan[] runs, double size, RevealInk ink)
    {
        if (runs.All(run => !run.Placeholder))
        {
            return new TextBlock { Text = string.Concat(runs.Select(run => run.Text)), FontSize = size, TextWrapping = TextWrapping.Wrap };
        }
        var line = new StackPanel { Orientation = Orientation.Horizontal };
        foreach (var run in runs)
        {
            line.Children.Add(run.Placeholder
                ? new Border
                {
                    Background = ink.AccentFaint,
                    BorderBrush = ink.AccentLine,
                    BorderThickness = new Thickness(1),
                    CornerRadius = new CornerRadius(9),
                    Padding = new Thickness(7, 0, 7, 1),
                    Margin = new Thickness(1, 0, 1, 0),
                    VerticalAlignment = VerticalAlignment.Center,
                    Child = new TextBlock { Text = run.Text, FontSize = 12, FontWeight = FontWeights.SemiBold, Foreground = ink.Accent },
                }
                : new TextBlock { Text = run.Text, FontSize = size, VerticalAlignment = VerticalAlignment.Center });
        }
        return line;
    }

    private static void AddFact(StackPanel page, string label, string value, RevealInk ink)
    {
        if (value.Length == 0)
        {
            return;
        }
        var fact = new StackPanel { Spacing = 2 };
        fact.Children.Add(Label(label, ink));
        fact.Children.Add(new TextBlock { Text = value, TextWrapping = TextWrapping.Wrap });
        page.Children.Add(fact);
    }

    // A bar from the leading edge to `fraction` of the width, as two star columns, so it needs no
    // measuring and follows the panel's width.
    private static Grid Bar(double fraction, Brush fill, double radius)
    {
        var share = Math.Clamp(fraction, 0, 1);
        var bar = new Grid();
        bar.ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(share, GridUnitType.Star) });
        bar.ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(1 - share, GridUnitType.Star) });
        bar.Children.Add(new Border { Background = fill, CornerRadius = new CornerRadius(radius) });
        return bar;
    }

    private static TextBlock Label(string text, RevealInk ink) =>
        new() { Text = text, FontWeight = FontWeights.SemiBold, Foreground = ink.Secondary, TextWrapping = TextWrapping.Wrap };

    private static TextBlock Muted(string text, RevealInk ink) =>
        new() { Text = text, Foreground = ink.Secondary, TextWrapping = TextWrapping.Wrap };

    private static Border Card(UIElement content, RevealInk ink) => new()
    {
        Background = ink.Card,
        BorderBrush = ink.Stroke,
        BorderThickness = new Thickness(1),
        CornerRadius = new CornerRadius(8),
        Padding = new Thickness(16, 14, 16, 14),
        Child = content,
    };

    private static Style TextStyle(string key) => (Style)Application.Current.Resources[key];

    // The colours a style is drawn with, as brushes that follow the dialog's theme: Fluent's subtle
    // and strong fills for the neutral ones, the desktop's accent, lightened, for the accented ones.
    private sealed class RevealInk(ThemeBrushes brushes)
    {
        internal Brush Primary { get; } = brushes.Of(ThemePalette.PrimaryText);
        internal Brush Secondary { get; } = brushes.Of(ThemePalette.SecondaryText);
        internal Brush Separator { get; } = brushes.Of(ThemePalette.Divider);
        internal Brush Stroke { get; } = brushes.Of(ThemePalette.ControlStroke);
        internal Brush Card { get; } = brushes.Of(ThemePalette.CardBackground);
        internal Brush Track { get; } = brushes.Of(dark => Neutral(dark, light: 0x08, night: 0x0F));
        internal Brush Fill { get; } = brushes.Of(dark => Neutral(dark, light: 0x0F, night: 0x15));
        internal Brush FillStrong { get; } = brushes.Of(dark => Neutral(dark, light: 0x1A, night: 0x28));
        internal Brush Ink { get; } = brushes.Of(dark => Neutral(dark, light: 0x24, night: 0x29));
        internal Brush Accent { get; } = brushes.Of(ThemePalette.AccentText);
        internal Brush AccentSoft { get; } = brushes.Of(dark => Accented(dark, light: 0x2E, night: 0x52));
        internal Brush AccentFaint { get; } = brushes.Of(dark => Accented(dark, light: 0x17, night: 0x29));
        internal Brush AccentLine { get; } = brushes.Of(dark => Accented(dark, light: 0x66, night: 0x66));

        private static Color Neutral(bool dark, byte light, byte night) =>
            dark ? Color.FromArgb(night, 0xFF, 0xFF, 0xFF) : Color.FromArgb(light, 0x00, 0x00, 0x00);

        private static Color Accented(bool dark, byte light, byte night)
        {
            var accent = ThemePalette.AccentText(dark);
            return Color.FromArgb(dark ? night : light, accent.R, accent.G, accent.B);
        }
    }
}
