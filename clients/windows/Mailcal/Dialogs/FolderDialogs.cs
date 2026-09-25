// The two questions changing the folder tree asks (docs/folder-pane.md, "Changing the tree"): a
// folder's name, checked while it is typed (rule 25), and where Move to… sends a folder (rule 24).
// The delete question is a plain confirm, DialogHelper.ConfirmAsync, worded by FolderActions.
//
// Both open from a row's menu, never from inside another dialog: WinUI allows one ContentDialog at
// a time and DialogHelper drops a second, so a question raised from within one would vanish.

using Allodia.Mailcal.Services;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Automation;
using Microsoft.UI.Xaml.Controls;
using uniffi.mailcal_bindings;

namespace Allodia.Mailcal.Dialogs;

/// <summary>The folder-name and Move to… dialogs.</summary>
internal static class FolderDialogs
{
    private static FolderNameWords NameWords => new(
        L10n.FolderNameEmpty(),
        L10n.FolderNameSurrounded(),
        L10n.FolderNameControl(),
        L10n.FolderNameSeparator(),
        L10n.FolderNameTaken());

    /// <summary>
    /// Asks for a folder's name, seeded with <paramref name="initial"/>, and returns it, or
    /// <c>null</c> when the user cancelled. The confirm button is enabled only while
    /// <paramref name="check"/> answers <c>Valid</c>, and every other answer is said under the
    /// field, as it is typed.
    /// </summary>
    public static async Task<string?> AskNameAsync(
        XamlRoot root,
        string title,
        string confirm,
        string initial,
        Func<string, FolderNameCheck> check)
    {
        var box = new TextBox
        {
            Text = initial,
            Header = L10n.FolderNameLabel(),
            AcceptsReturn = false,
        };
        AutomationProperties.SetName(box, L10n.FolderNameLabel());
        AutomationProperties.SetAutomationId(box, "FolderNameField");
        var line = new TextBlock
        {
            TextWrapping = TextWrapping.Wrap,
            // The caption style and no colour of its own: a brush looked up here would be the
            // app's theme, not the one the dialog is drawn in (DialogHelper mirrors the window's).
            Style = (Style)Application.Current.Resources["CaptionTextBlockStyle"],
        };
        // Read out when it changes, so a screen reader hears why the button is disabled.
        AutomationProperties.SetLiveSetting(line, AutomationLiveSetting.Polite);
        var panel = new StackPanel { Spacing = 8, MinWidth = 320 };
        panel.Children.Add(box);
        panel.Children.Add(line);
        var dialog = new ContentDialog
        {
            XamlRoot = root,
            Title = title,
            Content = panel,
            PrimaryButtonText = confirm,
            CloseButtonText = L10n.ActionCancel(),
            DefaultButton = ContentDialogButton.Primary,
        };

        void Recheck()
        {
            var (canConfirm, words) = FolderActions.NameState(check(box.Text), NameWords);
            dialog.IsPrimaryButtonEnabled = canConfirm;
            line.Text = words;
            line.Visibility = words.Length == 0 ? Visibility.Collapsed : Visibility.Visible;
        }
        box.TextChanged += (_, _) => Recheck();
        // Focused with its text selected, so a rename is typed straight over the old name.
        box.Loaded += (_, _) =>
        {
            box.Focus(FocusState.Programmatic);
            box.SelectAll();
        };
        Recheck();

        var result = await DialogHelper.ShowAsync(dialog);
        return result == ContentDialogResult.Primary ? box.Text : null;
    }

    /// <summary>
    /// Asks where a folder goes and returns the chosen destination, or <c>null</c> when the user
    /// cancelled. Choosing a row answers at once: there is nothing else to decide, and a separate
    /// confirm button would need a verb the list already is.
    /// </summary>
    public static async Task<MoveTarget?> AskMoveTargetAsync(
        XamlRoot root,
        string title,
        IReadOnlyList<MoveTarget> targets)
    {
        MoveTarget? chosen = null;
        var list = new ListView
        {
            // Wrapped, and matched by reference: two destinations can read alike (a folder called
            // "Top level" at the top of the tree), and a match on the words would move it wrong.
            ItemsSource = targets.Select(target => new Choice(target)).ToList(),
            SelectionMode = ListViewSelectionMode.None,
            IsItemClickEnabled = true,
            MaxHeight = 360,
            MinWidth = 320,
        };
        AutomationProperties.SetName(list, title);
        AutomationProperties.SetAutomationId(list, "FolderMoveTargets");
        var dialog = new ContentDialog
        {
            XamlRoot = root,
            Title = title,
            Content = list,
            CloseButtonText = L10n.ActionCancel(),
            DefaultButton = ContentDialogButton.Close,
        };
        list.ItemClick += (_, args) =>
        {
            if (args.ClickedItem is Choice choice)
            {
                chosen = choice.Target;
                dialog.Hide();
            }
        };
        _ = await DialogHelper.ShowAsync(dialog);
        return chosen;
    }

    /// <summary>One row of Move to…: drawn as its label, which a list without a template reads
    /// from <c>ToString</c>, and a screen reader from the same.</summary>
    private sealed class Choice(MoveTarget target)
    {
        public MoveTarget Target { get; } = target;

        public override string ToString() => Target.Label;
    }
}
