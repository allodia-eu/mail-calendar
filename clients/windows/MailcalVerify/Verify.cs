// Headless runtime proof of the Rust <-> C# UniFFI binding (the Windows twin of macOS
// verify.swift). No WinUI, no display, no credentials, no network: it drives the
// unidirectional loop against the in-memory DEMO provider, so it is deterministic and
// CI-able. It asserts the seeded demo rows cross the FFI into C# and that the threaded
// re-projection round-trips, the empirical proof that dispatch -> runtime -> app ->
// bridge -> observer -> snapshot works before any UI is built on top of it.

using uniffi.mailcal_bindings;

namespace Allodia.MailcalVerify;

/// <summary>
/// Forwards the Rust-driven <c>Observer</c> callback (invoked from a runtime thread) into
/// a plain delegate, so the synchronous gate below can wait on a semaphore for it.
/// </summary>
internal sealed class SignalObserver : Observer
{
    private readonly Action<Surface> _onChange;
    public SignalObserver(Action<Surface> onChange) => _onChange = onChange;
    public void SurfaceChanged(Surface @surface) => _onChange(@surface);
}

/// <summary>
/// Forwards the Rust core's log records to the console, so the gate also exercises the FFI
/// logging port (the real clients forward to their platform-native logger instead).
/// </summary>
internal sealed class ConsoleLogger : Logger
{
    public void Log(LogLevel @level, string @target, string @message) =>
        Console.WriteLine($"[{@level}] [{@target}] {@message}");
}

/// <summary>
/// The secure-store port, for a gate whose account is a plaintext fixture rather than a
/// Credential Manager entry. It writes to the console rather than doing nothing: a rotation
/// reaching here means the fixture has a live OAuth grant, which this gate does not expect.
/// </summary>
internal sealed class ConsoleCredentialStore : AccountCredentialStore
{
    public void Persist(string @accountId, string @configToml) =>
        Console.WriteLine($"[credential-store] the core stored a credential for {@accountId}; this gate keeps nothing");

    public void Delete(string @accountId) =>
        Console.WriteLine($"[credential-store] the core erased the credential for {@accountId}; this gate keeps nothing");
}

/// <summary>
/// Counts the Debug records that cross the FFI (silently, a debug flood would drown the CI
/// output), so the level-toggle gate can assert on suppression vs. delivery. The core logs
/// from runtime worker threads, hence the interlocked count.
/// </summary>
internal sealed class DebugCountingLogger : Logger
{
    private int _debugRecords;

    public int DebugRecords => Volatile.Read(ref _debugRecords);

    public void Log(LogLevel @level, string @target, string @message)
    {
        if (@level == LogLevel.Debug)
        {
            Interlocked.Increment(ref _debugRecords);
        }
    }
}

