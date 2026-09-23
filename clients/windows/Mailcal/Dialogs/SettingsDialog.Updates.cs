// Settings → About → Updates: which mechanism keeps this copy current, and a way to ask it now.
//
// The app updates itself either way (docs/windows-channels.md), so this exists for the question a
// user actually has, which is "am I on the old one?". Answering it needs a button, because the
// honest answer changes between the moment the page opens and the moment they read it.
//
// Both channels answer in the same three outcomes and offer the same action; only who is asked
// differs, and that is settled once by `Updates.ChannelFor`. The Store is asked through
// `StoreUpdates` and a hosted copy through `AppIdentity`, because neither service can answer for
// the other's packages (docs/updates.md).
//
// A separate partial so SettingsDialog.About.cs stays a description of what About holds.

using System;
using System.Diagnostics;
using System.Threading.Tasks;
using Allodia.Mailcal.Services;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using WinRT.Interop;

namespace Allodia.Mailcal.Dialogs;

public sealed partial class SettingsDialog
{
    /// <summary>Built on the first Store check, and kept so an install acts on what was found.</summary>
    private StoreUpdates? storeUpdates;

    /// <summary>The Updates group, or <c>null</c> when nothing updates this build.</summary>
    private UIElement? BuildUpdates()
    {
        var channel = Updates.ChannelFor(AppIdentity.IsPackaged, AppIdentity.UpdateSource);
        if (channel == UpdateChannel.None)
        {
            // The unpackaged dev loop. There is no package to replace, and a button that always
            // failed would be worse than the absence of one.
            return null;
        }

        var hosted = channel == UpdateChannel.Hosted;
        var content = new StackPanel { Spacing = 8 };
        var status = new TextBlock { TextWrapping = TextWrapping.Wrap, Opacity = 0.7 };
        var check = new Button { Content = L10n.AboutUpdatesAction() };

        // Shown only once a check has said there is something to get.
        var get = new HyperlinkButton
        {
            Content = L10n.AboutUpdatesAvailableAction(),
            Visibility = Visibility.Collapsed,
        };
        get.Click += async (_, _) => await GetUpdateAsync(hosted);

        check.Click += async (_, _) =>
        {
            check.IsEnabled = false;
            get.Visibility = Visibility.Collapsed;
            status.Text = L10n.AboutUpdatesChecking();
            var outcome = await CheckAsync(hosted);
            status.Text = outcome switch
            {
                UpdateOutcome.Available => L10n.AboutUpdatesAvailable(),
                UpdateOutcome.UpToDate => L10n.AboutUpdatesCurrent(),
                _ => L10n.AboutUpdatesFailed(),
            };
            get.Visibility =
                outcome == UpdateOutcome.Available ? Visibility.Visible : Visibility.Collapsed;
            check.IsEnabled = true;
        };

        content.Children.Add(check);
        content.Children.Add(status);
        content.Children.Add(get);
        return Group(
            L10n.AboutUpdatesHeading(),
            hosted ? L10n.AboutUpdatesDescriptionHosted() : L10n.AboutUpdatesDescriptionStore(),
            content);
    }

    private async Task<UpdateOutcome> CheckAsync(bool hosted)
    {
        var channel = hosted ? UpdateChannel.Hosted : UpdateChannel.Store;
        Log.Info(Updates.CheckStartedLine(channel));
        var clock = Stopwatch.StartNew();
        var outcome = await AskAsync(hosted);
        var line = Updates.CheckFinishedLine(channel, outcome, clock.Elapsed);
        if (outcome == UpdateOutcome.Failed)
        {
            Log.Warn(line);
        }
        else
        {
            Log.Info(line);
        }
        return outcome;
    }

    private Task<UpdateOutcome> AskAsync(bool hosted)
    {
        if (hosted)
        {
            return AppIdentity.CheckForUpdateAsync();
        }
        // Built here rather than in BuildUpdates so a machine with no working Store costs an
        // answer on a button press rather than a Settings page that will not open.
        storeUpdates ??= new StoreUpdates(WindowNative.GetWindowHandle(App.MainWindow));
        return storeUpdates.CheckAsync();
    }

    private async Task GetUpdateAsync(bool hosted)
    {
        if (!hosted)
        {
            // The Store downloads and installs it, showing its own progress and consent.
            if (storeUpdates is { } store)
            {
                await store.InstallAsync();
            }
            return;
        }
        // The hosted channel has no equivalent: App Installer fetches on its own schedule, so the
        // route to it now is the `.appinstaller` itself. The browser saves it and App Installer
        // runs it, because the `ms-appinstaller:` protocol that made that one click is disabled on
        // consumer machines.
        if (AppIdentity.UpdateSource is { } source)
        {
            var opened = await Windows.System.Launcher.LaunchUriAsync(source);
            Log.Info(opened
                ? "update: opened the installer file in the browser"
                : "update: the installer file could not be opened");
        }
    }
}
