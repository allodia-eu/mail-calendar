// Browser sign-in serialization, as arithmetic. What it pins is exactly what the running app
// cannot show you: a sign-in waiting on a browser tab the user already closed looks identical to
// one the user is still working through, so the only observable symptom of getting this wrong was
// a button that did nothing for five minutes.
//
//   - a second request SUPERSEDES the first rather than being refused, the bug this replaces;
//   - the superseded flow is cancelled AND awaited before the new one starts, so two flows never
//     race for one redirect rendezvous;
//   - a superseded flow's failure never surfaces as the new flow's error;
//   - Cancel still aborts the outstanding flow (the setup form's Cancel button).
//
// This is the WinUI-free half of the flow (SignInFlight), so it runs in the plain net10.0 test
// assembly, no renderer, no cdylib, no browser.
using System;
using System.Collections.Generic;
using System.Threading;
using System.Threading.Tasks;
using Allodia.Mailcal.Services;
using Xunit;

namespace Allodia.Mailcal.Tests;

public class SignInFlightTests
{
    /// A flow that waits for the browser redirect that will never come, exactly as the real one
    /// does once the user closes the tab: it completes only on cancellation.
    private static Task AbandonedInBrowser(CancellationToken cancel) =>
        Task.Delay(Timeout.Infinite, cancel);

    /// Every wait here is bounded, because the regression these tests guard against does not make
    /// them fail, it makes them **hang**. A refused second request leaves the abandoned flow
    /// waiting on a redirect that never comes, so an unbounded await would wedge CI instead of
    /// reporting a red test, and a wedged job reads as "still running", not "broken".
    private static Task Bounded(Task task) => task.WaitAsync(TimeSpan.FromSeconds(10));

    private static Task<T> Bounded<T>(Task<T> task) => task.WaitAsync(TimeSpan.FromSeconds(10));

    [Fact]
    public async Task A_second_request_supersedes_a_sign_in_abandoned_in_the_browser()
    {
        var flight = new SignInFlight();
        var firstStarted = new TaskCompletionSource();
        var secondRan = false;

        var first = flight.RunAsync(async cancel =>
        {
            firstStarted.SetResult();
            try
            {
                await AbandonedInBrowser(cancel);
            }
            catch (OperationCanceledException)
            {
                // The real flow catches this too, and returns to the form quietly.
            }
        });
        await Bounded(firstStarted.Task);

        // The user closed the tab and clicked "sign in again". Before the fix this was a no-op
        // until the five-minute cap elapsed.
        await Bounded(flight.RunAsync(_ =>
        {
            secondRan = true;
            return Task.CompletedTask;
        }));

        Assert.True(secondRan, "the second request must start, not be refused");
        // Awaited rather than asserted on IsCompleted: the superseding request resumes off the
        // superseded flow's completion, so that flow's own Task is marked complete a hair later.
        // Asserting the flag is a race, and a test that fails once in twenty is worse than none.
        await Bounded(first);
    }

    [Fact]
    public async Task The_superseded_flow_finishes_before_the_new_one_starts()
    {
        var flight = new SignInFlight();
        var order = new List<string>();
        var firstStarted = new TaskCompletionSource();

        var first = flight.RunAsync(async cancel =>
        {
            firstStarted.SetResult();
            try
            {
                await AbandonedInBrowser(cancel);
            }
            catch (OperationCanceledException)
            {
                order.Add("first-unwound");
            }
        });
        await Bounded(firstStarted.Task);

        await Bounded(flight.RunAsync(_ =>
        {
            order.Add("second-started");
            return Task.CompletedTask;
        }));
        await Bounded(first);

        // Two flows racing for one redirect rendezvous is the invariant the old guard protected,
        // and superseding must not trade it away: the listener/callback slot of the abandoned flow
        // is released before the replacement arms its own.
        Assert.Equal(new[] { "first-unwound", "second-started" }, order);
    }

    [Fact]
    public async Task A_superseded_failure_is_not_reported_as_the_new_attempt_s_error()
    {
        var flight = new SignInFlight();
        var firstStarted = new TaskCompletionSource();

        _ = flight.RunAsync(async _ =>
        {
            firstStarted.SetResult();
            await Task.Yield();
            throw new InvalidOperationException("the abandoned attempt's own problem");
        });
        await Bounded(firstStarted.Task);

        // Must not throw: the superseded attempt owns its error reporting, and surfacing it here
        // would blame the new attempt for the old one's failure.
        await Bounded(flight.RunAsync(_ => Task.CompletedTask));
    }

    [Fact]
    public async Task Cancel_aborts_the_outstanding_flow()
    {
        var flight = new SignInFlight();
        var started = new TaskCompletionSource();
        var cancelled = false;

        var running = flight.RunAsync(async cancel =>
        {
            started.SetResult();
            try
            {
                await AbandonedInBrowser(cancel);
            }
            catch (OperationCanceledException)
            {
                cancelled = true;
            }
        });
        await Bounded(started.Task);

        flight.Cancel();
        await Bounded(running);

        Assert.True(cancelled);
    }

    [Fact]
    public async Task A_result_returning_flow_serializes_with_the_others_and_keeps_its_result()
    {
        // The JMAP sign-in reports an outcome, and shares ProtocolAuthCallback's single static
        // pending slot with the Microsoft flow, so it must run through the same serializer, and
        // still hand its caller the value the inline note is rendered from.
        var flight = new SignInFlight();
        var firstStarted = new TaskCompletionSource();

        var first = flight.RunAsync(async cancel =>
        {
            firstStarted.SetResult();
            try
            {
                await AbandonedInBrowser(cancel);
            }
            catch (OperationCanceledException)
            {
                // As the real flow does.
            }
        });
        await Bounded(firstStarted.Task);

        var outcome = await Bounded(flight.RunAsync(_ => Task.FromResult("added")));

        Assert.Equal("added", outcome);
        await Bounded(first);
    }

    [Fact]
    public async Task Cancel_with_nothing_running_is_a_no_op()
    {
        var flight = new SignInFlight();

        flight.Cancel();

        // And the next request still runs normally.
        var ran = false;
        await Bounded(flight.RunAsync(_ =>
        {
            ran = true;
            return Task.CompletedTask;
        }));
        Assert.True(ran);
    }
}
