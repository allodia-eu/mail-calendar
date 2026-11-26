// Signing in with Nextcloud from setup (docs/nextcloud-sign-in.md): the questions whether a server
// offers it, the flight, and its completion, a new calendar and contacts account. An existing
// account's, and a separate server's, are the core's browser sign-in (MailboxModel.BrowserSignIn.cs).
//
// The flight runs under the shared SignInFlight's token, so a new browser sign-in of any kind
// supersedes it and every Cancel retires it, as with the provider sign-ins beside it. Grants and
// pending handles are secrets: memory only, never logged.

using uniffi.mailcal_bindings;

namespace Allodia.Mailcal.Services;

public sealed partial class MailboxModel
{
    /// <summary>Wakes every flight waiting on the browser when the window is active again.</summary>
    internal static NextcloudWake NextcloudWake { get; } = new();

    // A grant setup could not make an account of yet, held for the server it was granted at.
    private readonly NextcloudGrantHold _setupNextcloud = new();

    /// <summary>Whether the server at <paramref name="serverUrl"/> is Nextcloud. Blocking in the
    /// core (one request, at most three seconds), so off the UI thread; never throws, and anything
    /// that goes wrong reads as "no", which leaves the password.</summary>
    internal async Task<bool> OffersNextcloudSignInAsync(string serverUrl)
    {
        if (_app is not { } app)
        {
            return false;
        }
        try
        {
            return await Task.Run(() => app.OffersNextcloudSignIn(serverUrl));
        }
        catch (Exception ex)
        {
            Log.Warn($"asking whether a server is Nextcloud failed: {CoreError.Describe(ex)}");
            return false;
        }
    }

    /// <summary>Withdraws a grant that became no account. Returns at once; best effort.</summary>
    internal void AbandonNextcloudGrant(string? grant)
    {
        if (grant is null || _app is not { } app)
        {
            return;
        }
        try
        {
            app.AbandonNextcloudSignIn(grant);
            Log.Info("nextcloud sign-in: withdrew a grant that became no account");
        }
        catch (Exception ex)
        {
            Log.Warn($"withdrawing a Nextcloud grant failed: {CoreError.Describe(ex)}");
        }
    }

    /// <summary>
    /// Flies one sign-in for setup, under its <paramref name="cancel"/>: the core starts it with
    /// <paramref name="begin"/>, the browser opens, and the core is polled until the person
    /// finishes. On a grant the window comes to the front, since the browser cannot hand the person
    /// back.
    /// </summary>
    private async Task<NextcloudFlightEnd> FlyNextcloudAsync(
        MailcalApp app, Func<MailcalApp, NextcloudSignInStart> begin, CancellationToken cancel, Action? opened = null)
    {
        var end = await NextcloudFlight.FlyAsync(
            () => Task.Run(() => begin(app)),
            OpenNextcloudLoginAsync,
            pending => Task.Run(() => app.PollNextcloudSignIn(pending)),
            app.AbandonNextcloudSignIn,
            TimeSpan.FromMilliseconds(MailcalBindingsMethods.NextcloudPollIntervalMs()),
            NextcloudWake,
            cancel,
            () =>
            {
                Log.Info("nextcloud sign-in: waiting on the browser");
                opened?.Invoke();
            });
        Log.Info($"nextcloud sign-in ended: {NextcloudEndName(end)}");
        if (end is NextcloudFlightEnd.Granted)
        {
            App.Shell?.BringToForeground();
        }
        return end;
    }

    private static async Task<bool> OpenNextcloudLoginAsync(string loginUrl)
    {
        try
        {
            return await ExternalLauncher.Open(new Uri(loginUrl));
        }
        catch (Exception ex)
        {
            Log.Warn($"the browser did not open for a Nextcloud sign-in: {ex.GetType().Name}");
            return false;
        }
    }

    private static string NextcloudEndName(NextcloudFlightEnd end) => end switch
    {
        NextcloudFlightEnd.Granted => "granted",
        NextcloudFlightEnd.Expired => "expired",
        NextcloudFlightEnd.BrowserFailed => "the browser did not open",
        NextcloudFlightEnd.Failed failed => $"failed ({CoreError.Describe(failed.Error)})",
        NextcloudFlightEnd.NotStarted notStarted => notStarted.Answer switch
        {
            NextcloudAddressStart.Unsure => "not started: the server showed nothing either way",
            NextcloudAddressStart.Maintenance => "not started: the server is in maintenance",
            NextcloudAddressStart.NotNextcloud => "not started: the server is something else",
            _ => "not started: no server can be at that address",
        },
        _ => "cancelled",
    };

