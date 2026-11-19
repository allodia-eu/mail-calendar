// A new style shown back once (docs/ai.md rule 15), from the core's writing_style_reveal(style): its
// pages in the flow host with a PipsPager, after learning in the flow's own host and from a library
// row in one of its own. Which pages there are and what is on each is the record's; the host keeps
// only which page is on screen and, on a page per language, which language. Leaving it, by Save or
// by closing, counts it as shown.

using Allodia.Mailcal.Controls;
using Allodia.Mailcal.Services;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Automation;
using Microsoft.UI.Xaml.Controls;
using uniffi.mailcal_bindings;

namespace Allodia.Mailcal.Dialogs;

public sealed partial class SettingsDialog
{
    private RevealParts? _revealParts;

    /// <summary>Draws <paramref name="reveal"/>, in the flow's host when it follows learning, and
    /// returns the host.</summary>
    private UIElement RevealHost(WritingStyleReveal reveal, bool inFlow)
    {
        if (_revealParts is not { } parts || parts.Style != reveal.Id || parts.InFlow != inFlow)
        {
            var host = inFlow && _learningHost is { } flow ? flow : new FlowHost("WritingStyleReveal", FlowHeight());
            parts = new RevealParts(reveal, host, inFlow, new RevealInk(_brushes));
            parts.Name.Header = L10n.WritingStyleNameLabel();
            parts.Notes.Header = L10n.WritingStyleNotes();
            parts.Notes.PlaceholderText = L10n.WritingStyleNotesHint();
            AutomationProperties.SetName(parts.Name, L10n.WritingStyleNameLabel());
            AutomationProperties.SetName(parts.Notes, L10n.WritingStyleNotes());
            AutomationProperties.SetAutomationId(parts.Name, "WritingStyleRevealName");
            AutomationProperties.SetAutomationId(parts.Notes, "WritingStyleRevealNotes");
            parts.Name.TextChanged += (_, _) => DrawReveal(parts);
            _revealParts = parts;
        }
        DrawReveal(parts);
        return parts.Host.Root;
    }

    private bool RefreshReveal(RevealParts parts, WritingStyleReveal reveal)
    {
        parts.Record = reveal;
        if (_styleOutcomes.Fresh(_model.WritingStylePage()?.Outcome) is { Style: var style } outcome && style == parts.Style)
        {
            switch (outcome.Result)
            {
                case WritingStyleResult.Refused refused:
                    parts.Said = WritingStyleRules.Say(refused.Reason, StyleWords());
                    break;
                case WritingStyleResult.Done or WritingStyleResult.NotWritten when !parts.InFlow:
                    // Left, by Save or by closing: the page, whose row now opens the detail.
                    _styleScreen = StyleScreen.Page;
                    _revealParts = null;
                    return false;
            }
        }
        DrawReveal(parts);
        return true;
    }

    private void DrawReveal(RevealParts parts)
    {
        var reveal = parts.Record;
        var count = reveal.Pages.Length;
        parts.At = Math.Clamp(parts.At, 0, Math.Max(count - 1, 0));
        var page = count > 0 ? reveal.Pages[parts.At] : WritingStyleRevealPage.Name;
        var key = $"{parts.At}:{parts.Language}";
        parts.SaidLine.Text = parts.Said ?? string.Empty;
        parts.SaidLine.Visibility = parts.Said is null ? Visibility.Collapsed : Visibility.Visible;
        if (key != parts.ShownKey)
        {
            parts.ShownKey = key;
            parts.Shown = RevealPage(parts, page);
        }
        var close = new FlowAction(L10n.ActionClose(), reveal.Close, () => LeaveReveal(parts));
        // Turning the pages, the pips' included, is held by the record's paging.
        var back = parts.At > 0 ? new FlowAction(L10n.WizardBack(), reveal.Paging, () => GoReveal(parts, parts.At - 1)) : null;
        var primary = page == WritingStyleRevealPage.Name
            ? new FlowAction(L10n.RevealSave(), WritingStyleRules.NameSave(reveal.Save, MailboxModel.StyleNameRefusal(parts.Name.Text)), () =>
            {
                parts.Said = null;
                _model.ChangeWritingStyle(new WritingStyleIntent.SaveReveal(parts.Style, parts.Name.Text, parts.Notes.Text));
            })
            : new FlowAction(L10n.WizardNext(), reveal.Paging, () => GoReveal(parts, parts.At + 1));
        ControlComponents.HoldFields(reveal.Save, parts.Name, parts.Notes);
        var steps = new FlowSteps(count, parts.At, L10n.A11yFlowPage(Number(parts.At + 1), count),
            reveal.Paging.Enabled ? at => GoReveal(parts, at) : null);
        parts.Host.Draw(new FlowFrame(RevealTitle(page), close, back, primary, steps), parts.Shown!);
    }

    private void GoReveal(RevealParts parts, int at)
    {
        parts.At = at;
        DrawReveal(parts);
    }

