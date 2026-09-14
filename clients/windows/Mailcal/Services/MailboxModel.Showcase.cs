// The showcase (screenshot) boot of MailboxModel, split out to keep each file under the 500-line
// limit: the two in-memory datasets a capture run brings up instead of connecting real accounts.
// Enabled by MAILCAL_SHOWCASE and never set in a shipped build, so no path here can reach a real
// mailbox, a credential store or the network.

using System.Threading.Tasks;
using uniffi.mailcal_bindings;

namespace Allodia.Mailcal.Services;

public sealed partial class MailboxModel
{
    /// <summary>
    /// Brings up the in-memory showcase (screenshot) dataset instead of connecting real accounts:
    /// two fictional accounts with a full mailbox, a threaded conversation, an attachment, and a
    /// calendar, all from bundled sample content. No network and no credential store, so nothing
    /// personal can appear in a screenshot. Enabled by <c>MAILCAL_SHOWCASE</c>; never in a shipped build.
    /// The sample content is seeded in the language the chrome renders in (<see cref="ShowcaseMode"/>),
    /// so each store listing gets a screenshot set that reads in one language throughout.
    /// </summary>
    /// <remarks>
    /// The add-account capture is the exception, and boots the account-less showcase instead. That
    /// screen is the one somebody sees before they have a mailbox, and what it offers there,
    /// the Allodia account above the address field, is put once and never again to somebody who
    /// already has one (docs/onboarding.md). Over the seeded two accounts the form is a plain
    /// address field: a true screenshot of a different screen, which is the kind of mistake that
    /// reaches a store looking perfectly fine.
    /// </remarks>
    private async Task ConnectShowcaseAsync()
    {
        _connecting = true;
        var deviceTz = MailcalBindingsMethods.DeviceTimeZone();
        var level = ResolveLogLevel();
        var firstRun = ShowcaseMode.Screen == ShowcaseScreen.AddAccount;
        var locale = ShowcaseMode.SeedLocale;
        Log.Info(firstRun
            ? $"MAILCAL_SHOWCASE set, bringing up the account-less {locale} showcase (the first-run screen)"
            : $"MAILCAL_SHOWCASE set, bringing up the in-memory {locale} showcase dataset (no real account)");
        try
        {
            var app = await Task.Run(() => firstRun
                ? MailcalApp.NewShowcaseFirstRun(_observer!, _logger!, level, deviceTz, locale)
                : MailcalApp.NewShowcase(_observer!, _logger!, level, deviceTz, locale));
            _ui.TryEnqueue(() =>
            {
                _connecting = false;
                _app = app;
                NeedsSetup = firstRun;
                SetupError = null;
                Reload();
                UpdateConnectivity(_app.Connectivity());
                ObserveSystemTimeZone();
                if (firstRun)
                {
                    return; // No account, so there is nothing to sync and no agenda to expand.
                }
                // Populate both the inbox and the agenda up front, so the mail list and the
                // calendar tab are each ready to screenshot without a real sync.
                _app.Dispatch(new Intent.RefreshMail());
                _app.Dispatch(new Intent.RefreshCalendar());
            });
        }
        catch (Exception ex)
        {
            Log.Error($"showcase bring-up failed: {CoreError.Describe(ex)}");
            _ui.TryEnqueue(() =>
            {
                _connecting = false;
                SetupError = L10n.StatusConnectFailed(CoreError.Describe(ex));
                NeedsSetup = true;
            });
        }
    }
}
