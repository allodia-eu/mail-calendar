// The reveal's first three pages: what was read, a typical reply as a miniature letter, and the
// greetings and sign-offs as bars (docs/ai.md, "Learning" step 5). The arithmetic behind the
// pictures is StyleReveal; the colours are RevealInk, handed out as brushes that follow the theme,
// because a panel built in code keeps the colours it was built with (ThemePalette.cs).

using Allodia.Mailcal.Controls;
using Allodia.Mailcal.Services;
using Microsoft.UI.Text;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Automation;
using Microsoft.UI.Xaml.Automation.Peers;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Documents;
using Microsoft.UI.Xaml.Media;
using Microsoft.UI.Xaml.Shapes;
using uniffi.mailcal_bindings;
using Windows.UI;

namespace Allodia.Mailcal.Dialogs;

public sealed partial class SettingsDialog
{
    // A language's dot beside its name on the first page: the accent for the main one, then colours
    // from the palette Windows offers as accents, so each reads apart from the next.
    private static readonly Color[] LanguageDots =
    [
        Color.FromArgb(0xFF, 0x87, 0x64, 0xB8),
        Color.FromArgb(0xFF, 0x03, 0x83, 0x87),
        Color.FromArgb(0xFF, 0xCA, 0x50, 0x10),
        Color.FromArgb(0xFF, 0xC2, 0x39, 0xB3),
        Color.FromArgb(0xFF, 0x49, 0x82, 0x05),
    ];

    // Step 1: how many messages in how many languages since when, and from which account.
    private UIElement ReadPage(WritingStyleDetail detail, RevealInk ink)
    {
        var page = WizardFrame.Page(L10n.RevealTitle());
        if (SourceAddress(detail) is { } address)
        {
            page.Children.Add(Muted(L10n.RevealBasedOn(address), ink));
        }
        var codes = detail.Languages.Select(language => language.Language).ToArray();
        page.Children.Add(Stats(detail, codes, ink));
        var chips = LanguageChips(codes, ink);
        // The row above already names the languages when it is read as one sentence.
        if (detail.Row.Oldest is not null)
        {
            foreach (var chip in chips.Children)
            {
                AutomationProperties.SetAccessibilityView(chip, AccessibilityView.Raw);
            }
        }
        page.Children.Add(chips);
        var intro = Muted(L10n.RevealPagesIntro(), ink);
        intro.Margin = new Thickness(0, 12, 0, 0);
        page.Children.Add(intro);
        return page;
    }

    // The address of the account the style was learned from, while that account is still here; the
    // row carries an account id, and nothing when the style came from another device.
    private string? SourceAddress(WritingStyleDetail detail) =>
        string.IsNullOrEmpty(detail.Row.SourceAccount)
            ? null
            : _model.WritingStyles.Accounts.FirstOrDefault(account => account.AccountId == detail.Row.SourceAccount)?.Email;

