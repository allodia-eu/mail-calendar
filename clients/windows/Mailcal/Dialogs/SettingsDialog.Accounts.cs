// The Accounts category: one card per configured account, how far back it fetches mail, whether
// it receives by push (IMAP IDLE) or on a timer, and which folders it watches. Split into its own
// partial to keep SettingsDialog.cs clear of the 500-line limit, which the Signatures category took
// it to within one line of.
//
// State lives in Rust (the sync snapshot the model projects) and each change forwards to the core,
// which re-signals the settings surface. A strategy or folder change alters the card's LAYOUT, so
// those go through Apply() and re-render; a depth or interval change does not, so it sets directly.

using System;
using System.Linq;
using Allodia.Mailcal.ViewModels;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Automation;
using Microsoft.UI.Xaml.Controls;
using uniffi.mailcal_bindings;

namespace Allodia.Mailcal.Dialogs;

public sealed partial class SettingsDialog
{
    // --- Accounts: per-account fetch depth + sync behaviour --------------------

    private UIElement BuildAccounts()
    {
        var panel = new StackPanel { Spacing = 16 };
        // What the person's other devices have to say, above their own accounts: an offer becomes
        // one of them.
        if (BuildAllodiaSync() is { } sync)
        {
            panel.Children.Add(sync);
        }
        var settings = _model.GetSyncSettings();
        if (settings is null || settings.Accounts.Count == 0)
        {
            panel.Children.Add(new TextBlock
            {
                Text = L10n.SettingsAccountsEmpty(),
                TextWrapping = TextWrapping.Wrap,
                Opacity = 0.7,
            });
            return panel;
        }
        foreach (var account in settings.Accounts)
        {
            panel.Children.Add(BuildAccountCard(account, settings));
        }
        return panel;
    }

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

    private UIElement BuildAccountCard(AccountSyncChoice account, SyncSettingsChoices settings)
    {
        var panel = new StackPanel { Spacing = 8 };
        // The address first. This panel is a flat stack of cards with no box around either of
        // them, so the heading is the only thing that says which account the rows under it are
        // about, and with two accounts on screen, anything above it reads as belonging to the
        // one before. (Linux puts the same rows above its heading and is right to: an
        // AdwPreferencesGroup draws its own box, and nothing there is ambiguous.)
        panel.Children.Add(Heading(account.Email));
        // How this one is shared. First under the address, because it decides whether anything
        // below it is anybody else's business. Absent in a build with no Allodia sign-in, which
        // draws nothing rather than a dead control.
        if (_model.AccountsSyncMode.TryGetValue(account.AccountId, out var mode))
        {
            panel.Children.Add(BuildSyncModePicker(account.AccountId, mode));
        }

        // Fetch depth, how far back this account downloads mail (per-account). A depth change
        // doesn't change the card's layout, so it sets directly (no rebuild), like the interval.
        panel.Children.Add(new TextBlock { Text = L10n.SettingsSyncDepthHeading() });
        panel.Children.Add(Description(L10n.SettingsSyncDepthDescription()));
        var depthOptions = settings.SyncDepths.Select(m => new DepthOption(m, DepthLabel(m))).ToList();
        var depthBox = new ComboBox
        {
            MinWidth = 220,
            ItemsSource = depthOptions,
            SelectedItem = depthOptions.FirstOrDefault(o => o.Months == account.SyncDepthMonths),
        };
        depthBox.SelectionChanged += (_, _) =>
        {
            if (!_rebuilding && depthBox.SelectedItem is DepthOption option)
            {
                _model.SetAccountSyncDepthChoice(account.AccountId, option.Months);
            }
        };
        // The heading above is a sibling TextBlock, which carries no relation a screen reader
        // follows, so the picker needs its own name, the catalog's field label, as Android uses it.
        AutomationProperties.SetName(depthBox, L10n.SettingsSyncDepthLabel());
        panel.Children.Add(depthBox);

        // Message size, the largest message kept offline (per-account). Like the depth, it sets
        // directly: the choice changes no layout, so the card needs no rebuild.
        panel.Children.Add(new TextBlock { Text = L10n.SettingsMessageSizeHeading() });
        panel.Children.Add(Description(L10n.SettingsMessageSizeDescription()));
        var sizeOptions = settings.MessageSizeLimitsMb
            .Select(mb => new MessageSizeOption(mb, MessageSizeLabel(mb)))
            .ToList();
        var sizeBox = new ComboBox
        {
            MinWidth = 220,
            ItemsSource = sizeOptions,
            SelectedItem = sizeOptions.FirstOrDefault(o => o.Megabytes == account.MessageSizeLimitMb),
        };
        sizeBox.SelectionChanged += (_, _) =>
        {
            if (!_rebuilding && sizeBox.SelectedItem is MessageSizeOption option)
            {
                _model.SetAccountMessageSizeChoice(account.AccountId, option.Megabytes);
            }
        };
        AutomationProperties.SetName(sizeBox, L10n.SettingsMessageSizeLabel());
        panel.Children.Add(sizeBox);

        // Sync behaviour, push (IMAP IDLE, when supported) vs. interval polling.
        if (account.IdleSupported)
        {
            var group = $"strategy-{account.AccountId}";
            panel.Children.Add(Radio(
                L10n.SettingsSyncStrategyPush(), group, account.Strategy == SyncStrategyChoice.Push,
                () => _model.SetSyncStrategyChoice(account.AccountId, true), rebuild: true));
            panel.Children.Add(Radio(
                L10n.SettingsSyncStrategyPoll(), group, account.Strategy == SyncStrategyChoice.Poll,
                () => _model.SetSyncStrategyChoice(account.AccountId, false), rebuild: true));
        }
        else
        {
            panel.Children.Add(new TextBlock
            {
                Text = L10n.SettingsSyncIdleUnsupported(),
                TextWrapping = TextWrapping.Wrap,
            });
        }

        panel.Children.Add(account.Strategy == SyncStrategyChoice.Push
            ? PushFolders(account, settings.MaxPushFolders)
            : PollIntervals(account, settings.PollIntervals));
        return panel;
    }

