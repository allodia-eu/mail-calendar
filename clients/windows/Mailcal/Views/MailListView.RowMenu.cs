// The menu a right-click raises over the message list, drawn as the core answers it
// (docs/row-actions.md): one row's own menu, a conversation's included, a message inside an
// expanded conversation's own, or the menu over the selection that row belongs to. This file draws
// it and runs what is picked; which items there are, which respond and which message each is about
// are the core's.
//
// Built in code as the menu opens, because the answer depends on the snapshot and the selection at
// that moment, and neither is a property of the row the template is bound to.

using Allodia.Mailcal.Dialogs;
using Allodia.Mailcal.Services;
using Allodia.Mailcal.ViewModels;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Automation;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Media;
using uniffi.mailcal_bindings;

namespace Allodia.Mailcal.Views;

public sealed partial class MailListView
{
    /// <summary>
    /// What a menu was raised over: the rows the core is asked about, and the list rows they are.
    /// </summary>
    /// <param name="Rows">The rows as the core names them.</param>
    /// <param name="Listed">The list rows a batch over them acts on; empty for a message inside a
    /// conversation, whose every item names that one message.</param>
    /// <param name="IsSelection">Whether they are the selection, which a batch then runs through.</param>
    private sealed record MenuSubject(SelectedRow[] Rows, IReadOnlyList<MailRow> Listed, bool IsSelection);

    /// <summary>
    /// The rows a menu on <paramref name="target"/> is about: the whole selection when the row is
    /// in it, and that row alone when it is not (docs/list-selection.md, rule 12). A message inside
    /// a conversation is not a row of the list's selection, so its menu is always its own.
    /// </summary>
    private MenuSubject? SubjectOf(object? target)
    {
        if (Model is not { } model)
        {
            return null;
        }
        switch ((target as FrameworkElement)?.DataContext)
        {
            case MailRow row when model.SelectionCovers(row):
                return new MenuSubject(SelectionActions.Rows(model.SelectedRows), model.SelectedRows, IsSelection: true);
            case MailRow row:
                return new MenuSubject(SelectionActions.Rows([row]), [row], IsSelection: false);
            case ThreadMessageItem message:
                return new MenuSubject([new SelectedRow.Message(message.Account, message.Key)], [], IsSelection: false);
            default:
                return null;
        }
    }

    private void OnRowMenuOpening(object sender, object e)
    {
        if (sender is not MenuFlyout flyout)
        {
            return;
        }
        flyout.Items.Clear();
        if (Model is { } model && SubjectOf(flyout.Target) is { } subject)
        {
            var menu = model.RowMenu(subject.Rows);
            foreach (var section in menu.Sections)
            {
                if (flyout.Items.Count > 0)
                {
                    flyout.Items.Add(new MenuFlyoutSeparator());
                }
                foreach (var item in section.Items)
                {
                    flyout.Items.Add(MenuItem(item, subject));
                }
            }
        }
        // A row the snapshot no longer lists has no menu; an empty flyout would draw a sliver.
        if (flyout.Items.Count == 0)
        {
            DispatcherQueue.TryEnqueue(flyout.Hide);
        }
    }

    private MenuFlyoutItem MenuItem(RowMenuItem item, MenuSubject subject)
    {
        // A disabled item keeps its place (rule 6). Move to folder… also needs a folder to list,
        // which is this client's to know: the core decides only whether a move would take anything.
        var enabled = item.Enabled
            && (item.Action != RowAction.MoveToFolder || MoveToFolderPlan(subject.Rows) is not null);
        var icon = new FontIcon { Glyph = RowMenuPresentation.Glyph(item.Action) };
        if (RowMenuPresentation.Critical(item.Action)
            && Application.Current.Resources.TryGetValue("SystemFillColorCriticalBrush", out var critical))
        {
            icon.Foreground = (Brush)critical;
        }
        var menuItem = new MenuFlyoutItem { Text = Label(item.Action), Icon = icon, IsEnabled = enabled };
        AutomationProperties.SetAutomationId(menuItem, $"RowMenu{item.Action}");
        menuItem.Click += async (_, _) => await PerformAsync(item, subject);
        return menuItem;
    }

    private static string Label(RowAction action) => action switch
    {
        RowAction.Open => L10n.ActionOpen(),
        RowAction.OpenInWindow => L10n.ActionOpenInWindow(),
        RowAction.Reply => L10n.ActionReply(),
        RowAction.ReplyAll => L10n.ActionReplyAll(),
        RowAction.Forward => L10n.ActionForward(),
        RowAction.MarkRead => L10n.ActionMarkRead(),
        RowAction.MarkUnread => L10n.ActionMarkUnread(),
        RowAction.Flag => L10n.ActionFlag(),
        RowAction.Unflag => L10n.ActionClearFlag(),
        RowAction.MarkAsSpam => L10n.ActionMarkAsSpam(),
        RowAction.MarkAsNotSpam => L10n.ActionMarkAsNotSpam(),
        RowAction.MoveToFolder => L10n.ActionMoveToFolder(),
        RowAction.Archive => L10n.ActionArchive(),
        RowAction.Delete => L10n.ActionMoveToTrash(),
        RowAction.PermanentlyDelete => L10n.ActionDeletePermanently(),
        _ => action.ToString(),
    };

    /// <summary>Runs <paramref name="item"/>: on its one message when it names one, otherwise over
    /// the rows the menu was raised on.</summary>
    private async Task PerformAsync(RowMenuItem item, MenuSubject subject)
    {
        if (item.Action == RowAction.MoveToFolder)
        {
            await MoveToFolderAsync(subject.Rows);
        }
        else if (item.Target is { } target)
        {
            await PerformAsync(item.Action, target, subject);
        }
        else if (RowMenuPresentation.Batch(item.Action) is { } batch)
        {
            await ActOnAsync(batch, subject);
        }
    }

