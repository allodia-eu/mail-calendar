// An account's page, its mail settings as the core's page record says them: how far back and how
// much it keeps, push or poll and the folders it watches, and, for a JMAP account whose pasted
// password or API token the server stopped accepting, the field that replaces it. Drawn only while
// the account is used for mail. And the sharing control the page opens with.

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
    /// The three positions an account can be shared in, what the selected one means, and why the
    /// last change did not take.
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
    private UIElement SharingGroup(string accountId, AccountSharing sharing)
    {
        var panel = new StackPanel { Spacing = 4 };
        var modes = new[]
        {
            AllodiaAccountSyncMode.On,
            AllodiaAccountSyncMode.Paused,
            AllodiaAccountSyncMode.Off,
        };
        panel.Children.Add(ControlComponents.RadioButtons(
            L10n.SettingsAccountSyncHeading(), [.. modes.Select(SyncModeLabel)],
            Array.IndexOf(modes, sharing.Mode), sharing.Control, "AccountSyncMode",
            at => _model.ChangeAccount(new AccountPageIntent.SetSharing(accountId, modes[at]))));
        panel.Children.Add(Description(SyncModeHint(sharing.Mode)));
        if (sharing.Failed is { } failed)
        {
            panel.Children.Add(SharingFailed(failed, "AccountSyncMode_Failed"));
        }
        return panel;
    }

    /// <summary>Why a sharing change did not take, under the control or the question that asked
    /// for it: the core's typed answer decides the sentence, never an error's text.</summary>
    private TextBlock SharingFailed(AllodiaSyncFailure failure, string automationId)
    {
        var line = Description(failure switch
        {
            AllodiaSyncFailure.NeedsReauth => L10n.SettingsAllodiaReauth(),
            AllodiaSyncFailure.SignedOut => L10n.SettingsAllodiaSignedOutElsewhere(),
            _ => L10n.SettingsAccountSyncFailed(),
        });
        line.Opacity = 1;
        line.Foreground = (Microsoft.UI.Xaml.Media.Brush)_brushes.Of(ThemePalette.Critical);
        AutomationProperties.SetAutomationId(line, automationId);
        return line;
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
