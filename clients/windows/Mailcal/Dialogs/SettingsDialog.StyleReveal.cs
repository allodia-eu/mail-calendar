// Settings → Writing style → one style (docs/ai.md, "Learning" step 5), in six steps: what was
// read, a typical reply, greetings and sign-offs, tone and approach, phrases, then the style's name,
// the person's own notes, which every draft reads and which win over the description, and
// forgetting it. Steps two to five are about one language at a time, with a control above them to
// choose it when there are two or more. The same frame opens after a learning run and from a row in
// the library.
//
// The steps and the arithmetic behind the pictures are StyleReveal, where Mailcal.Tests pins them;
// the pages are SettingsDialog.RevealPages.cs and SettingsDialog.RevealCards.cs. The
// Try-it-on-a-recent-message offer is not drawn here: Settings is a dialog over the mailbox, and
// opening a reply to the account's latest Inbox message from inside it would mean closing it first.

using Allodia.Mailcal.Services;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Automation;
using Microsoft.UI.Xaml.Controls;
using uniffi.mailcal_bindings;

namespace Allodia.Mailcal.Dialogs;

public sealed partial class SettingsDialog
{
    // The reveal on screen: its page, its language and what was typed, which a rebuild of the
    // category keeps.
    private RevealSheet? _reveal;

    private UIElement BuildStyleReveal(WritingStyleDetail detail)
    {
        if (_reveal is not { } sheet || sheet.StyleId != detail.Row.Id)
        {
            sheet = new RevealSheet(detail.Row.Id, detail.Row.Name, detail.Notes, detail.Languages.Length);
            _reveal = sheet;
        }
        var ink = new RevealInk(_brushes);
        var frame = new WizardFrame(
            sheet.Pager,
            PanelHeight,
            allowsJump: true,
            (host, index) => RevealPage(detail, sheet, ink, StyleReveal.Steps[index], host));
        frame.SetCancel(L10n.ActionCancel(), () => Apply(CloseStyle));
        if (StyleReveal.ChoosesLanguage(detail.Languages.Length))
        {
            frame.SetTop(LanguageChoice(detail, sheet, frame));
        }
        frame.Moved = () =>
        {
            frame.ShowTop(sheet.ShowsLanguageChoice);
            if (sheet.Pager.IsLast)
            {
                frame.SetAction(L10n.RevealSave(), sheet.CanSave, accent: true, () => SaveStyle(detail, sheet));
            }
        };
        frame.Show();
        return frame.Root;
    }

    private UIElement RevealPage(
        WritingStyleDetail detail, RevealSheet sheet, RevealInk ink, RevealStep step, WizardFrame frame)
    {
        if (step == RevealStep.Read)
        {
            return ReadPage(detail, ink);
        }
        if (step == RevealStep.Name)
        {
            return NamePage(detail, sheet, frame);
        }
        // A style that covers no language has nothing to show on the pages about one.
        if (sheet.Language >= detail.Languages.Length)
        {
            return WizardFrame.Page(StepTitle(step));
        }
        var style = detail.Languages[sheet.Language];
        return step switch
        {
            RevealStep.Letter => LetterPage(style, ink),
            RevealStep.Habits => HabitsPage(style, ink),
            RevealStep.Voice => VoicePage(style, ink),
            _ => PhrasesPage(style, ink),
        };
    }

    private static string StepTitle(RevealStep step) => step switch
    {
        RevealStep.Read => L10n.RevealTitle(),
        RevealStep.Letter => L10n.RevealStepLetter(),
        RevealStep.Habits => L10n.RevealStepHabits(),
        RevealStep.Voice => L10n.RevealStepVoice(),
        RevealStep.Phrases => L10n.RevealPhrases(),
        _ => L10n.RevealStepName(),
    };

