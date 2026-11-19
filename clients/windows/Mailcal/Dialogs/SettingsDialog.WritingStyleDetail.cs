// An existing style's detail (docs/ai.md rule 16), from the core's writing_style_detail(style): the
// facts, the name with its Save, the notes sent on Return and on leaving the field, the reveal's
// sections on one page without paging, and Forget, asked first and naming the accounts that draft
// in the style. Every control's state is the record's.

using Allodia.Mailcal.Controls;
using Allodia.Mailcal.Services;
using Microsoft.UI.Text;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Automation;
using Microsoft.UI.Xaml.Controls;
using uniffi.mailcal_bindings;

namespace Allodia.Mailcal.Dialogs;

public sealed partial class SettingsDialog
{
    private DetailParts? _detailParts;

    private UIElement DetailPanel(WritingStyleDetail detail)
    {
        var ink = new RevealInk(_brushes);
        var parts = new DetailParts(detail);
        parts.Fill();
        parts.Forgotten = () => _model.ChangeWritingStyle(new WritingStyleIntent.Forget(parts.Style));
        _detailParts = parts;
        var panel = new StackPanel { Spacing = 20 };

        // Where the detail sits, as Windows' own Settings says it: the category, then the style. The
        // category goes back to the page.
        var path = new BreadcrumbBar
        {
            ItemsSource = new[] { L10n.SettingsCategoryWritingStyle(), detail.Name },
        };
        path.Resources["BreadcrumbBarItemThemeFontSize"] = 24.0;
        path.Resources["BreadcrumbBarItemFontWeight"] = FontWeights.SemiBold;
        AutomationProperties.SetAutomationId(path, "WritingStyleDetailPath");
        path.ItemClicked += (_, e) =>
        {
            if (e.Index == 0)
            {
                _styleScreen = StyleScreen.Page;
                ShowCategory(WritingStyleTag);
            }
        };
        panel.Children.Add(path);
        panel.Children.Add(FactsSection(detail.Facts, ink));

        // The name and its Save, held while the core refuses the name typed.
        parts.Name.Header = L10n.WritingStyleNameLabel();
        AutomationProperties.SetName(parts.Name, L10n.WritingStyleNameLabel());
        AutomationProperties.SetAutomationId(parts.Name, "WritingStyleDetailName");
        parts.Name.TextChanged += (_, _) => DrawDetailControls(parts);
        var name = new StackPanel { Orientation = Orientation.Horizontal, Spacing = 8 };
        name.Children.Add(parts.Name);
        parts.RenameSlot.VerticalAlignment = VerticalAlignment.Bottom;
        name.Children.Add(parts.RenameSlot);
        var nameGroup = new StackPanel { Spacing = 4 };
        nameGroup.Children.Add(name);
        AutomationProperties.SetAutomationId(parts.Said, "WritingStyleDetailRefused");
        nameGroup.Children.Add(parts.Said);
        panel.Children.Add(nameGroup);

        panel.Children.Add(Group(L10n.WritingStyleNotes(), L10n.WritingStyleNotesHint(), parts.NotesSlot));

        foreach (var language in detail.Languages)
        {
            panel.Children.Add(LanguageSections(language, detail.Languages.Length > 1, detail.Sections, ink));
        }

        panel.Children.Add(parts.ForgetSlot);
        panel.Children.Add(parts.Forget.Element);
        DrawDetailControls(parts);
        return panel;
    }

    // One language's sections, under its name when the style has more than one.
    private UIElement LanguageSections(WritingStyleLanguage language, bool named, WritingStyleSection[] sections, RevealInk ink)
    {
        var panel = new StackPanel { Spacing = 16 };
        if (named)
        {
            panel.Children.Add(new TextBlock { Text = StyleLanguage(language.Language), FontSize = 18, FontWeight = FontWeights.SemiBold });
        }
        void Section(WritingStyleSection which, string heading, Func<UIElement?> body)
        {
            // Only the sections the record names, and of those only what this language has.
            if (!sections.Contains(which) || body() is not { } drawn)
            {
                return;
            }
            var section = new StackPanel { Spacing = 8 };
            var label = Heading(heading);
            AutomationProperties.SetHeadingLevel(label, Microsoft.UI.Xaml.Automation.Peers.AutomationHeadingLevel.Level3);
            section.Children.Add(label);
            section.Children.Add(drawn);
            panel.Children.Add(section);
        }
        Section(WritingStyleSection.Letter, L10n.RevealStepLetter(), () => language.Letter is { } letter ? LetterSection(letter, ink) : null);
        Section(WritingStyleSection.Habits, L10n.RevealStepHabits(), () => language.Habits is { } habits ? HabitsSection(habits, ink) : null);
        Section(WritingStyleSection.Voice, L10n.RevealStepVoice(), () => language.Cards.Length > 0 ? CardsSection(language.Cards, ink) : null);
        Section(WritingStyleSection.Phrases, L10n.RevealPhrases(), () => language.Phrases is { } phrases ? PhrasesSection(phrases, ink) : null);
        return panel;
    }

