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

[CollectionDefinition(Name)]
public sealed class ShowcaseCollection : ICollectionFixture<ShowcaseApp>
{
    public const string Name = "showcase";
}

/// <summary>
/// A case on the showcase. It needs a Debug build and nothing else, so it skips, saying why, only
/// without one: CI builds one before it runs these.
/// </summary>
public sealed class ShowcaseFactAttribute : FactAttribute
{
    public ShowcaseFactAttribute()
    {
        if (!App.HasBuild())
        {
            Skip = "no Debug build to launch (clients/windows/build-and-run.ps1 -NoRun)";
        }
    }
}
