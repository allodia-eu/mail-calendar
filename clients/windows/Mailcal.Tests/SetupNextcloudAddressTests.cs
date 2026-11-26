// The manual form's Nextcloud pane (docs/nextcloud-sign-in.md rule 2): every account type the core
// offers is drawn as a tab of its own; the pane signs in at the address it holds, for calendar and
// contacts; only an address that started a sign-in is flown; and the offer to sign in anyway
// stands while the pane holds the address the core could not tell was Nextcloud.

using Allodia.Mailcal.Services;
using uniffi.mailcal_bindings;
using Xunit;

namespace Allodia.Mailcal.Tests;

public class SetupNextcloudAddressTests
{
    private const string Address = "cloud.example.org";

    private static readonly NextcloudSignInStart Begun = new("https://cloud.example.org/login/v2/flow", "pending");

    private static NextcloudFlightEnd Unsure(string host) =>
        new NextcloudFlightEnd.NotStarted(new NextcloudAddressStart.Unsure(host));

    [Fact]
    public void Every_account_type_the_core_can_offer_is_a_tab_of_its_own()
    {
        var kinds = Enum.GetValues<ManualSetupKind>();
        var tabs = kinds.Select(AccountDetectForm.Tab).ToList();
        Assert.Equal(kinds.Length, tabs.Distinct().Count());
        Assert.Equal(DetectTab.Imap, AccountDetectForm.Tab(ManualSetupKind.Password));
        Assert.Equal(DetectTab.Nextcloud, AccountDetectForm.Tab(ManualSetupKind.Nextcloud));
        Assert.Equal(DetectTab.Dav, AccountDetectForm.Tab(ManualSetupKind.Dav));
    }

    [Fact]
    public void The_nextcloud_tab_has_no_connect_of_its_own()
    {
        Assert.False(AccountDetectForm.CanConnect(
            DetectTab.Nextcloud, needsApproval: false, approved: false,
            imapHost: "imap.example.org", email: "alice@example.org", password: "secret", jmapSecret: "secret"));
        Assert.False(AccountDetectForm.OffersCertificateException(DetectTab.Nextcloud));
    }

    [Fact]
    public void The_pane_signs_in_at_the_address_it_holds_for_calendar_and_contacts()
    {
        Assert.False(SetupNextcloudAddress.CanSignIn("   "));
        Assert.True(SetupNextcloudAddress.CanSignIn(Address));

        var asked = SetupNextcloudAddress.Start($" {Address} ", anyway: false);
        Assert.Equal(Address, asked.Server);
        Assert.Equal(NextcloudBegin.Asked, asked.Begin);
        Assert.Equal([AccountCapability.Calendar, AccountCapability.Contacts], asked.Uses);
        Assert.Equal(NextcloudBegin.Anyway, SetupNextcloudAddress.Start(Address, anyway: true).Begin);
    }

    [Fact]
    public void An_address_the_core_could_not_tell_is_offered_anyway_while_the_pane_holds_it()
    {
        var pane = new SetupNextcloudAddress();
        var start = SetupNextcloudAddress.Start(Address, anyway: false);
        Assert.Null(pane.UnsureFor(Address));

        pane.Ended(start, Unsure("cloud.example.org"));
        Assert.Equal("cloud.example.org", pane.UnsureFor(Address));
        Assert.Equal("cloud.example.org", pane.UnsureFor($"{Address} "));
        Assert.Null(pane.UnsureFor("cloud.example.com"));
        // Typed back, the offer is back.
        Assert.Equal("cloud.example.org", pane.UnsureFor(Address));

        // A sign-in starting answers the offer, whichever way it ends.
        pane.Started();
        Assert.Null(pane.UnsureFor(Address));
        pane.Ended(start, new NextcloudFlightEnd.Cancelled());
        Assert.Null(pane.UnsureFor(Address));
    }