    // Closing leaves it shown: the core ends the flow it followed, and a row's reveal goes back to
    // the page at once.
    private void LeaveReveal(RevealParts parts)
    {
        _model.ChangeWritingStyle(new WritingStyleIntent.CloseReveal(parts.Style));
        if (!parts.InFlow)
        {
            _styleScreen = StyleScreen.Page;
            _revealParts = null;
            ShowCategory(WritingStyleTag);
        }
    }

    private static string RevealTitle(WritingStyleRevealPage page) => page switch
    {
        WritingStyleRevealPage.Read => L10n.RevealTitle(),
        WritingStyleRevealPage.Letter => L10n.RevealStepLetter(),
        WritingStyleRevealPage.Habits => L10n.RevealStepHabits(),
        WritingStyleRevealPage.Voice => L10n.RevealStepVoice(),
        WritingStyleRevealPage.Phrases => L10n.RevealPhrases(),
        _ => L10n.RevealStepName(),
    };

    private UIElement RevealPage(RevealParts parts, WritingStyleRevealPage page)
    {
        var reveal = parts.Record;
        var panel = new StackPanel { Spacing = 14 };
        switch (page)
        {
            case WritingStyleRevealPage.Read:
                panel.Children.Add(FactsSection(reveal.Facts, parts.Ink));
                if (reveal.Pages.Length > 2)
                {
                    panel.Children.Add(Muted(L10n.RevealPagesIntro(), parts.Ink));
                }
                return panel;
            case WritingStyleRevealPage.Name:
                // Built once: its fields keep what is typed as the pages turn.
                if (parts.NamePage.Children.Count == 0)
                {
                    AutomationProperties.SetAutomationId(parts.SaidLine, "WritingStyleRevealRefused");
                    parts.NamePage.Children.Add(parts.Name);
                    parts.NamePage.Children.Add(parts.SaidLine);
                    parts.NamePage.Children.Add(parts.Notes);
                }
                return parts.NamePage;
        }
        // A page per language: a control over it while there is more than one, and the language's
        // own section, or a line saying nothing stood out in it.
        if (reveal.Languages.Length == 0)
        {
            return panel;
        }
        parts.Language = Math.Clamp(parts.Language, 0, reveal.Languages.Length - 1);
        if (reveal.Languages.Length > 1)
        {
            panel.Children.Add(LanguageBar(reveal.Languages, parts));
        }
        var language = reveal.Languages[parts.Language];
        UIElement? section = page switch
        {
            WritingStyleRevealPage.Letter => language.Letter is { } letter ? LetterSection(letter, parts.Ink) : null,
            WritingStyleRevealPage.Habits => language.Habits is { } habits ? HabitsSection(habits, parts.Ink) : null,
            WritingStyleRevealPage.Voice => language.Cards.Length > 0 ? CardsSection(language.Cards, parts.Ink) : null,
            WritingStyleRevealPage.Phrases => language.Phrases is { } phrases ? PhrasesSection(phrases, parts.Ink) : null,
            _ => null,
        };
        panel.Children.Add(section ?? Muted(L10n.RevealNothingInLanguage(), parts.Ink));
        return panel;
    }

    // Which language a per-language page shows: the platform's segmented control, one item each.
    private SelectorBar LanguageBar(WritingStyleLanguage[] languages, RevealParts parts)
    {
        var bar = new SelectorBar();
        AutomationProperties.SetAutomationId(bar, "WritingStyleRevealLanguages");
        AutomationProperties.SetName(bar, L10n.RevealStatLanguages());
        foreach (var language in languages)
        {
            bar.Items.Add(new SelectorBarItem { Text = StyleLanguage(language.Language) });
        }
        bar.SelectedItem = bar.Items[parts.Language];
        bar.SelectionChanged += (_, _) =>
        {
            var at = bar.Items.IndexOf(bar.SelectedItem);
            if (at >= 0 && at != parts.Language)
            {
                parts.Language = at;
                DrawReveal(parts);
            }
        };
        return bar;
    }

    // The reveal on screen: what it was drawn from, its host, the page and language on screen, and
    // the name and notes, which keep what is typed as the pages turn.
    private sealed class RevealParts(WritingStyleReveal record, FlowHost host, bool inFlow, RevealInk ink)
    {
        internal string Style { get; } = record.Id;

        internal bool InFlow { get; } = inFlow;

        internal FlowHost Host { get; } = host;

        internal RevealInk Ink { get; } = ink;

        internal WritingStyleReveal Record { get; set; } = record;

        internal int At { get; set; }

        internal int Language { get; set; }

        internal string? ShownKey { get; set; }

        internal UIElement? Shown { get; set; }

        internal string? Said { get; set; }

        internal StackPanel NamePage { get; } = new() { Spacing = 14 };

        internal TextBlock SaidLine { get; } = new() { TextWrapping = TextWrapping.Wrap, Visibility = Visibility.Collapsed };

        internal TextBox Name { get; } = new() { Text = record.Name, MinWidth = 320, HorizontalAlignment = HorizontalAlignment.Left };

        internal TextBox Notes { get; } = new()
        {
            Text = record.Notes,
            AcceptsReturn = true,
            TextWrapping = TextWrapping.Wrap,
            Height = 120,
        };
    }
}
