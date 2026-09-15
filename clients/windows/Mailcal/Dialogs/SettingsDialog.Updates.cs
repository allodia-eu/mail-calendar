// Settings → About → Updates: which mechanism keeps this copy current, and a way to ask it now.
//
// The app updates itself either way (docs/windows-channels.md), so this exists for the question a
// user actually has, which is "am I on the old one?". Answering it needs a button, because the
// honest answer changes between the moment the page opens and the moment they read it.
//
// A separate partial so SettingsDialog.About.cs stays a description of what About holds.

using System;
using Allodia.Mailcal.Services;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;

namespace Allodia.Mailcal.Dialogs;

public sealed partial class SettingsDialog
{
    // Windows' own "check for updates" page for installed apps. The Store owns what it installed,
    // so this is the only honest destination for that channel: an in-app check would query a
    // mechanism that is not the one holding this copy's updates.
    private const string StoreUpdatesUri = "ms-windows-store://downloadsandupdates";

    /// <summary>The Updates group, or <c>null</c> when nothing updates this build.</summary>
    private UIElement? BuildUpdates()
    {
        var channel = Updates.ChannelFor(AppIdentity.IsPackaged, AppIdentity.UpdateSource);
        return channel switch
        {
            // The unpackaged dev loop. There is no package to replace, and a button that always
            // failed would be worse than the absence of one.
            UpdateChannel.None => null,
            UpdateChannel.Store => StoreUpdates(),
            _ => HostedUpdates(),
        };
    }

    private UIElement StoreUpdates()
    {
        var content = new StackPanel { Spacing = 8 };
        var open = new Button { Content = L10n.AboutUpdatesActionStore() };
        open.Click += async (_, _) =>
            await Windows.System.Launcher.LaunchUriAsync(new Uri(StoreUpdatesUri));
        content.Children.Add(open);
        return Group(
            L10n.AboutUpdatesHeading(), L10n.AboutUpdatesDescriptionStore(), content);
    }

    private UIElement HostedUpdates()
    {
        var content = new StackPanel { Spacing = 8 };
        var status = new TextBlock { TextWrapping = TextWrapping.Wrap, Opacity = 0.7 };
        var check = new Button { Content = L10n.AboutUpdatesAction() };

        // Shown only once a check has said there is something to get. It opens the `.appinstaller`,
        // which the browser saves and App Installer then runs: the `ms-appinstaller:` protocol that
        // used to make this one click is disabled on consumer machines, so this is the route.
        var get = new HyperlinkButton
        {
            Content = L10n.AboutUpdatesAvailableAction(),
            Visibility = Visibility.Collapsed,
        };
        get.Click += async (_, _) =>
        {
            if (AppIdentity.UpdateSource is { } source)
            {
                await Windows.System.Launcher.LaunchUriAsync(source);
            }
        };

        check.Click += async (_, _) =>
        {
            check.IsEnabled = false;
            get.Visibility = Visibility.Collapsed;
            status.Text = L10n.AboutUpdatesChecking();
            var outcome = await AppIdentity.CheckForUpdateAsync();
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
            L10n.AboutUpdatesHeading(), L10n.AboutUpdatesDescriptionHosted(), content);
    }
}