    [Fact]
    public void Only_an_unsure_answer_is_offered_anyway()
    {
        Assert.Equal("h", SetupNextcloudAddress.UnsureHost(Unsure("h")));
        Assert.Null(SetupNextcloudAddress.UnsureHost(null));
        Assert.Null(SetupNextcloudAddress.UnsureHost(new NextcloudFlightEnd.Expired()));
        Assert.Null(SetupNextcloudAddress.UnsureHost(
            new NextcloudFlightEnd.NotStarted(new NextcloudAddressStart.NotNextcloud("h"))));
        Assert.Null(SetupNextcloudAddress.UnsureHost(
            new NextcloudFlightEnd.NotStarted(new NextcloudAddressStart.Maintenance("h"))));
        Assert.Null(SetupNextcloudAddress.UnsureHost(
            new NextcloudFlightEnd.NotStarted(new NextcloudAddressStart.NotAnAddress())));

        var pane = new SetupNextcloudAddress();
        pane.Ended(SetupNextcloudAddress.Start(Address, anyway: true),
            new NextcloudFlightEnd.NotStarted(new NextcloudAddressStart.NotNextcloud(Address)));
        Assert.Null(pane.UnsureFor(Address));
    }

    [Fact]
    public async Task An_address_that_started_a_sign_in_is_flown()
    {
        NextcloudSignInStart? flown = null;
        var end = await SetupNextcloudAddress.AskThenFlyAsync(
            () => Task.FromResult<NextcloudAddressStart>(new NextcloudAddressStart.Started(Begun)),
            begun =>
            {
                flown = begun;
                return Task.FromResult<NextcloudFlightEnd>(new NextcloudFlightEnd.Granted("grant"));
            },
            CancellationToken.None);
        Assert.Equal(Begun, flown);
        Assert.Equal(new NextcloudFlightEnd.Granted("grant"), end);
    }

    [Theory]
    [InlineData("unsure")]
    [InlineData("maintenance")]
    [InlineData("other")]
    [InlineData("nothing")]
    public async Task An_address_that_started_none_opens_no_browser(string answered)
    {
        NextcloudAddressStart answer = answered switch
        {
            "unsure" => new NextcloudAddressStart.Unsure("h"),
            "maintenance" => new NextcloudAddressStart.Maintenance("h"),
            "other" => new NextcloudAddressStart.NotNextcloud("h"),
            _ => new NextcloudAddressStart.NotAnAddress(),
        };
        var end = await SetupNextcloudAddress.AskThenFlyAsync(
            () => Task.FromResult(answer),
            _ => throw new InvalidOperationException("nothing started, so nothing is flown"),
            CancellationToken.None);
        Assert.Equal(new NextcloudFlightEnd.NotStarted(answer), end);
    }

    [Fact]
    public async Task A_core_that_refused_ends_the_attempt_failed()
    {
        var refused = new InvalidOperationException("refused");
        var end = await SetupNextcloudAddress.AskThenFlyAsync(
            () => Task.FromException<NextcloudAddressStart>(refused),
            _ => throw new InvalidOperationException("nothing started, so nothing is flown"),
            CancellationToken.None);
        Assert.Equal(new NextcloudFlightEnd.Failed(refused), end);
    }

    [Fact]
    public async Task The_way_back_while_the_server_is_asked_ends_the_attempt_at_once()
    {
        using var cancel = new CancellationTokenSource();
        var answer = new TaskCompletionSource<NextcloudAddressStart>();
        var flying = SetupNextcloudAddress.AskThenFlyAsync(
            () => answer.Task,
            _ => throw new InvalidOperationException("an answer after the way back opens no browser"),
            cancel.Token);
        Assert.False(flying.IsCompleted);

        await cancel.CancelAsync();
        Assert.Equal(new NextcloudFlightEnd.Cancelled(), await flying.WaitAsync(TimeSpan.FromSeconds(5), TestContext.Current.CancellationToken));
        // The answer that still comes is dropped.
        answer.SetResult(new NextcloudAddressStart.Started(Begun));
    }
}
