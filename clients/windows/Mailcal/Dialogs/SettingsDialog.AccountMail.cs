// An account's page, its mail settings as the core's page record says them: how far back and how
// much it keeps, push or poll and the folders it watches, and, for a JMAP account whose pasted
// password or API token the server stopped accepting, the field that replaces it. Drawn only while
// the account is used for mail. And the sharing picker the page opens with.

using Allodia.Mailcal.Controls;
using Allodia.Mailcal.Services;
using Allodia.Mailcal.Views;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Automation;
using Microsoft.UI.Xaml.Controls;
using uniffi.mailcal_bindings;

namespace Allodia.Mailcal.Dialogs;

public sealed partial class SettingsDialog
{
    /// <summary>
    /// The three positions an account can be shared in, and what the selected one means.
    /// </summary>
    /// <remarks>
    /// A single choice rather than a switch and a button: the two questions underneath, is this
    /// account on my other devices, and does this device exchange changes about it, are not
    /// independent in any way somebody can act on, and splitting them produced a screen where
    /// turning the switch off changed nothing the person could see.
    ///
    /// <c>RadioButtons</c> is WinUI's own single-choice control; laid out in three columns it is
    /// the segmented equivalent its Apple, Android and Linux twins draw. One subtext, the selected
    /// position's: three at once is a paragraph nobody reads.
    /// </remarks>
    private UIElement BuildSyncModePicker(string accountId, AllodiaAccountSyncMode mode)
    {
        var panel = new StackPanel { Spacing = 4 };
        var modes = new[]
        {
            AllodiaAccountSyncMode.On,
            AllodiaAccountSyncMode.Paused,
            AllodiaAccountSyncMode.Off,
        };
        var picker = new RadioButtons
        {
            Header = L10n.SettingsAccountSyncHeading(),
            MaxColumns = 3,
        };
        AutomationProperties.SetAutomationId(picker, "AccountSyncMode");
        foreach (var option in modes)
        {
            picker.Items.Add(SyncModeLabel(option));
        }
        picker.SelectedIndex = Array.IndexOf(modes, mode);
        var hint = Description(SyncModeHint(mode));
        picker.SelectionChanged += async (_, _) =>
        {
            var picked = modes[picker.SelectedIndex < 0 ? 0 : picker.SelectedIndex];
            if (picked == mode)
            {
                return;
            }
            var failure = await _model.SetAllodiaAccountSyncModeAsync(accountId, picked);
            Apply(() => _allodiaSyncFailure = failure);
        };
        panel.Children.Add(picker);
        panel.Children.Add(hint);
        // A refused change is said under the control that asked for it: the section the list
        // draws it in is not on this page.
        if (_allodiaSyncFailure is not null)
        {
            panel.Children.Add(BuildAllodiaFailure());
        }
        return panel;
    }

    private static string SyncModeLabel(AllodiaAccountSyncMode mode) => mode switch
    {
        AllodiaAccountSyncMode.On => L10n.SettingsAccountSyncOn(),
        AllodiaAccountSyncMode.Paused => L10n.SettingsAccountSyncPaused(),
        _ => L10n.SettingsAccountSyncOff(),
    };

    private static string SyncModeHint(AllodiaAccountSyncMode mode) => mode switch
    {
        AllodiaAccountSyncMode.On => L10n.SettingsAccountSyncOnHint(),
        AllodiaAccountSyncMode.Paused => L10n.SettingsAccountSyncPausedHint(),
        _ => L10n.SettingsAccountSyncOffHint(),
    };

    /// <summary>The mail settings, while the account is used for mail, as the core's page record
    /// says them.</summary>
    private UIElement MailGroup(AccountEntry entry, AccountMailSettings mail)
    {
        var panel = new StackPanel { Spacing = 8 };
        var id = entry.Id;
        panel.Children.Add(Count(
            L10n.SettingsSyncDepthHeading(), L10n.SettingsSyncDepthDescription(), mail.Depth,
            months => months == 0 ? L10n.SyncDepthAll() : L10n.SyncDepthMonths(months), "AccountSyncDepth",
            months => new AccountPageIntent.SetSyncDepth(id, months)));
        panel.Children.Add(Count(
            L10n.SettingsMessageSizeHeading(), L10n.SettingsMessageSizeDescription(), mail.Size,
            mb => mb == 0 ? L10n.MessageSizeUnlimited() : L10n.MessageSizeMegabytes(mb), "AccountMessageSize",
            mb => new AccountPageIntent.SetMessageSizeLimit(id, mb)));

        // How mail arrives: as it comes (IMAP IDLE, where the server offers it) or on a timer.
        if (mail.Push is { } push)
        {
            panel.Children.Add(ControlComponents.ComboBox(
                L10n.SettingsSyncStrategyLabel(),
                [L10n.SettingsSyncStrategyPush(), L10n.SettingsSyncStrategyPoll()],
                push.On ? 0 : 1, push.Control, "AccountSyncStrategy",
                at => _model.ChangeAccount(new AccountPageIntent.SetPush(id, at == 0))));
        }
        else
        {
            panel.Children.Add(new TextBlock
            {
                Text = L10n.SettingsSyncIdleUnsupported(),
                TextWrapping = TextWrapping.Wrap,
            });
        }
        if (mail.Interval is { } interval)
        {
            panel.Children.Add(Count(
                L10n.SettingsSyncIntervalLabel(), null, interval,
                minutes => L10n.SettingsSyncIntervalMinutes(minutes), "AccountPollInterval",
                minutes => new AccountPageIntent.SetPollInterval(id, minutes)));
        }
        if (mail.Push?.On == true)
        {
            panel.Children.Add(WatchedFolders(id, mail));
        }
        return Group(L10n.NavMail(), L10n.SettingsSyncDescription(), panel);
    }

    /// <summary>A combo box over <paramref name="choice"/>'s counts, worded by
    /// <paramref name="label"/>, asking for the count picked.</summary>
    private UIElement Count(
        string header, string? note, AccountCountChoice choice, Func<ushort, string> label,
        string automationId, Func<ushort, AccountPageIntent> intent)
    {
        var panel = new StackPanel { Spacing = 4 };
        var box = ControlComponents.ComboBox(
            header, choice.Options.Select(label).ToList(), (int)choice.Selected, choice.Control, automationId,
            at => _model.ChangeAccount(intent(choice.Options[at])));
        panel.Children.Add(box);
        if (note is not null)
        {
            panel.Children.Add(Description(note));
        }
        return panel;
    }

    private UIElement WatchedFolders(string id, AccountMailSettings mail)
    {
        var panel = new StackPanel { Spacing = 4 };
        panel.Children.Add(new TextBlock { Text = L10n.SettingsSyncFoldersHeading() });
        panel.Children.Add(new TextBlock
        {
            Text = L10n.SettingsSyncFoldersNote(mail.WatchLimit),
            TextWrapping = TextWrapping.Wrap,
        });
        foreach (var folder in mail.Folders)
        {
            // The same word the folder pane uses (docs/folder-pane.md rule 13).
            var key = folder.Key;
            var toggle = new ControlToggle(
                FolderLabel.For(folder.Role, folder.Name), null, folder.On, folder.Control,
                $"AccountWatchedFolder_{key}",
                on => _model.ChangeAccount(new AccountPageIntent.SetWatchedFolder(id, key, on)));
            panel.Children.Add(toggle.Element);
        }
        return panel;
    }
}
