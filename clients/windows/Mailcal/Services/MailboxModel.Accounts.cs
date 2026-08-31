// The account-lifecycle half of MailboxModel (split out to keep each file under the 500-line
// limit): bringing the engine up over every stored account, adding another at runtime, and
// the connect/add error handling. The Windows counterpart of macOS's MailcalModel connect /
// submitSetup. State stays in Rust; this file owns only the host-side orchestration of opening
// it and feeding the secure store.

using System.Linq;
using uniffi.mailcal_bindings;

namespace Allodia.Mailcal.Services;

public sealed partial class MailboxModel
{
    /// <summary>
    /// Wires the reactive loop and connects every stored account over one engine: credentials
    /// live in the Windows Credential Manager, never a plaintext file. On first run (nothing
    /// stored) the app comes up account-less and shows the setup form, which adds the first
    /// account via <see cref="SubmitSetup"/>.
    /// </summary>
    public void Start()
    {
        // The log is already open: Program.Main points it at the same AppPaths.Root before the app
        // object exists, so a crash during XAML init is written rather than lost.
        _observer = new SurfaceObserver(surface => _ui.TryEnqueue(() => Reload(surface)));
        // The shared bridge that routes every Rust-core log record into this same app.log.
        _logger = new CoreLogger();
        // Screenshot/demo mode: MAILCAL_SHOWCASE brings up an in-memory showcase mailbox + calendar
        // (two fictional accounts, seeded sample content) instead of connecting real accounts, so
        // store screenshots need no personal mail. It bypasses the credential store entirely and
        // is never set in a shipped build.
        if (ShowcaseMode.IsOn)
        {
            _ = ConnectShowcaseAsync();
            return;
        }
#if DEBUG
        // A debug launch may override the accounts to target the local Stalwart harness instead of
        // the stored (personal) accounts; both the engine store AND the credential store are then
        // isolated in a dev namespace, so a form-add during a harness run never touches real
        // accounts. Null storeSubdir ⇒ a normal stored-accounts launch. See ResolveStartupAccounts.
        var (configs, storeSubdir) = ResolveStartupAccounts();
#else
        var configs = CredentialStore.Configs();
        string? storeSubdir = null;
#endif
        // First run (no MAIL account stored) shows the full-screen form; set this before
        // connecting so a connect failure's own NeedsSetup is not overwritten, and so a successful
        // first-run connect (account-less engine) keeps the form up until the user adds an account.
        //
        // The Allodia grant lives in this same store under a reserved id, so the length of this
        // array is not the number of mail accounts. Somebody who signs in on the first-run screen
        // and quits before adding a mailbox would otherwise be met at the next launch by an empty
        // inbox and no way back to setup. The core routes that entry out before anything reads it
        // as a mailbox; this asks it the same question.
        NeedsSetup = configs.All(MailcalBindingsMethods.IsAllodiaAccountConfig);
        Log.Info(configs.Length == 0
            ? "no stored account, bringing up an account-less app for setup"
            : $"{configs.Length} stored account(s), connecting");
        _ = ConnectAsync(configs, storeSubdir);
    }