internal static class Verifier
{
    private static int Main()
    {
        // The observer releases the semaphore on each MailboxList change; each fire-and-forget
        // dispatch below then waits for that completion before reading the snapshot. We key on
        // MailboxList specifically because a refresh now also emits SyncProgress signals (the
        // download bar) before the list is ready, waiting on "any" surface would read too early.
        using var done = new SemaphoreSlim(0);
        var app = MailcalApp.NewDemo(
            new SignalObserver(surface =>
            {
                if (surface == Surface.MailboxList)
                {
                    done.Release();
                }
            }),
            new ConsoleLogger(),
            LogLevel.Info,
            "Etc/UTC");

        // Initially empty, observer silent.
        Console.WriteLine($"demo created; before refresh: {app.MailboxList().Rows.Length} rows");

        // The message-list grouping is now a persisted preference that defaults to Threaded (the
        // product default); the demo has no preferences file, so it boots Threaded. Pin Flat
        // explicitly so the flat-row assertions below are independent of that default, the
        // SetViewMode(Threaded) step further down still proves the toggle collapses the reply.
        app.Dispatch(new Intent.SetViewMode(ViewMode.Flat));
        if (!done.Wait(TimeSpan.FromSeconds(5)))
        {
            Console.WriteLine("FAIL: setting the flat view did not re-project within 5s");
            return 1;
        }

        // Dispatch is fire-and-forget; the observer signals when the Rust work completes.
        app.Dispatch(new Intent.RefreshMail());
        if (!done.Wait(TimeSpan.FromSeconds(10)))
        {
            Console.WriteLine("FAIL: the observer did not fire within 10s");
            return 1;
        }
        var refreshed = app.MailboxList();
        var flat = refreshed.Rows.Length;
        var total = refreshed.Total;
        Console.WriteLine($"after RefreshMail (flat): {flat} rows, total={total}");

        // The pagination window crosses the FFI: ShowMore re-projects and (the demo holds far
        // under one page) still returns every row. Proves the new intent round-trips.
        app.Dispatch(new Intent.ShowMore());
        if (!done.Wait(TimeSpan.FromSeconds(5)))
        {
            Console.WriteLine("FAIL: ShowMore did not re-project within 5s");
            return 1;
        }
        var afterMore = app.MailboxList().Rows.Length;
        Console.WriteLine($"after ShowMore: {afterMore} rows");

        // Toggle to threaded and confirm the re-projection crosses the FFI (the demo's
        // reply collapses into its thread, so conversations <= messages).
        app.Dispatch(new Intent.SetViewMode(ViewMode.Threaded));
        if (!done.Wait(TimeSpan.FromSeconds(5)))
        {
            Console.WriteLine("FAIL: the threaded re-projection did not arrive within 5s");
            return 1;
        }
        var threaded = app.MailboxList().Rows.Length;
        Console.WriteLine($"threaded: {threaded} conversations");

        // The engine-sourced time-zone list crosses the FFI as a string[] and carries the
        // secondary IANA cities a host OS zone set collapses away (e.g. Europe/Amsterdam).
        var zones = MailcalBindingsMethods.AvailableTimeZones();
        var hasAmsterdam = zones.Contains("Europe/Amsterdam");
        Console.WriteLine($"available zones: {zones.Length}, Europe/Amsterdam present: {hasAmsterdam}");

        // The demo provider seeds exactly four messages (one a reply); the whole loop ran
        // iff they crossed into C# as four flat rows (with total == 4, the window unwound by
        // ShowMore) that collapse to fewer threads.
        var demoOk = flat == 4 && total == 4UL && afterMore == 4 && threaded > 0 && threaded <= flat
            && zones.Length > 100 && hasAmsterdam;
        Console.WriteLine(demoOk ? "PASS: the Rust <-> C# binding round-trips" : "FAIL: unexpected result");

        // The composing/swipe settings the Windows composer and settings dialog now read (see
        // VerifySendAndSwipeSettings). Uses the same demo app, whose preferences are in-memory.
        var settingsOk = VerifySendAndSwipeSettings(app);

        // The per-account-outage contract, exercised over the real FFI (see VerifyOutage).
        var outageOk = VerifyOutage();

        // The runtime log-level toggle behind Settings → Diagnostics (see VerifyLogLevelToggle).
        var logLevelOk = VerifyLogLevelToggle();

        return demoOk && settingsOk && outageOk && logLevelOk ? 0 : 2;
    }

    /// <summary>
    /// Proves the runtime log-level toggle (the Settings → Diagnostics debug switch) round-trips
    /// over the FFI: at the Info ceiling no Debug record crosses (the gate lives in the core's
    /// log macros, so a suppressed record never reaches the Logger), after SetLogLevel(Debug)
    /// the core's own debug records (e.g. the threaded projection's "thread completion" timing)
    /// do reach it, and the ceiling restores to Info. The ceiling is process-wide
    /// (log::set_max_level), so restoring it is part of the gate.
    /// </summary>
    private static bool VerifyLogLevelToggle()
    {
        using var done = new SemaphoreSlim(0);
        var counter = new DebugCountingLogger();
        var app = MailcalApp.NewDemo(
            new SignalObserver(surface =>
            {
                if (surface == Surface.MailboxList)
                {
                    done.Release();
                }
            }),
            counter,
            LogLevel.Info,
            "Etc/UTC");

        // At the Info ceiling: a refresh + both view-mode re-projections (the demo boots
        // Threaded, and the threaded projection logs at debug) must deliver no Debug record.
        var settled = Step(app, done, new Intent.RefreshMail(), "RefreshMail (info)")
            && Step(app, done, new Intent.SetViewMode(ViewMode.Flat), "flat re-projection (info)");
        if (!settled)
        {
            return false;
        }
        var suppressed = counter.DebugRecords == 0;

        // Raise the ceiling and drive the same work again: debug records must now cross.
        app.SetLogLevel(LogLevel.Debug);
        settled = Step(app, done, new Intent.RefreshMail(), "RefreshMail (debug)")
            && Step(app, done, new Intent.SetViewMode(ViewMode.Threaded), "threaded re-projection (debug)");
        var delivered = counter.DebugRecords > 0;
        // Restore the process-wide ceiling regardless, so this gate leaks nothing to later runs.
        app.SetLogLevel(LogLevel.Info);
        if (!settled)
        {
            return false;
        }

        var ok = suppressed && delivered;
        Console.WriteLine($"log level: suppressed-at-info={suppressed} delivered-at-debug={delivered} "
            + $"(debug records: {counter.DebugRecords})");
        Console.WriteLine(ok
            ? "PASS: the runtime log-level toggle gates and delivers debug records"
            : "FAIL: the log-level toggle did not gate/deliver as expected");
        return ok;
    }

