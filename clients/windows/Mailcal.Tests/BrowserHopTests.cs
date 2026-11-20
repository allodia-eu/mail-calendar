// The browser half of a sign-in (docs/onboarding.md, "Nobody may be stranded on it"): an attempt
// retired by Cancel or by a newer request while its begin still dialled opens no browser, and the
// end of the wait on the browser is said once the redirect is in, so the form can take its Cancel
// away for the bounded exchange that follows.

using Allodia.Mailcal.Services;
using Xunit;

namespace Allodia.Mailcal.Tests;

public class BrowserHopTests
{
    [Fact]
    public async Task AnAttemptRetiredWhileItBeganOpensNoBrowser()
    {
        using var cancel = new CancellationTokenSource();
        var opened = 0;
        var arrived = 0;
        var hop = BrowserHop.RunAsync(
            async () =>
            {
                // Discovery is still dialling when the person presses Cancel.
                await cancel.CancelAsync();
                return "start";
            },
            _ =>
            {
                opened++;
                return Task.CompletedTask;
            },
            _ => Task.FromResult("callback"),
            () => arrived++,
            cancel.Token);

        await Assert.ThrowsAnyAsync<OperationCanceledException>(() => hop);
        Assert.Equal(0, opened);
        Assert.Equal(0, arrived);
    }

    [Fact]
    public async Task ALiveAttemptOpensTheBrowserOnceAndSaysWhenTheRedirectIsIn()
    {
        var steps = new List<string>();
        var (start, callback) = await BrowserHop.RunAsync(
            () => Task.FromResult("start"),
            begun =>
            {
                steps.Add($"open {begun}");
                return Task.CompletedTask;
            },
            _ =>
            {
                steps.Add("wait");
                return Task.FromResult("callback");
            },
            () => steps.Add("arrived"),
            CancellationToken.None);

        Assert.Equal("start", start);
        Assert.Equal("callback", callback);
        Assert.Equal(["open start", "wait", "arrived"], steps);
    }

    [Fact]
    public async Task AnAttemptRetiredInTheBrowserNeverSaysTheRedirectArrived()
    {
        using var cancel = new CancellationTokenSource();
        var arrived = 0;
        var hop = BrowserHop.RunAsync(
            () => Task.FromResult("start"),
            _ => Task.CompletedTask,
            token => Task.Delay(Timeout.Infinite, token).ContinueWith<string>(
                _ => throw new OperationCanceledException(token), TaskScheduler.Default),
            () => arrived++,
            cancel.Token);
        await cancel.CancelAsync();

        await Assert.ThrowsAnyAsync<OperationCanceledException>(() => hop);
        Assert.Equal(0, arrived);
    }
}