    /// <summary>
    /// Builds the config from the setup-form fields and adds it as a new account (the first, or
    /// another over the running app); on success it's stored in the Credential Manager. Blank
    /// optional fields are omitted; an invalid config or failed connect surfaces as
    /// <see cref="SetupError"/> and keeps the form up.
    /// </summary>
    // `internal`, not `public`: the method now takes the generated UniFFI `ConnectionSecurity`
    // enum, which the C# bindings emit as `internal` (like every generated type), so a `public`
    // signature is a CS0051 accessibility error. Its only caller is AccountSetupView's code-behind
    // in this same assembly, and it mirrors DetectAsync, which is `internal` for the same reason.
    internal void SubmitSetup(
        string imapHost,
        string username,
        string password,
        string smtpHost,
        string caldavUrl,
        ConnectionSecurity imapSecurity = ConnectionSecurity.ImplicitTls,
        ConnectionSecurity smtpSecurity = ConnectionSecurity.ImplicitTls)
    {
        // Ignore a second submit while a connect/add is already in flight (e.g. a double-click),
        // so we never build two engines or add one account twice.
        if (_connecting || IsSubmitting)
        {
            return;
        }
        var setup = new AccountSetup(
            ImapHost: imapHost,
            Username: username,
            Password: password,
            SmtpHost: string.IsNullOrWhiteSpace(smtpHost) ? null : smtpHost,
            CaldavBaseUrl: string.IsNullOrWhiteSpace(caldavUrl) ? null : caldavUrl,
            // The manual form passes the implicit-TLS defaults; the detected path passes what
            // detection found, so the engine dials implicit TLS or STARTTLS to match.
            ImapSecurity: imapSecurity,
            SmtpSecurity: smtpSecurity);
        // The app is built (account-less) before the form is shown, so this normally holds. It
        // fails only if the engine itself couldn't open at launch, surface that rather than
        // letting the Connect button silently do nothing.
        if (_app is null)
        {
            // The engine itself couldn't open at launch (ConnectAsync's own error is already
            // shown); restate it plainly rather than silently no-op. Mirrors macOS's wording.
            SetupError = "Could not open the app. Please relaunch.";
            return;
        }
        string configToml;
        try
        {
            configToml = MailcalBindingsMethods.AccountConfigToml(setup);
        }
        catch (Exception ex)
        {
            // An invalid config (or a Rust panic surfacing as PanicException) stays on the form
            // rather than crashing, like macOS.
            SetupError = CoreError.Describe(ex);
            return;
        }
        _ = AddAccountAsync(configToml);
    }

    /// <summary>
    /// Builds a JMAP config from the setup-form fields and adds it as a new account; on success it's
    /// stored in the Credential Manager. The server may be blank (the core derives it from the email
    /// domain) and the one secret is a password or an API token alike (they collapsed into a single
    /// field once the engine began negotiating the scheme). Mirrors <see cref="SubmitSetup"/>, same
    /// build-then-connect-then-persist discipline, but JMAP is HTTP Basic/bearer, so unlike
    /// Microsoft it needs no browser flow. An invalid config or failed connect surfaces as
    /// <see cref="SetupError"/> and keeps the form up.
    /// </summary>
    public void SubmitJmapSetup(string email, string serverUrl, string secret)
    {
        // Ignore a second submit while a connect/add is already in flight (e.g. a double-click),
        // so we never build two engines or add one account twice.
        if (_connecting || IsSubmitting)
        {
            return;
        }
        var setup = JmapSetupForm.Build(email, serverUrl, secret);
        // The app is built (account-less) before the form is shown, so this normally holds; it fails
        // only if the engine itself couldn't open at launch, surface that rather than no-op.
        if (_app is null)
        {
            SetupError = "Could not open the app. Please relaunch.";
            return;
        }
        string configToml;
        try
        {
            configToml = MailcalBindingsMethods.JmapAccountConfigToml(setup);
        }
        catch (Exception ex)
        {
            // An invalid config (e.g. a missing secret) or a Rust panic surfacing as PanicException
            // stays on the form rather than crashing, like macOS.
            SetupError = CoreError.Describe(ex);
            return;
        }
        _ = AddAccountAsync(configToml);
    }

    /// <summary>
    /// Starts the Microsoft 365 sign-in: opens the authorization URL in the user's browser (which
    /// reuses its logged-in Microsoft session) and captures the redirect on a loopback listener,
    /// then completes the flow (token exchange + connect) off the UI thread and stores the config.
    /// The account appears at once; its first sync runs in the background. A cancel/failure keeps
    /// the form up with <see cref="SetupError"/>. The Windows twin of macOS's signInWithMicrosoft.
    /// </summary>
    /// <remarks>
    /// Deliberately NOT guarded on <see cref="IsSubmitting"/>, see the twin remark on
    /// <see cref="SignInWithGoogle"/>: a request arriving while one is outstanding supersedes it,
    /// because a sign-in abandoned in the browser cannot be told apart from one still in progress.
    /// </remarks>
    public void SignInWithMicrosoft(string? loginHint = null)
    {
        if (_connecting)
        {
            return;
        }
        if (_app is null)
        {
            SetupError = "Could not open the app. Please relaunch.";
            return;
        }
        _ = _signIn.RunAsync(cancel => SignInWithMicrosoftAsync(loginHint, cancel));
    }

    /// <summary>
    /// Aborts a Microsoft sign-in that's waiting on the browser redirect (the user pressed Cancel).
    /// Safe to call when none is in flight. The awaiting flow unwinds cleanly and re-enables the form.
    /// </summary>
    public void CancelMicrosoftSignIn() => _signIn.Cancel();

