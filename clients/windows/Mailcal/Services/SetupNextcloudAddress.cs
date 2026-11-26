// The manual form's Nextcloud pane (docs/nextcloud-sign-in.md rule 2): the server's address alone,
// which the core asks whether it is Nextcloud before a browser opens. When it cannot tell, the pane
// says so and offers the sign-in anyway for as long as it holds the address the core was asked
// about. WinUI-free and L10n-free, so Mailcal.Tests drives it.

using uniffi.mailcal_bindings;

namespace Allodia.Mailcal.Services;

internal sealed class SetupNextcloudAddress
{
    /// <summary>What an account signed in to from the pane is used for: the calendar and the
    /// address book Nextcloud serves side by side.</summary>
    internal static readonly AccountCapability[] Uses = [AccountCapability.Calendar, AccountCapability.Contacts];

    private (string Address, string Host)? _unsure;

    /// <summary>The address the pane holds, as the core is given it.</summary>
    internal static string Address(string typed) => typed.Trim();

    /// <summary>Whether the pane's sign-in can be pressed: it holds an address.</summary>
    internal static bool CanSignIn(string typed) => Address(typed).Length > 0;

    /// <summary>A press on the pane: at the address it holds, asked first unless
    /// <paramref name="anyway"/>.</summary>
    internal static NextcloudStart Start(string typed, bool anyway) =>
        new(Address(typed), Uses, anyway ? NextcloudBegin.Anyway : NextcloudBegin.Asked);

    /// <summary>A sign-in starts, which answers any offer to sign in anyway.</summary>
    internal void Started() => _unsure = null;

    /// <summary>The form opens again with nothing asked.</summary>
    internal void Reset() => _unsure = null;

    /// <summary>The sign-in a press on <paramref name="start"/> started ended as
    /// <paramref name="end"/>: one the core could not tell was Nextcloud is offered anyway.</summary>
    internal void Ended(NextcloudStart start, NextcloudFlightEnd? end)
    {
        if (UnsureHost(end) is { } host)
        {
            _unsure = (start.Server, host);
        }
    }

    /// <summary>The host the pane says it could not confirm, offering the sign-in anyway, while it
    /// holds <paramref name="typed"/>; null once it holds another address.</summary>
    internal string? UnsureFor(string typed) =>
        _unsure is { } unsure && unsure.Address == Address(typed) ? unsure.Host : null;

    /// <summary>The host of an address the core could not tell was Nextcloud.</summary>
    internal static string? UnsureHost(NextcloudFlightEnd? end) =>
        end is NextcloudFlightEnd.NotStarted { Answer: NextcloudAddressStart.Unsure unsure } ? unsure.Host : null;

    /// <summary>
    /// Starts a sign-in at a typed address: <paramref name="ask"/> is the core's start (a blocking
    /// call the caller runs off the UI thread), and only an address that started one is flown, by
    /// <paramref name="fly"/>. One that did not ends <see cref="NextcloudFlightEnd.NotStarted"/>,
    /// and one raised while the core is asking ends the attempt at once: the answer, when it
    /// comes, opens no browser.
    /// </summary>
    internal static async Task<NextcloudFlightEnd> AskThenFlyAsync(
        Func<Task<NextcloudAddressStart>> ask,
        Func<NextcloudSignInStart, Task<NextcloudFlightEnd>> fly,
        CancellationToken cancel)
    {
        NextcloudAddressStart answer;
        try
        {
            var asked = ask();
            if (await Task.WhenAny(asked, Task.Delay(Timeout.Infinite, cancel)) != asked)
            {
                return new NextcloudFlightEnd.Cancelled();
            }
            answer = await asked;
        }
        catch (Exception ex)
        {
            return cancel.IsCancellationRequested ? new NextcloudFlightEnd.Cancelled() : new NextcloudFlightEnd.Failed(ex);
        }
        if (cancel.IsCancellationRequested)
        {
            return new NextcloudFlightEnd.Cancelled();
        }
        return answer is NextcloudAddressStart.Started started
            ? await fly(started.Start)
            : new NextcloudFlightEnd.NotStarted(answer);
    }
}