    // Dispatches one intent and waits for its MailboxList re-projection (dispatch is
    // fire-and-forget; the observer signals when the Rust work completes).
    private static bool Step(MailcalApp app, SemaphoreSlim done, Intent intent, string label)
    {
        app.Dispatch(intent);
        if (done.Wait(TimeSpan.FromSeconds(10)))
        {
            return true;
        }
        Console.WriteLine($"FAIL: log-level gate: {label} did not re-project within 10s");
        return false;
    }

    /// <summary>
    /// Proves the two app-level preferences the composer's From dropdown and the settings dialog
    /// depend on cross the FFI and round-trip: the default send account (unset until chosen, and
    /// clearable) and the per-direction swipe actions (both Delete before the setting is touched,
    /// and independently settable). Windows itself has no swipe gesture, but it consumes the same
    /// generated binding, so this catches a generator or signature regression on the CI gate.
    /// The demo app persists nothing, so this mutates no developer preferences file.
    /// </summary>
    private static bool VerifySendAndSwipeSettings(MailcalApp app)
    {
        // Unset until the user picks one, the core then falls back to the first configured account.
        var initiallyUnset = app.DefaultSendAccount() is null;
        app.SetDefaultSendAccount("acct-1");
        var stored = app.DefaultSendAccount() == "acct-1";
        // Clearing restores "the first configured account" rather than dropping the send.
        app.SetDefaultSendAccount(null);
        var cleared = app.DefaultSendAccount() is null;

        // Both directions default to Delete: the behaviour before the setting existed.
        var swipe = app.SwipeSettings();
        var defaulted = swipe.Left == SwipeActionKind.Delete && swipe.Right == SwipeActionKind.Delete;
        // The two directions are configured independently, setting one leaves the other alone.
        app.SetSwipeAction(SwipeDirection.Left, SwipeActionKind.Archive);
        swipe = app.SwipeSettings();
        var independent = swipe.Left == SwipeActionKind.Archive && swipe.Right == SwipeActionKind.Delete;

        // Quoting: an indented quote with no per-message picker, until the user says otherwise.
        var quoting = app.QuoteSettings();
        var quoteDefaulted = quoting.Style == QuoteStyleKind.Indented && !quoting.PerMessage;
        // The style and the per-message opt-in are independent, setting one leaves the other alone.
        app.SetQuoteStyle(QuoteStyleKind.LineAndHeader);
        app.SetQuoteStylePerMessage(true);
        quoting = app.QuoteSettings();
        var quoteStored = quoting.Style == QuoteStyleKind.LineAndHeader && quoting.PerMessage;
        app.SetQuoteStylePerMessage(false);
        quoting = app.QuoteSettings();
        var quoteIndependent = quoting.Style == QuoteStyleKind.LineAndHeader && !quoting.PerMessage;

        var ok = initiallyUnset && stored && cleared && defaulted && independent
            && quoteDefaulted && quoteStored && quoteIndependent;
        Console.WriteLine($"send/swipe settings: unset={initiallyUnset} stored={stored} "
            + $"cleared={cleared} defaulted={defaulted} independent={independent}");
        Console.WriteLine($"quote settings: defaulted={quoteDefaulted} stored={quoteStored} "
            + $"independent={quoteIndependent}");
        Console.WriteLine(ok
            ? "PASS: the default-send-account, swipe-action and quote settings round-trip"
            : "FAIL: a composing/swipe/quote setting did not round-trip");
        return ok;
    }