    private async Task SignInWithMicrosoftAsync(string? loginHint, CancellationToken cancelToken)
    {
        // IsSigningIn drives the (enabled) Cancel; IsSubmitting keeps the setup form's own buttons
        // from firing twice (they bind to it) while the browser step is outstanding.
        IsSigningIn = true;
        IsSubmitting = true;
        SetupError = null;
        try
        {
            // Arm the redirect rendezvous before opening the browser: the browser returns to our
            // custom scheme (eu.allodia.mailcal://auth), delivered here as a protocol activation
            // routed by Program into ProtocolAuthCallback.
            using var callback = ProtocolAuthCallback.Expect(MicrosoftOAuthConfig.CallbackHost);
            var start = MailcalBindingsMethods.BeginMicrosoftLogin(
                MicrosoftOAuthConfig.Tenant, MicrosoftOAuthConfig.RedirectUri,
                // The address the user is connecting (from autodetection), so Microsoft targets that
                // account instead of offering a different signed-in one; null/blank ⇒ the picker.
                string.IsNullOrWhiteSpace(loginHint) ? null : loginHint);
            // Open the default browser (Edge, where the user is usually already signed in).
            await Windows.System.Launcher.LaunchUriAsync(new Uri(start.AuthorizationUrl));
            var callbackUrl = await callback.WaitAsync(cancelToken);
            // The token exchange + folder connect block, so run them off the UI thread.
            var row = await Task.Run(() => _app!.CompleteMicrosoftLogin(start.Pending, callbackUrl));
            SetupError = null;
            NeedsSetup = false;
            AddingAccount = false;
            Log.Info($"microsoft account added: {row.Email}");
            // This route never touches AddAccountAsync, so the pass is owed here: without it the
            // account stays on this device until the next launch, and its card in Settings draws
            // no sharing control at all (docs/settings.md, category 9).
            SyncAfterAccountChange();
            // Land directly in the newly connected account rather than the unified inbox, so the
            // user sees their mail arriving where they just signed in (the core owns selection;
            // this dispatches the select intent and the snapshot expands the account in the sidebar).
            SelectAccount(row.Id);
        }
        catch (OperationCanceledException)
        {
            // The user pressed Cancel while the browser step was outstanding, return to the form
            // quietly (no error banner); the disposed callback slot ignores any late redirect.
            Log.Info("microsoft sign-in cancelled by user");
        }
        catch (Exception ex)
        {
            Log.Error($"microsoft sign-in failed: {CoreError.Describe(ex)}");
            SetupError = L10n.StatusConnectFailed(CoreError.Describe(ex));
        }
        finally
        {
            IsSigningIn = false;
            IsSubmitting = false;
        }
    }

