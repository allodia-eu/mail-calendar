// The launches on the local Stalwart harness, one per set of switches, each shared by the suites of
// its collection, and the attribute that skips a case when the harness or a Debug build is missing.

using Allodia.Mailcal.UiTests.Driver;
using Xunit;

namespace Allodia.Mailcal.UiTests.Suites;

/// <summary>The app on the harness store.</summary>
public class HarnessApp : UiApp
{
    /// <summary>Quits the app and launches it again on the first-run store as it was left, with this
    /// launch's switches: for a case about the dial a launch makes.</summary>
    internal void RelaunchOnFirstRunStore() => Relaunch(() => App.RelaunchOnFirstRunStore(Env));

    /// <summary>Quits the app and launches it again on the harness store as it was left: for a case
    /// about what a launch makes of what an earlier one stored.</summary>
    internal void RelaunchOnHarness() => Relaunch(() => App.LaunchOnHarness(Env));

    /// <summary>Whether this launch starts from an empty store, on the account-setup form.</summary>
    protected virtual bool FirstRun => false;

    private protected override App Launch() => FirstRun ? App.LaunchOnFirstRun(Env) : App.LaunchOnHarness(Env);
}

/// <summary>The harness app signed in as Alice and Bob at once (stalwart-multi): for a meeting
/// between them, whose invitation and answer both cross this app.</summary>
public sealed class TwoAccountHarnessApp : HarnessApp
{
    protected override IReadOnlyDictionary<string, string> Env { get; } =
        new Dictionary<string, string> { ["MAILCAL_DEV_ACCOUNT"] = "stalwart-multi" };
}

/// <summary>The harness app on alice's IMAP account (stalwart-imap), whose account pass re-reads every
/// folder, so a sync brings the folder a message was moved to up to date.</summary>
public sealed class ImapHarnessApp : HarnessApp
{
    protected override IReadOnlyDictionary<string, string> Env { get; } =
        new Dictionary<string, string> { ["MAILCAL_DEV_ACCOUNT"] = "stalwart-imap" };
}

/// <summary>The harness app on alice's IMAP account, its CalDAV half, told to pretend the calendar
/// server stored each RSVP and reported it could not pass it on (MAILCAL_FAKE_REPLY_DELIVERY, debug
/// builds only): the one server verdict no harness produces, everything below it real.</summary>
public sealed class FakeReplyHarnessApp : HarnessApp
{
    internal const string Status = "5.2";

    protected override IReadOnlyDictionary<string, string> Env { get; } = new Dictionary<string, string>
    {
        ["MAILCAL_DEV_ACCOUNT"] = "stalwart-imap",
        ["MAILCAL_FAKE_REPLY_DELIVERY"] = $"failed:{Status}",
    };
}

/// <summary>The harness app with alice's mailbox used for mail alone, beside alice's and bob's
/// calendar and contacts as accounts of their own (stalwart-linked): for linking one to another. It
/// connects only its canned accounts, so a link made is gone at the next launch.</summary>
public sealed class LinkedHarnessApp : HarnessApp
{
    protected override IReadOnlyDictionary<string, string> Env { get; } =
        new Dictionary<string, string> { ["MAILCAL_DEV_ACCOUNT"] = "stalwart-linked" };
}

/// <summary>The harness app on alice's IMAP account, whose IDLE delivers into the running app, for
/// the notifications it raises. A class fixture, one launch a suite: a click with the app quit ends
/// this launch, and the copy the click starts belongs to the case.</summary>
public sealed class NotificationHarnessApp : HarnessApp
{
    protected override IReadOnlyDictionary<string, string> Env { get; } = ImapEnv;

    internal static readonly IReadOnlyDictionary<string, string> ImapEnv =
        new Dictionary<string, string> { ["MAILCAL_DEV_ACCOUNT"] = "stalwart-imap" };
}

/// <summary>The harness app with its log at debug: for a rule the app states only in its log.</summary>
public sealed class DebugLogHarnessApp : HarnessApp
{
    protected override IReadOnlyDictionary<string, string> Env { get; } =
        new Dictionary<string, string> { ["ALLODIA_LOG_LEVEL"] = "debug" };
}