    // The language the four pages in the middle show, one segment per language by its own name.
    // Choosing one redraws the page on screen and keeps the step.
    private static SelectorBar LanguageChoice(WritingStyleDetail detail, RevealSheet sheet, WizardFrame frame)
    {
        var bar = new SelectorBar();
        AutomationProperties.SetName(bar, L10n.A11yRevealLanguage());
        for (var index = 0; index < detail.Languages.Length; index++)
        {
            var item = new SelectorBarItem { Text = WritingStyleText.LanguageName(detail.Languages[index].Language) };
            bar.Items.Add(item);
            if (index == sheet.Language)
            {
                bar.SelectedItem = item;
            }
        }
        bar.SelectionChanged += (_, _) =>
        {
            var chosen = bar.SelectedItem is { } selected ? bar.Items.IndexOf(selected) : -1;
            if (chosen >= 0 && chosen != sheet.Language)
            {
                sheet.Language = chosen;
                frame.Show();
            }
        };
        return bar;
    }

    // Step 6: the name, the notes, and forgetting the style. Save is the frame's last button.
    private UIElement NamePage(WritingStyleDetail detail, RevealSheet sheet, WizardFrame frame)
    {
        var page = WizardFrame.Page(L10n.RevealStepName());
        var name = new TextBox
        {
            Header = L10n.WritingStyleNameLabel(),
            Text = sheet.Name,
            MinWidth = 240,
            HorizontalAlignment = HorizontalAlignment.Left,
        };
        name.TextChanged += (_, _) =>
        {
            sheet.Name = name.Text;
            frame.EnablePrimary(sheet.CanSave);
        };
        page.Children.Add(name);
        var notes = new TextBox
        {
            Header = L10n.WritingStyleNotes(),
            Description = L10n.WritingStyleNotesHint(),
            Text = sheet.Notes,
            AcceptsReturn = true,
            TextWrapping = TextWrapping.Wrap,
            MinHeight = 92,
            MaxHeight = 160,
        };
        notes.TextChanged += (_, _) => sheet.Notes = notes.Text;
        page.Children.Add(notes);
        page.Children.Add(ForgetStyle(detail.Row));
        return page;
    }

    private void SaveStyle(WritingStyleDetail detail, RevealSheet sheet)
    {
        if (!sheet.CanSave)
        {
            return;
        }
        if (sheet.Rename(detail.Row.Name) is { } name)
        {
            _model.RenameWritingStyle(detail.Row.Id, name);
        }
        if (sheet.NewNotes(detail.Notes) is { } notes)
        {
            _model.UpdateWritingStyleNotes(detail.Row.Id, notes);
        }
        Apply(CloseStyle);
    }

    // Forget, confirmed in place, the way deleting a signature is: a nested ContentDialog is not
    // allowed, and redrawing the screen for the question would drop an unsaved name or note.
    private UIElement ForgetStyle(WritingStyleRow style)
    {
        var stack = new StackPanel { Spacing = 6, Margin = new Thickness(0, 10, 0, 0) };
        var forget = new Button { Content = L10n.WritingStyleForget() };
        var question = new StackPanel { Spacing = 6 };
        question.Children.Add(Heading(L10n.WritingStyleForgetTitle(style.Name)));
        question.Children.Add(Description(L10n.WritingStyleForgetMessage()));
        var buttons = new StackPanel { Orientation = Orientation.Horizontal, Spacing = 8 };
        var yes = new Button { Content = L10n.WritingStyleForget() };
        yes.Click += (_, _) => Apply(() =>
        {
            _model.DeleteWritingStyle(style.Id);
            CloseStyle();
        });
        var no = new Button { Content = L10n.ActionCancel() };
        buttons.Children.Add(yes);
        buttons.Children.Add(no);
        question.Children.Add(buttons);
        var confirm = new Border
        {
            Background = _brushes.Of(ThemePalette.LayerFill),
            CornerRadius = new CornerRadius(6),
            Padding = new Thickness(10),
            Child = question,
            Visibility = Visibility.Collapsed,
        };
        forget.Click += (_, _) =>
        {
            forget.Visibility = Visibility.Collapsed;
            confirm.Visibility = Visibility.Visible;
        };
        no.Click += (_, _) =>
        {
            confirm.Visibility = Visibility.Collapsed;
            forget.Visibility = Visibility.Visible;
        };
        stack.Children.Add(forget);
        stack.Children.Add(confirm);
        return stack;
    }

    private void CloseStyle()
    {
        _openStyle = null;
        _reveal = null;
        _styleScreen = StyleScreen.Library;
    }
}