    private bool RefreshDetail(DetailParts parts, WritingStyleDetail detail)
    {
        parts.Record = detail;
        if (_styleOutcomes.Fresh(_model.WritingStylePage()?.Outcome) is { Style: var style, Result: WritingStyleResult.Refused refused } && style == parts.Style)
        {
            parts.Said.Text = WritingStyleRules.Say(refused.Reason, StyleWords());
            parts.Said.Visibility = Visibility.Visible;
        }
        DrawDetailControls(parts);
        return true;
    }

    // Rename, the notes and Forget from the record; the name box held with Rename. What is typed
    // stays.
    private void DrawDetailControls(DetailParts parts)
    {
        var detail = parts.Record;
        ControlComponents.HoldFields(detail.Rename, parts.Name);
        var rename = WritingStyleRules.NameSave(detail.Rename, MailboxModel.StyleNameRefusal(parts.Name.Text));
        if (rename != parts.RenameDrawn)
        {
            parts.RenameDrawn = rename;
            parts.RenameSlot.Child = ControlComponents.Button(L10n.WritingStyleRename(), rename, "WritingStyleRename", () =>
            {
                parts.Said.Visibility = Visibility.Collapsed;
                _model.ChangeWritingStyle(new WritingStyleIntent.Rename(parts.Style, parts.Name.Text));
            });
        }
        if (detail.NotesControl != parts.NotesDrawn)
        {
            parts.NotesDrawn = detail.NotesControl;
            parts.NotesSlot.Child = ControlComponents.TextBox(L10n.WritingStyleNotes(), detail.Notes, detail.NotesControl, "WritingStyleNotes", notes =>
                _model.ChangeWritingStyle(new WritingStyleIntent.SetNotes(parts.Style, notes)), wraps: true);
        }
        if (detail.Forget != parts.ForgetDrawn)
        {
            parts.ForgetDrawn = detail.Forget;
            parts.ForgetSlot.Child = ControlComponents.Button(L10n.WritingStyleForget(), detail.Forget, "WritingStyleForget", () => parts.Forget.Show());
        }
        parts.Forget.Enabled = detail.Forget.Enabled;
    }

    // The detail on screen: what it was drawn from, the name as typed, and the slots its
    // record-drawn controls sit in.
    private sealed class DetailParts
    {
        internal DetailParts(WritingStyleDetail detail)
        {
            Style = detail.Id;
            Record = detail;
            var message = detail.DraftingAccounts.Length > 0
                ? $"{L10n.WritingStyleForgetMessage()} {L10n.WritingStyleForgetAccounts(string.Join(", ", detail.DraftingAccounts))}"
                : L10n.WritingStyleForgetMessage();
            Forget = new ConfirmPanel(
                L10n.WritingStyleForgetTitle(detail.Name), message, L10n.WritingStyleForget(), "WritingStyleForget", yes =>
                {
                    if (yes)
                    {
                        Forgotten?.Invoke();
                    }
                });
        }

        internal string Style { get; }

        internal WritingStyleDetail Record { get; set; }

        internal TextBox Name { get; } = new() { MinWidth = 280 };

        /// <summary>The name as the record gave it when the detail opened.</summary>
        internal void Fill() => Name.Text = Record.Name;

        internal TextBlock Said { get; } = new() { TextWrapping = TextWrapping.Wrap, Visibility = Visibility.Collapsed };

        internal Border RenameSlot { get; } = new();

        internal Border NotesSlot { get; } = new();

        internal Border ForgetSlot { get; } = new() { HorizontalAlignment = HorizontalAlignment.Left };

        internal ConfirmPanel Forget { get; }

        internal Action? Forgotten { get; set; }

        internal ControlState? RenameDrawn { get; set; }

        internal ControlState? NotesDrawn { get; set; }

        internal ControlState? ForgetDrawn { get; set; }
    }
}
