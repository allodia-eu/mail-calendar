// Settings → Writing style → one style (docs/ai.md, "The reveal"): what was learned, per language,
// in plain words; the style's name and the person's own notes, which every draft reads and which
// win over the description; and forgetting it. The same screen opens after a learning run and from
// a row in the library.
//
// Which fields are drawn, in which order, is StyleReveal, where Mailcal.Tests pins it. The
// Try-it-on-a-recent-message offer is not drawn here: Settings is a dialog over the mailbox, and
// opening a reply to the account's latest Inbox message from inside it would mean closing it first.

using Allodia.Mailcal.Services;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using uniffi.mailcal_bindings;

namespace Allodia.Mailcal.Dialogs;

public sealed partial class SettingsDialog
{
    private UIElement BuildStyleReveal(WritingStyleDetail detail)
    {
        var panel = new StackPanel { Spacing = 16 };
        panel.Children.Add(Heading(L10n.RevealTitle()));
        foreach (var language in detail.Languages)
        {
            panel.Children.Add(LanguageSection(language));
        }

        // A style with no name is a row nobody can tell apart in the pickers, so Save waits for one.
        var save = new Button
        {
            Content = L10n.RevealSave(),
            Style = (Style)Application.Current.Resources["AccentButtonStyle"],
            IsEnabled = !string.IsNullOrWhiteSpace(detail.Row.Name),
        };
        var name = new TextBox
        {
            Header = L10n.WritingStyleNameLabel(),
            Text = detail.Row.Name,
            MinWidth = 240,
            HorizontalAlignment = HorizontalAlignment.Left,
        };
        name.TextChanged += (_, _) => save.IsEnabled = !string.IsNullOrWhiteSpace(name.Text);
        panel.Children.Add(name);
        var notes = new TextBox
        {
            Header = L10n.WritingStyleNotes(),
            PlaceholderText = L10n.WritingStyleNotesHint(),
            Text = detail.Notes,
            AcceptsReturn = true,
            TextWrapping = TextWrapping.Wrap,
            MinHeight = 80,
        };
        panel.Children.Add(notes);

        var buttons = new StackPanel
        {
            Orientation = Orientation.Horizontal,
            Spacing = 8,
            HorizontalAlignment = HorizontalAlignment.Right,
        };
        var close = new Button { Content = L10n.ActionClose() };
        close.Click += (_, _) => Apply(CloseStyle);
        save.Click += (_, _) =>
        {
            var trimmed = name.Text.Trim();
            if (trimmed != detail.Row.Name)
            {
                _model.RenameWritingStyle(detail.Row.Id, trimmed);
            }
            if (notes.Text != detail.Notes)
            {
                _model.UpdateWritingStyleNotes(detail.Row.Id, notes.Text);
            }
            Apply(CloseStyle);
        };
        buttons.Children.Add(close);
        buttons.Children.Add(save);
        panel.Children.Add(buttons);
        panel.Children.Add(ForgetStyle(detail.Row));
        return panel;
    }

    // One language: its own name, then each field that says something.
    private UIElement LanguageSection(LanguageStyleRow language)
    {
        var section = new StackPanel { Spacing = 8 };
        section.Children.Add(Heading(WritingStyleText.LanguageName(language.Language)));
        foreach (var entry in StyleReveal.Sections(language, Culture))
        {
            if (entry.Field == RevealField.Length)
            {
                section.Children.Add(Line(L10n.RevealLength(entry.Words)));
                continue;
            }
            var field = new StackPanel { Spacing = 2 };
            field.Children.Add(Description(WritingStyleText.Label(entry.Field)));
            foreach (var line in entry.Lines)
            {
                field.Children.Add(Line(line));
            }
            section.Children.Add(field);
        }
        return section;
    }

    // Forget, confirmed in place, the way deleting a signature is: a nested ContentDialog is not
    // allowed, and redrawing the screen for the question would drop an unsaved name or note.
    private UIElement ForgetStyle(WritingStyleRow style)
    {
        var stack = new StackPanel { Spacing = 6 };
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
        _styleScreen = StyleScreen.Library;
    }
}