    // The three figures, large, each under its label. Scaled down rather than cut off when a long
    // month name does not fit beside the counts. Read as one sentence when there is a date to say
    // it with.
    private UIElement Stats(WritingStyleDetail detail, string[] codes, RevealInk ink)
    {
        var zone = WritingStyleFormat.Zone(_model.ActiveZone);
        var figures = new List<(string Label, string Value)>
        {
            (L10n.RevealStatMessages(), detail.Row.Messages.ToString(Culture)),
            (L10n.RevealStatLanguages(), codes.Length.ToString(Culture)),
        };
        if (detail.Row.Oldest is { } oldest)
        {
            figures.Add((L10n.RevealStatSince(), StyleReveal.Since(oldest, DateTimeOffset.UtcNow, zone, Culture)));
        }
        var row = new StackPanel { Orientation = Orientation.Horizontal };
        var texts = new List<TextBlock>();
        for (var index = 0; index < figures.Count; index++)
        {
            var (label, value) = figures[index];
            var stat = new StackPanel { Spacing = 2, Padding = new Thickness(index == 0 ? 0 : 20, 0, 20, 0) };
            var caption = new TextBlock { Text = label, FontWeight = FontWeights.SemiBold, Foreground = ink.Secondary };
            var figure = new TextBlock { Text = value, FontSize = 34, FontWeight = FontWeights.SemiBold };
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
        if (detail.Row.Oldest is { } since)
        {
            // A panel has no automation peer, so the sentence rides on the first text and the rest
            // is left out of the tree.
            AutomationProperties.SetName(texts[0], L10n.RevealStatsA11y(
                (int)detail.Row.Messages,
                WritingStyleText.Date(since, _model.ActiveZone),
                WritingStyleText.Languages(codes)));
            foreach (var text in texts.Skip(1))
            {
                AutomationProperties.SetAccessibilityView(text, AccessibilityView.Raw);
            }
        }
        return new Viewbox
        {
            Child = row,
            Stretch = Stretch.Uniform,
            StretchDirection = StretchDirection.DownOnly,
            HorizontalAlignment = HorizontalAlignment.Left,
            Margin = new Thickness(0, 8, 0, 0),
        };
    }

    private static FlowPanel LanguageChips(string[] codes, RevealInk ink)
    {
        var chips = new FlowPanel { Spacing = 8 };
        for (var index = 0; index < codes.Length; index++)
        {
            var dot = new Ellipse
            {
                Width = 8,
                Height = 8,
                Fill = index == 0 ? ink.Accent : new SolidColorBrush(LanguageDots[(index - 1) % LanguageDots.Length]),
                VerticalAlignment = VerticalAlignment.Center,
            };
            var content = new StackPanel { Orientation = Orientation.Horizontal, Spacing = 8 };
            content.Children.Add(dot);
            content.Children.Add(new TextBlock { Text = WritingStyleText.LanguageName(codes[index]) });
            chips.Children.Add(new Border
            {
                Background = ink.Fill,
                CornerRadius = new CornerRadius(14),
                Padding = new Thickness(10, 4, 12, 4),
                Child = content,
            });
        }
        return chips;
    }

    // Step 2: a typical reply as a miniature letter, then its length, shape and punctuation.
    private UIElement LetterPage(LanguageStyleRow style, RevealInk ink)
    {
        var page = WizardFrame.Page(L10n.RevealStepLetter());
        page.Children.Add(Letter(style, ink));
        var caption = Muted(L10n.RevealLetterCaption(), ink);
        caption.Style = TextStyle("CaptionTextBlockStyle");
        caption.Margin = new Thickness(2, -6, 0, 0);
        page.Children.Add(caption);
        if (style.TypicalWords > 0)
        {
            page.Children.Add(LengthLine((int)style.TypicalWords, ink));
        }
        AddFact(page, L10n.RevealShape(), style.Shape, ink);
        AddFact(page, L10n.RevealPunctuation(), style.Punctuation, ink);
        return page;
    }

    // The letter: the usual greeting, grey lines for the usual length and paragraphs, then the
    // usual sign-off and the name signed with. The lines are a picture, so they are not read out.
    private static Border Letter(LanguageStyleRow style, RevealInk ink)
    {
        var letter = new StackPanel();
        if (style.Greetings.FirstOrDefault() is { } greeting)
        {
            var line = RunsLine(StyleReveal.Runs(greeting.Text), 22, ink);
            line.Margin = new Thickness(0, 0, 0, 12);
            letter.Children.Add(line);
        }
        var paragraphs = new StackPanel { Spacing = 12 };
        foreach (var paragraph in StyleReveal.LetterLines(style.TypicalWords, style.TypicalParagraphs))
        {
            var lines = new StackPanel { Spacing = 7 };
            foreach (var width in paragraph)
            {
                var bar = Bar(width, ink.Ink, 3.5);
                bar.Height = 7;
                lines.Children.Add(bar);
            }
            paragraphs.Children.Add(lines);
        }
        letter.Children.Add(paragraphs);
        var close = new StackPanel { Spacing = 2, Margin = new Thickness(0, 20, 0, 0) };
        if (style.SignOffs.FirstOrDefault() is { } signOff)
        {
            close.Children.Add(RunsLine(StyleReveal.Runs(signOff.Text), 15, ink));
        }
        if (!string.IsNullOrWhiteSpace(style.SignsAs))
        {
            close.Children.Add(new TextBlock { Text = style.SignsAs, FontSize = 15, TextWrapping = TextWrapping.Wrap });
        }
        if (close.Children.Count > 0)
        {
            letter.Children.Add(close);
        }
        return Card(letter, ink);
    }

    // "Usually about 70 words", the number drawn large in the sentence the catalog words.
    private static TextBlock LengthLine(int words, RevealInk ink)
    {
        var sentence = L10n.RevealLength(words);
        var number = words.ToString(Culture);
        var line = new TextBlock { TextWrapping = TextWrapping.Wrap, Foreground = ink.Secondary };
        var at = sentence.IndexOf(number, StringComparison.Ordinal);
        if (at < 0)
        {
            line.Text = sentence;
            return line;
        }
        line.Inlines.Add(new Run { Text = sentence[..at] });
        line.Inlines.Add(new Run
        {
            Text = number,
            FontSize = 28,
            FontWeight = FontWeights.SemiBold,
            Foreground = ink.Primary,
        });
        line.Inlines.Add(new Run { Text = sentence[(at + number.Length)..] });
        return line;
    }

    private static void AddFact(StackPanel page, string label, string value, RevealInk ink)
    {
        if (string.IsNullOrWhiteSpace(value))
        {
            return;
        }
        var fact = new StackPanel { Spacing = 2 };
        fact.Children.Add(Label(label, ink));
        fact.Children.Add(new TextBlock { Text = value, TextWrapping = TextWrapping.Wrap });
        page.Children.Add(fact);
    }

    // Step 3: greetings and sign-offs, each over a bar as long as its part of its list, the word
    // beside it saying roughly how often.
    private static UIElement HabitsPage(LanguageStyleRow style, RevealInk ink)
    {
        var page = WizardFrame.Page(L10n.RevealStepHabits());
        AddHabits(page, L10n.RevealGreetings(), style.Greetings, ink);
        AddHabits(page, L10n.RevealSignOffs(), style.SignOffs, ink);
        if (!string.IsNullOrWhiteSpace(style.SignsAs))
        {
            var signs = new StackPanel { Orientation = Orientation.Horizontal, Spacing = 12 };
            var label = Label(L10n.RevealSignsAs(), ink);
            label.VerticalAlignment = VerticalAlignment.Center;
            signs.Children.Add(label);
            signs.Children.Add(new TextBlock { Text = style.SignsAs, FontSize = 18, FontWeight = FontWeights.SemiBold });
            page.Children.Add(signs);
        }
        return page;
    }

    private static void AddHabits(StackPanel page, string label, HabitRow[] habits, RevealInk ink)
    {
        var shown = habits.Where(habit => !string.IsNullOrWhiteSpace(habit.Text)).ToArray();
        if (shown.Length == 0)
        {
            return;
        }
        var group = new StackPanel { Spacing = 5 };
        var heading = Label(label, ink);
        heading.Margin = new Thickness(0, 0, 0, 1);
        AutomationProperties.SetHeadingLevel(heading, AutomationHeadingLevel.Level3);
        group.Children.Add(heading);
        for (var index = 0; index < shown.Length; index++)
        {
            group.Children.Add(HabitBar(shown[index], top: index == 0, ink));
        }
        page.Children.Add(group);
    }

    private static Grid HabitBar(HabitRow habit, bool top, RevealInk ink)
    {
        var row = new Grid { MinHeight = 34 };
        row.Children.Add(new Border { Background = ink.Track, CornerRadius = new CornerRadius(6) });
        row.Children.Add(Bar(habit.Relative / 100.0, top ? ink.AccentSoft : ink.FillStrong, 6));
        var text = new Grid { Padding = new Thickness(12, 4, 12, 4), ColumnSpacing = 12 };
        text.ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(1, GridUnitType.Star) });
        text.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });
        var words = RunsLine(StyleReveal.Runs(habit.Text), 14, ink);
        words.VerticalAlignment = VerticalAlignment.Center;
        var frequency = new TextBlock
        {
            Text = WritingStyleText.Frequency(habit.Frequency),
            Style = TextStyle("CaptionTextBlockStyle"),
            Foreground = ink.Secondary,
            VerticalAlignment = VerticalAlignment.Center,
        };
        Grid.SetColumn(frequency, 1);
        text.Children.Add(words);
        text.Children.Add(frequency);
        row.Children.Add(text);
        return row;
    }

    // A greeting or sign-off, a placeholder such as [Name] drawn as a small pill in the accent.
    private static FrameworkElement RunsLine(IReadOnlyList<RevealRun> runs, double size, RevealInk ink)
    {
        if (runs.All(run => !run.Placeholder))
        {
            return new TextBlock
            {
                Text = string.Concat(runs.Select(run => run.Text)),
                FontSize = size,
                TextWrapping = TextWrapping.Wrap,
            };
        }
        var line = new StackPanel { Orientation = Orientation.Horizontal };
        foreach (var run in runs)
        {
            line.Children.Add(run.Placeholder
                ? Pill(run.Text, ink)
                : new TextBlock { Text = run.Text, FontSize = size, VerticalAlignment = VerticalAlignment.Center });
        }
        return line;
    }

    private static Border Pill(string text, RevealInk ink) => new()
    {
        Background = ink.AccentFaint,
        BorderBrush = ink.AccentLine,
        BorderThickness = new Thickness(1),
        CornerRadius = new CornerRadius(9),
        Padding = new Thickness(7, 0, 7, 1),
        Margin = new Thickness(1, 0, 1, 0),
        VerticalAlignment = VerticalAlignment.Center,
        Child = new TextBlock
        {
            Text = text,
            FontSize = 12,
            FontWeight = FontWeights.SemiBold,
            Foreground = ink.Accent,
        },
    };

    // A bar drawn from the leading edge to `fraction` of the width, as two star columns, so it
    // needs no measuring and follows the panel's width.
    private static Grid Bar(double fraction, Brush fill, double radius)
    {
        var share = Math.Clamp(fraction, 0, 1);
        var bar = new Grid();
        bar.ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(share, GridUnitType.Star) });
        bar.ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(1 - share, GridUnitType.Star) });
        bar.Children.Add(new Border { Background = fill, CornerRadius = new CornerRadius(radius) });
        return bar;
    }

    private static TextBlock Label(string text, RevealInk ink) => new()
    {
        Text = text,
        FontWeight = FontWeights.SemiBold,
        Foreground = ink.Secondary,
        TextWrapping = TextWrapping.Wrap,
    };

    private static TextBlock Muted(string text, RevealInk ink) => new()
    {
        Text = text,
        Foreground = ink.Secondary,
        TextWrapping = TextWrapping.Wrap,
    };

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
}
