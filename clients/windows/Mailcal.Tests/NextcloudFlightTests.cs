// The browser half of a Nextcloud sign-in (docs/nextcloud-sign-in.md rules 5 and 6): the poll, its
// wake when the window is active again, how each end is reported, a grant answered after the way
// back withdrawn rather than dropped, and the grant a surface holds for another try.

using Allodia.Mailcal.Services;
using uniffi.mailcal_bindings;
using Xunit;

namespace Allodia.Mailcal.Tests;

public class NextcloudFlightTests
{
    private static readonly NextcloudSignInStart Started = new("https://cloud.example.org/login/v2/flow", "pending");

    // A long interval, so a test that finishes did so because of what it drove, not a timer.
    private static readonly TimeSpan Long = TimeSpan.FromMinutes(5);

    private sealed class Server(params NextcloudSignInPoll[] answers)
    {
        private readonly Queue<NextcloudSignInPoll> _answers = new(answers);
        internal int Polls { get; private set; }
        internal List<string> Abandoned { get; } = [];
        internal string? Opened { get; private set; }

        internal Task<NextcloudSignInPoll> Poll(string pending)
        {
            Assert.Equal("pending", pending);
            Polls++;
            return Task.FromResult(_answers.Count > 0 ? _answers.Dequeue() : new NextcloudSignInPoll.Waiting());
        }

        internal Task<bool> Open(string url)
        {
            Opened = url;
            return Task.FromResult(true);
        }
    }

    private static Task<NextcloudFlightEnd> Fly(
        Server server, NextcloudWake wake, CancellationToken cancel, TimeSpan? interval = null,
        Func<string, Task<bool>>? open = null, Func<Task<NextcloudSignInStart>>? begin = null,
        Func<string, Task<NextcloudSignInPoll>>? poll = null) =>
        NextcloudFlight.FlyAsync(
            begin ?? (() => Task.FromResult(Started)),
            open ?? server.Open,
            poll ?? server.Poll,
            server.Abandoned.Add,
            interval ?? Long,
            wake,
            cancel);

    [Fact]
    public async Task A_grant_ends_the_flight_after_the_browser_opened_on_the_login_page()
    {
        var server = new Server(new NextcloudSignInPoll.Granted("grant"));
        var end = await Fly(server, new NextcloudWake(), CancellationToken.None);
        Assert.Equal(new NextcloudFlightEnd.Granted("grant"), end);
        Assert.Equal("https://cloud.example.org/login/v2/flow", server.Opened);
        Assert.Empty(server.Abandoned);
    }

    [Fact]
    public async Task The_flight_asks_again_every_interval_until_the_server_answers()
    {
        var server = new Server(
            new NextcloudSignInPoll.Waiting(), new NextcloudSignInPoll.Waiting(), new NextcloudSignInPoll.Granted("grant"));
        var end = await Fly(server, new NextcloudWake(), CancellationToken.None, TimeSpan.FromMilliseconds(1));
        Assert.IsType<NextcloudFlightEnd.Granted>(end);
        Assert.Equal(3, server.Polls);
    }

    [Fact]
    public async Task Coming_back_to_the_window_asks_at_once_rather_than_at_the_next_interval()
    {
        var server = new Server(new NextcloudSignInPoll.Waiting(), new NextcloudSignInPoll.Granted("grant"));
        var wake = new NextcloudWake();
        var flight = Fly(server, wake, CancellationToken.None);
        // The first ask has answered Waiting, and the flight now waits out a five-minute interval.
        await WaitUntil(() => server.Polls == 1);
        Assert.False(flight.IsCompleted);
        wake.Returned();
        Assert.IsType<NextcloudFlightEnd.Granted>(await flight.WaitAsync(TimeSpan.FromSeconds(10), TestContext.Current.CancellationToken));
        Assert.Equal(2, server.Polls);
    }

    [Fact]
    public async Task A_sign_in_not_finished_in_time_ends_expired()
    {
        var end = await Fly(new Server(new NextcloudSignInPoll.Expired()), new NextcloudWake(), CancellationToken.None);
        Assert.IsType<NextcloudFlightEnd.Expired>(end);
    }