/// <summary>The harness app pinned to the dark appearance: for what a dark window draws that a light
/// one does not.</summary>
public sealed class DarkHarnessApp : HarnessApp
{
    protected override IReadOnlyDictionary<string, string> Env { get; } =
        new Dictionary<string, string> { ["MAILCAL_APPEARANCE"] = "dark" };
}

/// <summary>The app on an empty store beside the harness: for a suite that sets an account up
/// through the form, uses it against the server and removes it again. A class fixture, one launch a
/// suite, because what a first run asks is asked once a store: a second suite on the same store
/// would set its account up without the questions the first one answered.</summary>
public sealed class SetupHarnessApp : HarnessApp
{
    protected override bool FirstRun => true;
}

/// <summary>The app on an empty store beside the harness, told that looking an account's calendar and
/// contacts servers up again finds them at <see cref="Moved"/> (MAILCAL_FAKE_SERVER_CHANGE, debug
/// builds only): the one answer no loopback server gives, everything around it real.</summary>
public sealed class ServerChangeHarnessApp : HarnessApp
{
    /// <summary>The harness again, under a host name rather than its address: another site to the
    /// question, and the same server to the connect that allowing it makes.</summary>
    internal const string Moved = "http://localhost:28080";

    protected override bool FirstRun => true;

    protected override IReadOnlyDictionary<string, string> Env { get; } =
        new Dictionary<string, string> { ["MAILCAL_FAKE_SERVER_CHANGE"] = Moved };
}

[CollectionDefinition(Name)]
public sealed class HarnessCollection : ICollectionFixture<HarnessApp>
{
    public const string Name = "harness";
}

[CollectionDefinition(Name)]
public sealed class TwoAccountHarnessCollection : ICollectionFixture<TwoAccountHarnessApp>
{
    public const string Name = "harness, two accounts";
}

[CollectionDefinition(Name)]
public sealed class ImapHarnessCollection : ICollectionFixture<ImapHarnessApp>
{
    public const string Name = "harness, IMAP";
}

[CollectionDefinition(Name)]
public sealed class FakeReplyHarnessCollection : ICollectionFixture<FakeReplyHarnessApp>
{
    public const string Name = "harness, IMAP, replies reported undelivered";
}

[CollectionDefinition(Name)]
public sealed class LinkedHarnessCollection : ICollectionFixture<LinkedHarnessApp>
{
    public const string Name = "harness, linked accounts";
}

[CollectionDefinition(Name)]
public sealed class DebugLogHarnessCollection : ICollectionFixture<DebugLogHarnessApp>
{
    public const string Name = "harness, log at debug";
}

[CollectionDefinition(Name)]
public sealed class DarkHarnessCollection : ICollectionFixture<DarkHarnessApp>
{
    public const string Name = "harness, dark";
}

/// <summary>
/// A case that needs the harness and a Debug build. Skipped, and saying why, when either is
/// missing, unless MAILCAL_UITESTS_REQUIRE_HARNESS=1: then it runs and fails, so a host meant to
/// have both cannot pass by having neither.
/// </summary>
public sealed class HarnessFactAttribute : FactAttribute
{
    public HarnessFactAttribute()
    {
        if (Environment.GetEnvironmentVariable("MAILCAL_UITESTS_REQUIRE_HARNESS") == "1")
        {
            return;
        }
        if (!Harness.IsUp())
        {
            Skip = "the harness is not up (scripts/dev/harness.sh up)";
        }
        else if (!App.HasBuild())
        {
            Skip = "no Debug build to launch (clients/windows/build-and-run.ps1 -NoRun)";
        }
    }

    /// <summary>
    /// Whether the case asks about a surface only a build with the Allodia sign-in draws. Such a
    /// build is derived from the registration (clients/windows/core-features.ps1), so a host without
    /// one skips the case, saying so, whatever MAILCAL_UITESTS_REQUIRE_HARNESS says.
    /// </summary>
    public bool NeedsAllodia
    {
        get => _needsAllodia;
        set
        {
            _needsAllodia = value;
            if (value && Skip is null && !App.AllodiaRegistered())
            {
                Skip = "this build has no Allodia sign-in: MAILCAL_ALLODIA_CLIENT_ID is set in neither the environment nor .env";
            }
        }
    }

    private bool _needsAllodia;
}