    /// <summary>
    /// Opens the engine over every stored account <paramref name="configs"/> and wires the
    /// reactive loop. NewAccounts blocks on each account's network connect, so it runs off the
    /// UI thread (a spinner would be nicer). An empty <paramref name="configs"/> brings up an
    /// account-less app for first-run setup; on failure it surfaces the error and shows the form.
    /// A non-null <paramref name="storeSubdir"/> isolates the engine store in that subdirectory
    /// (the dev accounts do this, so harness test data never mixes with real accounts).
    /// </summary>
    private async Task ConnectAsync(string[] configs, string? storeSubdir = null)
    {
        _connecting = true;
        // Device zone detected in shared Rust (region-aware: the real city, not the
        // Windows-zone primary), so first boot adopts e.g. Europe/Amsterdam, not Berlin.
        var deviceTz = MailcalBindingsMethods.DeviceTimeZone();
        var level = ResolveLogLevel();
        // The rotating log stays at the shared DataDir, unaffected by the store isolation.
        var storeDir = storeSubdir is null ? DataDir : System.IO.Path.Combine(DataDir, storeSubdir);
        System.IO.Directory.CreateDirectory(storeDir);
        Log.Info($"connecting {configs.Length} account(s) (device zone {deviceTz}, log level {level})");
        try
        {
            // Time the whole connect+open (the blocking IMAP logins + engine open); the core
            // logs its own per-account breakdown into the same file via the injected logger.
            var sw = System.Diagnostics.Stopwatch.StartNew();
            // Device facts are reported raw; the core coarsens them and sends nothing until the
            // user opts in (docs/analytics.md).
            // The Credential Manager sink is handed over HERE, not set on the returned app. This
            // constructor starts dialing before it returns, a real launch measured the first
            // OAuth refresh 6 ms later, and the setter this replaces ran inside the
            // TryEnqueue below, so a rotation arriving before the UI thread got a turn was
            // dropped (docs/provider-oauth.md rule 5).
            var app = await Task.Run(() => MailcalApp.NewAccounts(_observer!, _logger!, level, configs, storeDir, deviceTz, DeviceFacts.Current(), new CredentialStoreSink()));
            Log.Info($"NewAccounts (connect + engine open) returned in {sw.ElapsedMilliseconds}ms");
            // Resume on the UI thread explicitly (ObservableCollections + bindable state must
            // only be touched there), not relying on the await's captured context.
            _ui.TryEnqueue(() =>
            {
                _connecting = false;
                _app = app;
                // Where this device remembers what it has synced with the account service, and
                // then what the person's other devices have to say. Installed before anything can
                // ask for a pass; unlike the Credential Manager sink above it is not racing a
                // dial, because nothing syncs until somebody asks.
                InstallAllodiaSyncStore(app);
                SyncAfterAccountChange();
                // The agent (MCP) surface, the composer port and the named-pipe endpoint. Setting
                // the endpoint applies the persisted settings, so a user who had it on last session
                // is listening again from this point (MailboxModel.Mcp.cs).
                WireAgentAccess(_app);
                SetupError = null;
                Log.Info("connected");
                // The core can now answer the usage-statistics question, and `asked == false` is
                // what puts the welcome screen up. All three top-level surfaces depend on it, so
                // re-evaluate them now the answer exists.
                Raise(nameof(AnalyticsAsked));
                Raise(nameof(WelcomeVisibility));
                Raise(nameof(SetupVisibility));
                Raise(nameof(MainVisibility));
                // The retention signal, once per launch. A no-op until the user opts in.
                _app.ReportAppOpened();
                // A configured-but-failed CalDAV connect is non-fatal (mail still works), so it's
                // otherwise invisible, log it as the likely empty-calendar cause.
                var calendarError = app.CalendarConnectError();
                if (calendarError is not null)
                {
                    Log.Warn($"calendar (CalDAV) failed to connect: {calendarError}");
                }
                Reload();
                // Pull connectivity once now (a boot outage is seeded before any surface signal
                // fires), so a stored account whose server is unreachable shows its warning badge +
                // the connection banner immediately, not only after the next connectivity change.
                UpdateConnectivity(_app.Connectivity());
                ObserveSystemTimeZone();
                ObserveNetworkReachability();
                _app.Dispatch(new Intent.RefreshMail());
            });
        }
        catch (Exception ex)
        {
            // Any failure (the runtime can't start or the engine can't open, a single account's
            // connect failure is non-fatal and never reaches here) falls back to the setup form
            // rather than crashing, and leaves the faulted Task fully observed.
            Log.Error($"connect failed: {CoreError.Describe(ex)}");
            _ui.TryEnqueue(() =>
            {
                _connecting = false;
                SetupError = L10n.StatusConnectFailed(CoreError.Describe(ex));
                NeedsSetup = true;
            });
        }
    }

