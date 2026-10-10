// The launch view of MailboxModel (docs/boot-sequence.md, "Hosting the constructor: the launch
// view"): what the window shows while ConnectAsync waits on the core's constructor. LaunchState
// decides; this file owns the delay and what the XAML binds.

using Microsoft.UI.Xaml;
using uniffi.mailcal_bindings;

namespace Allodia.Mailcal.Services;

public sealed partial class MailboxModel
{
    private readonly LaunchState _launch = new();
    private CancellationTokenSource? _launchDelay;

    /// <summary>
    /// Show the progress ring and "Opening your mailbox…": only once the constructor has outlasted
    /// the core's launch-status delay, so a normal open never raises and removes it.
    /// </summary>
    public Visibility LaunchVisibility => _launch.ShowsStatus ? Visibility.Visible : Visibility.Collapsed;

    // On the UI thread, as the constructor is dispatched. Until EndLaunch the window is a blank page.
    private void BeginLaunch()
    {
        _launch.Begin();
        _launchDelay = new CancellationTokenSource();
        _ = RevealLaunchStatusAsync(_launchDelay.Token);
        Raise(nameof(SetupVisibility));
        Raise(nameof(MainVisibility));
    }

    private async Task RevealLaunchStatusAsync(CancellationToken token)
    {
        try
        {
            await Task.Delay((int)MailcalBindingsMethods.LaunchStatusAfterMs(), token).ConfigureAwait(false);
        }
        catch (OperationCanceledException)
        {
            return;
        }
        _ui.TryEnqueue(() =>
        {
            _launch.DelayElapsed();
            Raise(nameof(LaunchVisibility));
        });
    }

    // On the UI thread, once the constructor has returned or thrown: the launch view gives way to
    // whichever surface the state now calls for.
    private void EndLaunch()
    {
        _launchDelay?.Cancel();
        _launchDelay?.Dispose();
        _launchDelay = null;
        _launch.Opened();
        Raise(nameof(LaunchVisibility));
        Raise(nameof(SetupVisibility));
        Raise(nameof(MainVisibility));
    }
}