    [Fact]
    public async Task A_browser_that_does_not_open_ends_the_flight_without_polling()
    {
        var server = new Server(new NextcloudSignInPoll.Granted("grant"));
        var end = await Fly(server, new NextcloudWake(), CancellationToken.None, open: _ => Task.FromResult(false));
        Assert.IsType<NextcloudFlightEnd.BrowserFailed>(end);
        Assert.Equal(0, server.Polls);
    }

    [Fact]
    public async Task A_server_that_starts_no_sign_in_or_cannot_be_polled_fails_with_what_the_core_said()
    {
        var refused = new MailcalException.Connect("no");
        var end = await Fly(new Server(), new NextcloudWake(), CancellationToken.None, begin: () => throw refused);
        Assert.Same(refused, Assert.IsType<NextcloudFlightEnd.Failed>(end).Error);

        end = await Fly(new Server(), new NextcloudWake(), CancellationToken.None, poll: _ => throw refused);
        Assert.Same(refused, Assert.IsType<NextcloudFlightEnd.Failed>(end).Error);
    }

    [Fact]
    public async Task Cancel_stops_the_wait_at_once_and_opens_no_browser_after_a_late_begin()
    {
        var server = new Server();
        using var cancel = new CancellationTokenSource();
        var flight = Fly(server, new NextcloudWake(), cancel.Token);
        await WaitUntil(() => server.Polls == 1);
        cancel.Cancel();
        Assert.IsType<NextcloudFlightEnd.Cancelled>(await flight.WaitAsync(TimeSpan.FromSeconds(10), TestContext.Current.CancellationToken));

        var late = new Server();
        using var retired = new CancellationTokenSource();
        var end = await Fly(late, new NextcloudWake(), retired.Token, begin: () =>
        {
            retired.Cancel();
            return Task.FromResult(Started);
        });
        Assert.IsType<NextcloudFlightEnd.Cancelled>(end);
        Assert.Null(late.Opened);
    }

    [Fact]
    public async Task A_grant_answered_after_the_way_back_is_withdrawn_rather_than_dropped()
    {
        var server = new Server();
        using var cancel = new CancellationTokenSource();
        var end = await Fly(server, new NextcloudWake(), cancel.Token, poll: _ =>
        {
            // The person pressed Cancel while this ask was out, and the server granted it anyway.
            cancel.Cancel();
            return Task.FromResult<NextcloudSignInPoll>(new NextcloudSignInPoll.Granted("grant"));
        });
        Assert.IsType<NextcloudFlightEnd.Cancelled>(end);
        Assert.Equal(["grant"], server.Abandoned);
    }

    [Fact]
    public void A_granted_end_never_prints_its_grant()
    {
        Assert.DoesNotContain("secret", new NextcloudFlightEnd.Granted("secret").ToString());
    }

    [Fact]
    public void A_held_grant_is_offered_again_for_what_it_was_granted_for_only()
    {
        var hold = new NextcloudGrantHold();
        hold.Complete("https://cloud.example.org", "grant");
        hold.Completed(spent: false);
        Assert.Equal("grant", hold.HeldFor("https://cloud.example.org"));
        Assert.Null(hold.HeldFor("https://other.example.org"));
    }

    [Fact]
    public void A_grant_the_core_used_or_withdrew_is_forgotten()
    {
        var hold = new NextcloudGrantHold();
        hold.Complete("account", "grant");
        hold.Completed(spent: true);
        Assert.Null(hold.HeldFor("account"));
        Assert.Null(hold.Release());
    }

    [Fact]
    public void A_grant_the_core_is_completing_is_never_released_to_be_withdrawn()
    {
        var hold = new NextcloudGrantHold();
        hold.Complete("account", "grant");
        Assert.True(hold.Completing);
        Assert.Null(hold.Release());
        hold.Completed(spent: false);
        // Left once the completion failed: the surface's way back withdraws it.
        Assert.Equal("grant", hold.Release());
        Assert.Null(hold.Release());
    }

    private static async Task WaitUntil(Func<bool> condition)
    {
        for (var tries = 0; !condition(); tries++)
        {
            Assert.True(tries < 1000, "the flight did not get there");
            await Task.Delay(10);
        }
    }
}
