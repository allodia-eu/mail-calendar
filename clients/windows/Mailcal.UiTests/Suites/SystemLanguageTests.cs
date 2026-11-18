// A launch with no language chosen, which is every fresh install and everyone who picked "System"
// in Settings. The language reaches the app's own strings, WinUI's and the dates, and the launch
// applies it in App's constructor, inside Application.Start: a refusal there ends the process
// before the window exists, leaving a WER entry and a log that stops at the session marker. Every
// other launch in these suites stores English, so none of them reaches this path.

using Allodia.Mailcal.UiTests.Driver;
using Xunit;

namespace Allodia.Mailcal.UiTests.Suites;

[Trait("Dataset", "first-run")]
public sealed class SystemLanguageTests(SystemLanguageFirstRunApp firstRun) : IClassFixture<SystemLanguageFirstRunApp>
{
    [OfflineFact]
    public void AFirstRunThatFollowsTheSystemLanguageReachesTheSetupForm() =>
        firstRun.Case(nameof(AFirstRunThatFollowsTheSystemLanguageReachesTheSetupForm), app =>
        {
            // The launch itself is most of the assertion: it waits for the welcome question and
            // the form behind it, by id, so it holds in whatever language the host is in.
            Assert.True(app.Running, "the app is gone after a launch that follows the system language");
            Assert.NotNull(app.Agent.Find(Filter.ById("DetectEmail", "Edit") with { OnScreen = true }));
        });
}