    private UIElement PollIntervals(AccountSyncChoice account, IReadOnlyList<ushort> intervals)
    {
        var options = intervals
            .Select(minutes => new IntervalOption(minutes, L10n.SettingsSyncIntervalMinutes(minutes)))
            .ToList();
        var combo = new ComboBox
        {
            Header = L10n.SettingsSyncIntervalLabel(),
            ItemsSource = options,
            SelectedItem = options.FirstOrDefault(option => option.Minutes == account.PollIntervalMins),
        };
        // An interval change doesn't change the layout, so it sets directly (no rebuild).
        combo.SelectionChanged += (_, _) =>
        {
            if (!_rebuilding && combo.SelectedItem is IntervalOption option)
            {
                _model.SetPollIntervalChoice(account.AccountId, option.Minutes);
            }
        };
        return combo;
    }

    private UIElement PushFolders(AccountSyncChoice account, int maxFolders)
    {
        var panel = new StackPanel { Spacing = 4 };
        panel.Children.Add(new TextBlock { Text = L10n.SettingsSyncFoldersHeading() });
        panel.Children.Add(new TextBlock
        {
            Text = L10n.SettingsSyncFoldersNote(maxFolders),
            TextWrapping = TextWrapping.Wrap,
        });
        foreach (var folder in account.Folders)
        {
            var check = new CheckBox
            {
                Content = folder.Name,
                IsChecked = folder.Subscribed,
                // Unchecked folders are disabled once the account is at the cap.
                IsEnabled = folder.Subscribed || !account.AtPushLimit,
            };
            check.Checked += (_, _) => Apply(() => _model.SetPushFolderChoice(account.AccountId, folder.Key, true));
            check.Unchecked += (_, _) => Apply(() => _model.SetPushFolderChoice(account.AccountId, folder.Key, false));
            panel.Children.Add(check);
        }
        return panel;
    }

    // The label for a fetch-depth option: a month count, or "All time" for the 0 sentinel.
    private static string DepthLabel(ushort months) =>
        months == 0 ? L10n.SyncDepthAll() : L10n.SyncDepthMonths(months);

    // The label for a message-size option: a megabyte count, or "Any size" for the 0 sentinel.
    private static string MessageSizeLabel(ushort megabytes) =>
        megabytes == 0 ? L10n.MessageSizeUnlimited() : L10n.MessageSizeMegabytes(megabytes);

    // A message-size option; ToString is the localised label so the ComboBox shows it directly.
    private sealed record MessageSizeOption(ushort Megabytes, string Label)
    {
        public override string ToString() => Label;
    }

    // A fetch-depth option; ToString is the localised label so the ComboBox shows it directly.
    private sealed record DepthOption(ushort Months, string Label)
    {
        public override string ToString() => Label;
    }

    // One interval option; ToString is the localised label so the ComboBox shows it directly.
    private sealed record IntervalOption(ushort Minutes, string Label)
    {
        public override string ToString() => Label;
    }
}