    /// <summary>What a flight that ended without a grant says; null for one that was retired,
    /// which says nothing. A server that would not start or answer is said as a failed sign-in,
    /// since its own words are for a developer.</summary>
    internal static string? NextcloudSaid(NextcloudFlightEnd end) => end switch
    {
        NextcloudFlightEnd.Expired => L10n.AccountSigninTimeout(),
        NextcloudFlightEnd.BrowserFailed => L10n.AccountSigninBrowserFailed(),
        NextcloudFlightEnd.Failed { Error: MailcalException.Connect } => L10n.SetupImapSigninFailed(),
        NextcloudFlightEnd.Failed failed => CoreErrorCopy.Said(failed.Error),
        // An address the core could not tell was Nextcloud is said on the pane, beside its offer.
        NextcloudFlightEnd.NotStarted notStarted => notStarted.Answer switch
        {
            NextcloudAddressStart.NotNextcloud other => L10n.SetupNextcloudNotFound(other.Host),
            NextcloudAddressStart.Maintenance maintenance => L10n.SetupNextcloudMaintenance(maintenance.Host),
            NextcloudAddressStart.NotAnAddress => L10n.SetupNextcloudAddressInvalid(),
            _ => null,
        },
        _ => null,
    };

    /// <summary>
    /// Setup's sign-in, from the found card, the manual calendar and contacts form or the Nextcloud
    /// pane: a grant held from the last press at that server goes straight to the core; otherwise
    /// the browser is asked for one, at an address the pane holds once the core has asked it. The
    /// account it makes is used for <paramref name="start"/>'s uses. <paramref name="waiting"/>
    /// hears what the form says while the sign-in is out. Answers how a flight ended without an
    /// account, and null when none flew or one made an account.
    /// </summary>
    internal async Task<NextcloudFlightEnd?> SetupNextcloudSignInAsync(NextcloudStart start, Action<NextcloudWait> waiting)
    {
        if (_connecting || _app is not { } app || IsSubmitting)
        {
            return null;
        }
        SetupError = null;
        if (_setupNextcloud.HeldFor(start.Server) is not { } grant)
        {
            AbandonNextcloudGrant(_setupNextcloud.Release());
            waiting(start.Begin == NextcloudBegin.Asked ? NextcloudWait.Checking : NextcloudWait.Browser);
            var end = await _signIn.RunAsync(async cancel =>
            {
                IsSigningIn = true;
                try
                {
                    return await FlySetupNextcloudAsync(app, start, cancel, () => waiting(NextcloudWait.Browser));
                }
                finally
                {
                    IsSigningIn = false;
                }
            });
            waiting(NextcloudWait.None);
            if (end is not NextcloudFlightEnd.Granted granted)
            {
                if (NextcloudSaid(end) is { } said)
                {
                    SetupError = said;
                }
                return end;
            }
            grant = granted.Grant;
        }
        await AddNextcloudAccountAsync(app, start, grant);
        return null;
    }

    private Task<NextcloudFlightEnd> FlySetupNextcloudAsync(
        MailcalApp app, NextcloudStart start, CancellationToken cancel, Action opened) => start.Begin switch
        {
            NextcloudBegin.Asked => AskNextcloudAsync(app, core => core.BeginNextcloudSignInAt(start.Server), cancel, opened),
            NextcloudBegin.Anyway => AskNextcloudAsync(app, core => core.BeginNextcloudSignInAnyway(start.Server), cancel, opened),
            _ => FlyNextcloudAsync(app, core => core.BeginNextcloudSignIn(start.Server), cancel, opened),
        };

    /// <summary>A sign-in at an address the person typed: flown only once <paramref name="ask"/>
    /// started one.</summary>
    private async Task<NextcloudFlightEnd> AskNextcloudAsync(
        MailcalApp app, Func<MailcalApp, NextcloudAddressStart> ask, CancellationToken cancel, Action opened)
    {
        var flown = false;
        var end = await SetupNextcloudAddress.AskThenFlyAsync(
            () => Task.Run(() => ask(app)),
            begun =>
            {
                flown = true;
                return FlyNextcloudAsync(app, _ => begun, cancel, opened);
            },
            cancel);
        if (!flown)
        {
            Log.Info($"nextcloud sign-in ended: {NextcloudEndName(end)}");
        }
        return end;
    }

    private async Task AddNextcloudAccountAsync(MailcalApp app, NextcloudStart start, string grant)
    {
        IsSubmitting = true;
        _setupNextcloud.Complete(start.Server, grant);
        try
        {
            var row = await Task.Run(() => app.AddNextcloudAccount(grant, start.Uses));
            _setupNextcloud.Completed(spent: true);
            IsSubmitting = false;
            Log.Info($"account added through a Nextcloud sign-in: {LogHandle(row.Id)}");
            AccountAdded(row.Id);
            app.Dispatch(new Intent.RefreshMail());
        }
        catch (Exception ex)
        {
            // Kept for the next press, which tries it again without the browser.
            _setupNextcloud.Completed(spent: false);
            Log.Error($"adding a Nextcloud account failed: {CoreError.Describe(ex)}");
            IsSubmitting = false;
            SetupError = CoreErrorCopy.SaidAsConnect(ex);
            // An account set up already is offered the grant, as a refused add's config is.
            if (ex is MailcalException.AlreadySetUp twice)
            {
                SetupExisting = new ExistingAccountOffer(twice.accountId, string.Empty, [], NextcloudGrant: grant);
            }
        }
    }

    /// <summary>Withdraws the grant setup held for another try: setup is being left.</summary>
    private void ReleaseSetupNextcloudGrant() => AbandonNextcloudGrant(_setupNextcloud.Release());
}
