// Settings → Advanced as the core decides it (AdvancedPage): AI assistant access first, its status
// line following the server while the category is open. What each status says is pinned in
// Mailcal.Tests (AdvancedPageRules); what this is for is the wiring: a switch that dispatches
// nothing, or a status line drawn once and never redrawn when the server binds.
//
// The harness, because a showcase run wires no assistant access at all: its mailbox is fiction, and
// binding the pipe would take it from the real app beside it. The pipe is the machine's, so another
// copy of the app may already serve it here: a bound server and one refused as another copy both say
// the switch reached the core, and either passes.

using Allodia.Mailcal.UiTests.Driver;
using Xunit;

namespace Allodia.Mailcal.UiTests.Suites;

[Collection(HarnessCollection.Name)]
[Trait("Dataset", "harness")]
public sealed class SettingsAdvancedTests(HarnessApp harness)
{
    private static Element Find(App app, SetupFlow settings, string id) =>
        app.Agent.Wait(new Filter { Id = id, Under = settings.OpenCategory("advanced") }, TimeSpan.FromSeconds(10))
        ?? throw new InvalidOperationException($"Settings → Advanced draws no #{id}");

    private static string Status(App app) =>
        app.Agent.Find(Filter.ById("McpStatus"))?.Name ?? throw new InvalidOperationException("the assistant access status line is gone");

    [HarnessFact]
    public void TheStatusFollowsTheSwitchWhileTheCategoryIsOpen()
    {
        harness.Case(nameof(TheStatusFollowsTheSwitchWhileTheCategoryIsOpen), app =>
        {
            var settings = new SetupFlow(app);
            var toggle = Find(app, settings, "McpEnabledToggle");
            // The store persists the switch, so the case starts it off rather than assuming so.
            app.Agent.SetToggle(toggle, false);
            Assert.True(
                App.Eventually(() => Status(app) == Catalog.English("settings_mcp_status_off"), TimeSpan.FromSeconds(10)),
                $"off, the status reads '{Status(app)}'");

            app.Agent.SetToggle(toggle, true);
            var running = Catalog.English("settings_mcp_status_running");
            var another = Catalog.English("settings_mcp_status_another_instance").Split('{')[0];
            Assert.True(
                App.Eventually(() => Status(app) is var said && (said == running || said.StartsWith(another, StringComparison.Ordinal)), TimeSpan.FromSeconds(10)),
                $"turned on, the status reads '{Status(app)}': the server never bound, or the line was not redrawn when it did");

            app.Agent.SetToggle(toggle, false);
            Assert.True(
                App.Eventually(() => Status(app) == Catalog.English("settings_mcp_status_off"), TimeSpan.FromSeconds(10)),
                $"turned off, the status reads '{Status(app)}'");
        }, app =>
        {
            // Leave the server off, as the suite found it.
            var settings = new SetupFlow(app);
            app.Agent.SetToggle(Find(app, settings, "McpEnabledToggle"), false);
            app.CloseDialogs();
        });
    }
}
