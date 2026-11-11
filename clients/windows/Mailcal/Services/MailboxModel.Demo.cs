// MAILCAL_DEV_ACCOUNT=demo: the core's in-memory demo account, which the test agent's conformance
// suite runs against on every desktop client (docs/test-agent.md). Debug builds only, and nothing
// here reaches a credential store, a stored account or the network.

#if DEBUG
using System;
using System.Threading.Tasks;
using uniffi.mailcal_bindings;

namespace Allodia.Mailcal.Services;

public sealed partial class MailboxModel
{
    /// <summary>Whether this launch asked for the demo account.</summary>
    private static bool WantsDemo =>
        DevBootRules.Resolve(Environment.GetEnvironmentVariable("MAILCAL_DEV_ACCOUNT")) == DevBoot.Demo;

    /// <summary>Brings up the core's in-memory demo account instead of the stored accounts.</summary>
    private async Task ConnectDemoAsync()
    {
        _connecting = true;
        var deviceTz = MailcalBindingsMethods.DeviceTimeZone();
        var level = ResolveLogLevel();
        Log.Info("MAILCAL_DEV_ACCOUNT=demo, bringing up the in-memory demo account (no real account)");
        try
        {
            var app = await Task.Run(() => MailcalApp.NewDemo(_observer!, _logger!, level, deviceTz));
            _ui.TryEnqueue(() =>
            {
                _connecting = false;
                _app = app;
                NeedsSetup = false;
                SetupError = null;
                Reload();
                PullNotices();
                ObserveSystemTimeZone();
                _app.Dispatch(new Intent.RefreshMail());
                _app.Dispatch(new Intent.RefreshCalendar());
            });
        }
        catch (Exception ex)
        {
            Log.Error($"demo bring-up failed: {CoreError.Describe(ex)}");
            _ui.TryEnqueue(() =>
            {
                _connecting = false;
                SetupError = CoreErrorCopy.SaidAsConnect(ex);
                NeedsSetup = true;
            });
        }
    }
}
#endif
