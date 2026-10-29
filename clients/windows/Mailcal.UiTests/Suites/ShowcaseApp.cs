// The launches on the in-memory showcase seed, which CI runs on every pull request: no server, two
// accounts, deterministic, English unless a fixture says otherwise. The showcase engine performs no
// real mail action, so anything that has to survive a round trip is a harness suite's.

using Allodia.Mailcal.UiTests.Driver;
using Xunit;

namespace Allodia.Mailcal.UiTests.Suites;

/// <summary>The app on the showcase seed, in English, on the list, light.</summary>
public class ShowcaseApp : UiApp
{
    private protected override App Launch() => App.LaunchOnShowcase(Env);
}

/// <summary>
/// The showcase pinned to the appearance this desktop is not in, which is what makes an appearance
/// assertion mean something on any host: one that asked for dark on a dark desktop would pass with
/// the setting reaching nothing.
/// </summary>
public class OppositeAppearanceApp : ShowcaseApp
{
    internal static readonly bool DesktopDark = Desktop.Dark();

    protected override IReadOnlyDictionary<string, string> Env { get; } =
        new Dictionary<string, string> { ["MAILCAL_APPEARANCE"] = DesktopDark ? "light" : "dark" };
}

[CollectionDefinition(Name)]
public sealed class ShowcaseCollection : ICollectionFixture<ShowcaseApp>
{
    public const string Name = "showcase";
}

/// <summary>The app on an emptied first-run store, past the welcome question, on the account-setup
/// form: what a person sees once. It needs no server.</summary>
public sealed class FirstRunApp : HarnessApp
{
    protected override bool FirstRun => true;
}

/// <summary>
/// A case that needs no server: on the showcase, or on a first run. It needs a Debug build and
/// nothing else, so it skips, saying why, only without one: CI builds one before it runs these.
/// </summary>
public sealed class OfflineFactAttribute : FactAttribute
{
    public OfflineFactAttribute()
    {
        if (!App.HasBuild())
        {
            Skip = "no Debug build to launch (clients/windows/build-and-run.ps1 -NoRun)";
        }
    }
}