    private async Task PerformAsync(RowAction action, MenuMessage message, MenuSubject subject)
    {
        if (Model is not { } model)
        {
            return;
        }
        var (account, key) = (message.Account, message.Key);
        switch (action)
        {
            case RowAction.Open:
                await OpenTargetAsync(message);
                break;
            case RowAction.OpenInWindow:
                if (HeaderOf(message) is { } header)
                {
                    OpenInWindow(header);
                }
                break;
            // The same composer as the reading pane's toolbar: starting a different draft replaces
            // whatever is already open, so the open one is left first, which keeps it in Drafts.
            case RowAction.Reply or RowAction.ReplyAll:
                await LeaveComposerForDraftAsync();
                App.Shell?.ComposeReply(account, key, action == RowAction.ReplyAll);
                break;
            case RowAction.Forward:
                await LeaveComposerForDraftAsync();
                App.Shell?.ComposeForward(account, key);
                break;
            case RowAction.MarkRead or RowAction.MarkUnread:
                model.MarkRead(account, key, action == RowAction.MarkRead);
                break;
            case RowAction.Flag or RowAction.Unflag:
                model.SetFlagged(account, key, action == RowAction.Flag);
                break;
            case RowAction.MarkAsSpam:
                model.MarkAsSpam(account, key);
                break;
            case RowAction.MarkAsNotSpam:
                model.MarkAsNotSpam(account, key);
                break;
            case RowAction.Archive or RowAction.Delete:
                Remove(action, message, subject);
                break;
            case RowAction.PermanentlyDelete:
                if (await ConfirmPermanentDeleteAsync(count: 1))
                {
                    model.PermanentlyDelete(account, key);
                }
                break;
        }
    }

    // A flat row's own message goes through the SAME deferred machine as a swipe: the row hides,
    // nothing is dispatched until the undo window closes (MailListView.Swipe.cs). The swipe gesture
    // answers to touch, pen and a precision touchpad but never to a mouse, so for a mouse-only user
    // this menu IS the feature, and the undo window is what makes it an equal path. A message inside
    // a conversation is not a row the list can hide, so it is moved at once.
    private void Remove(RowAction action, MenuMessage message, MenuSubject subject)
    {
        var kind = action == RowAction.Archive ? SwipeActionKind.Archive : SwipeActionKind.Delete;
        if (subject.Listed is [{ IsThread: false } row] && row.Account == message.Account && row.Key == message.Key)
        {
            PerformSwipe(row, kind);
        }
        else if (kind == SwipeActionKind.Archive)
        {
            Model?.Archive(message.Account, message.Key);
        }
        else
        {
            Model?.Delete(message.Account, message.Key);
        }
    }

    /// <summary>The batch over the menu's rows: through the bar's own path when they are the
    /// selection, otherwise for those rows alone.</summary>
    private async Task ActOnAsync(BulkAction action, MenuSubject subject)
    {
        if (Model is not { } model)
        {
            return;
        }
        if (action == BulkAction.PermanentlyDelete && !await ConfirmPermanentDeleteAsync(subject.Listed.Count))
        {
            return;
        }
        if (subject.IsSelection)
        {
            model.ActOnSelection(action);
        }
        else
        {
            model.ActOnRows(subject.Listed, action);
        }
    }

    // The one irreversible item on the menu, so it asks first whichever set it is about; over
    // several rows it counts them, the way the bar's own button does.
    private async Task<bool> ConfirmPermanentDeleteAsync(int count)
    {
        var result = await DialogHelper.ConfirmAsync(
            XamlRoot,
            L10n.DeletePermanentlyTitle(),
            count > 1 ? L10n.DeletePermanentlyMessageMany(count) : L10n.DeletePermanentlyMessage(),
            L10n.ActionDelete());
        return result == ContentDialogResult.Primary;
    }

    /// <summary>
    /// Opens the message an item names: a flat row as itself, a conversation's message by expanding
    /// its conversation onto it, as selecting the conversation would. A draft opens its composer.
    /// </summary>
    private async Task OpenTargetAsync(MenuMessage target)
    {
        if (Model is not { } model)
        {
            return;
        }
        await LeaveComposerForMessageAsync();
        if (Listed(target) is ({ IsThread: true } conversation, { } message))
        {
            if (!conversation.IsExpanded)
            {
                model.ToggleThread(conversation, openLatest: false);
            }
            await OpenThreadMessageAsync(message);
        }
        else if (Listed(target) is ({ } row, null))
        {
            await OpenOrResumeAsync(row);
        }
    }

    /// <summary>The reading header of the message an item names, which a window opens with.</summary>
    private OpenedMessage? HeaderOf(MenuMessage target) => Listed(target) switch
    {
        (_, { } message) => MailboxModel.ThreadMessageHeader(message),
        ({ } row, null) => MailboxModel.RowHeader(row),
        _ => null,
    };

    /// <summary>Where the list shows <paramref name="target"/>: its flat row, or its conversation
    /// and its place in it.</summary>
    private (MailRow? Row, ThreadMessageItem? Message) Listed(MenuMessage target)
    {
        foreach (var row in Model?.Rows ?? [])
        {
            if (row.Account != target.Account)
            {
                continue;
            }
            if (!row.IsThread && row.Key == target.Key)
            {
                return (row, null);
            }
            if (row.IsThread && row.Messages.FirstOrDefault(m => m.Key == target.Key) is { } message)
            {
                return (row, message);
            }
        }
        return (null, null);
    }
}