    /// <summary>
    /// Brings up the in-memory showcase (screenshot) dataset instead of connecting real accounts:
    /// two fictional accounts with a full mailbox, a threaded conversation, an attachment, and a
    /// calendar, all from bundled sample content. No network and no credential store, so nothing
    /// personal can appear in a screenshot. Enabled by <c>MAILCAL_SHOWCASE</c>; never in a shipped build.
    /// The sample content is seeded in the language the chrome renders in (<see cref="ShowcaseMode"/>),
    /// so each store listing gets a screenshot set that reads in one language throughout.
    /// </summary>
    private async Task ConnectShowcaseAsync()
    {
        _connecting = true;
        var deviceTz = MailcalBindingsMethods.DeviceTimeZone();
        var level = ResolveLogLevel();
        var locale = ShowcaseMode.SeedLocale;
        Log.Info($"MAILCAL_SHOWCASE set, bringing up the in-memory {locale} showcase dataset (no real account)");
        try
        {
            var app = await Task.Run(() => MailcalApp.NewShowcase(_observer!, _logger!, level, deviceTz, locale));
            _ui.TryEnqueue(() =>
            {
                _connecting = false;
                _app = app;
                NeedsSetup = false;
                SetupError = null;
                Reload();
                UpdateConnectivity(_app.Connectivity());
                ObserveSystemTimeZone();
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

    /// <summary>
    /// Connects the config as a new account over the running engine and, on success, stores it.
    /// AddAccount blocks on the IMAP login, so it runs off the UI thread. The core writes the
    /// config to the Credential Manager itself, only once it connects, so a bad config is never
    /// stored, and so the write no longer waits on this UI-thread hop. A failure keeps the form
    /// up.
    /// </summary>
    private async Task AddAccountAsync(string configToml)
    {
        IsSubmitting = true;
        try
        {
            var row = await Task.Run(() => _app!.AddAccount(configToml));
            // The account list changed, so the person's other devices should hear about it now
            // rather than at the next launch.
            SyncAfterAccountChange();
            _ui.TryEnqueue(() =>
            {
                IsSubmitting = false;
                // A configured-but-failed CalDAV connect is non-fatal (mail still works), so
                // it's otherwise invisible, surface it here as the likely empty-calendar cause.
                var calendarError = _app!.CalendarConnectError();
                if (calendarError is not null)
                {
                    Log.Warn($"calendar (CalDAV) failed to connect: {calendarError}");
                }
                SetupError = null;
                NeedsSetup = false;
                AddingAccount = false;
                Log.Info($"account added: {row.Email}");
                // The core synced the new account and refreshed the snapshot (the observer
                // reloads the sidebar + unified inbox); nudge a mail sync too.
                _app.Dispatch(new Intent.RefreshMail());
            });
        }
        catch (Exception ex)
        {
            // A bad config or failed login (or a Rust panic surfacing as PanicException) stays on
            // the form, first run keeps NeedsSetup, an add keeps AddingAccount, rather than
            // crashing, and leaves the faulted Task fully observed.
            Log.Error($"add account failed: {CoreError.Describe(ex)}");
            _ui.TryEnqueue(() =>
            {
                IsSubmitting = false;
                SetupError = L10n.StatusConnectFailed(CoreError.Describe(ex));
            });
        }
    }

    /// <summary>
    /// Removes account <paramref name="id"/>: drops it from the running core (which stops its
    /// background sync, takes it out of the switcher/list, and, if it was selected, falls back
    /// to the unified inbox, then rebuilds the snapshot so the sidebar updates via the observer)
    /// and deletes its stored credential so it doesn't return on the next launch.
    /// </summary>
    public void RemoveAccount(string id) => _ = RemoveAccountAsync(id);

    private async Task RemoveAccountAsync(string id)
    {
        if (_app is null)
        {
            return;
        }
        try
        {
            // Quick (no network), but run off the UI thread to match the other account
            // operations; the observer refreshes the sidebar + list as the snapshot rebuilds.
            // The core erases the stored credential too, through the port it wrote it through.
            await Task.Run(() => _app!.RemoveAccount(id));
            Log.Info($"account removed: {id}");
        }
        catch (Exception ex)
        {
            // The account IS gone from the app, what can still fail here is erasing its stored
            // credential, which would bring it back at the next launch. Say so rather than letting
            // it reappear unexplained.
            Log.Error($"remove account failed: {CoreError.Describe(ex)}");
        }
    }

    /// <summary>
    /// The core log verbosity to boot with, resolved by <see cref="DiagnosticsLog.ResolveLevel"/>:
    /// the <c>ALLODIA_LOG_LEVEL</c> environment variable (error|warn|info|debug|trace, the dev
    /// escape hatch) wins when set; otherwise the persisted Settings → Diagnostics choice
    /// (<see cref="LogLevelStore"/>, "debug" from the toggle); otherwise <see cref="LogLevel.Info"/>,
    /// which keeps the rotating file log (<see cref="Log"/>) useful over a long window.
    /// </summary>
    private static LogLevel ResolveLogLevel() => DiagnosticsLog.ResolveLevel(
        Environment.GetEnvironmentVariable("ALLODIA_LOG_LEVEL"), LogLevelStore.Read());
}