    /// <summary>
    /// Proves the per-account-outage contract end to end over the real FFI: a stored account whose
    /// IMAP server is unreachable stays a first-class account, kept as a disconnected placeholder,
    /// badged unreachable, its connect error available for the "details" link, instead of
    /// vanishing from the switcher. The outage is simulated by pointing IMAP at a closed loopback
    /// port (connection refused, instant and deterministic), so this needs no real server, no
    /// credentials, and no server-level hosts-file override, the manual setup this test replaces.
    /// </summary>
    private static bool VerifyOutage()
    {
        var dir = Path.Combine(Path.GetTempPath(), "mailcalverify-outage-" + Guid.NewGuid().ToString("N"));
        Directory.CreateDirectory(dir);
        try
        {
            // A valid config whose IMAP endpoint refuses the connection (nothing listens on
            // 127.0.0.1:1), the connect fails fast with WSAECONNREFUSED, the outage we model.
            var configToml = MailcalBindingsMethods.AccountConfigToml(new AccountSetup(
                ImapHost: "127.0.0.1:1",
                Username: "outage@example.com",
                Password: "unused",
                SmtpHost: null,
                CaldavBaseUrl: null));

            // NewAccounts returns as soon as the account is prepared as a placeholder, boot paints
            // cached mail first and dials each server in the background, so the outaged account is
            // kept (not returned as an error), and the background dial badges it a moment later.
            // The gate compiles the generated bindings directly, not the WinUI app, so it cannot
            // reach `DeviceFacts`, and does not need to. Analytics is off in any build with no
            // relay baked in, and unconsented in every build, so these values are inert. They are
            // here to exercise the FFI record, which is the gate's whole job.
            var device = new DeviceInfo(
                Platform: Platform.Windows,
                OsVersion: "0",
                DeviceClass: DeviceClass.Unknown,
                AppVersion: "0.0.0",
                Locale: "en");
            var app = MailcalApp.NewAccounts(
                new SignalObserver(_ => { }),
                new ConsoleLogger(),
                LogLevel.Info,
                new[] { configToml },
                dir,
                "Etc/UTC",
                device,
                new ConsoleCredentialStore());

            // 1. The account still lists in the switcher (the whole point, it must not vanish).
            //    This holds immediately: the account is kept as a placeholder before any dial.
            var accounts = app.MailboxList().Accounts;
            var kept = accounts.Length == 1;
            var id = kept ? accounts[0].Id : string.Empty;

            // 2. It is badged unreachable, a per-account outage, the device itself being online,
            //    and 3. its technical detail is available (the "details" link) and names the account.
            //    Both land when the BACKGROUND dial fails, shortly after NewAccounts returns, so poll
            //    until they do (a refused connection fails fast; the timeout is generous for slow CI).
            var badged = false;
            string? detail = null;
            var hasDetail = false;
            var deadline = DateTime.UtcNow.AddSeconds(15);
            while (kept && DateTime.UtcNow < deadline)
            {
                var connectivity = app.Connectivity();
                badged = !connectivity.Offline && connectivity.UnreachableAccounts.Contains(id);
                detail = app.ConnectionDetail(id);
                hasDetail = detail is not null && detail.Contains("outage@example.com");
                if (badged && hasDetail) { break; }
                Thread.Sleep(200);
            }

            Console.WriteLine(
                $"outage: kept={kept} (accounts={accounts.Length}), badged={badged}, detail=\"{detail}\"");
            var ok = kept && badged && hasDetail;
            Console.WriteLine(ok
                ? "PASS: an unreachable account is kept, badged, and carries its error"
                : "FAIL: the per-account-outage contract did not hold");
            return ok;
        }
        finally
        {
            try { Directory.Delete(dir, recursive: true); } catch { /* best-effort temp cleanup */ }
        }
    }
}
