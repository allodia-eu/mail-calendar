// What a setup card's toggles make of an account (docs/account-autodetect.md rule 8,
// docs/accounts.md rules 2 and 10): which uses it is set up with, and which servers it stores.

using Allodia.Mailcal.Services;
using uniffi.mailcal_bindings;
using Xunit;

namespace Allodia.Mailcal.Tests;

public class SetupUsesTests
{
    private const string CalDav = "https://dav.example.org/caldav";
    private const string CardDav = "https://dav.example.org/carddav";
    private static readonly PickedUse On = new(true, string.Empty);
    private static readonly PickedUse Off = new(false, string.Empty);

    private static UseOffer Offer(string caldav, string carddav)
    {
        static SetupChoice Choice(AccountCapability capability, bool found) => new(capability, found, found);
        return new UseOffer(
            [
                Choice(AccountCapability.Mail, true),
                Choice(AccountCapability.Calendar, caldav.Length > 0),
                Choice(AccountCapability.Contacts, caldav.Length > 0 || carddav.Length > 0),
            ],
            caldav,
            carddav);
    }

    private static UseOffer Provider() => new(
        new[] { AccountCapability.Mail, AccountCapability.Calendar, AccountCapability.Contacts, AccountCapability.Colleagues }
            .Select(capability => new SetupChoice(capability, true, true)).ToList(),
        string.Empty,
        string.Empty);

    [Fact]
    public void EverythingFoundAndKeptStoresBothEndpointsAndEveryUse()
    {
        var chosen = SetupUses.Chosen(Offer(CalDav, CardDav), true, On, On, null);
        Assert.Equal([AccountCapability.Mail, AccountCapability.Calendar, AccountCapability.Contacts], chosen.Uses);
        Assert.Equal(CalDav, chosen.CaldavUrl);
        Assert.Equal(CardDav, chosen.CarddavUrl);
    }

    [Fact]
    public void ContactsFoundThroughTheCalendarKeepItsServerWithTheCalendarOff()
    {
        var kept = SetupUses.Chosen(Offer(CalDav, string.Empty), true, Off, On, null);
        Assert.Equal([AccountCapability.Mail, AccountCapability.Contacts], kept.Uses);
        Assert.Equal(CalDav, kept.CaldavUrl);
        Assert.Empty(kept.CarddavUrl);
    }

    [Fact]
    public void AUseSwitchedOffStoresNoServer()
    {
        var mailOnly = SetupUses.Chosen(Offer(CalDav, CardDav), true, Off, Off, null);
        Assert.Equal([AccountCapability.Mail], mailOnly.Uses);
        Assert.Empty(mailOnly.CaldavUrl + mailOnly.CarddavUrl);

        var withoutMail = SetupUses.Chosen(Offer(CalDav, CardDav), false, On, Off, null);
        Assert.Equal([AccountCapability.Calendar], withoutMail.Uses);
        Assert.Empty(withoutMail.CarddavUrl);
    }

    [Fact]
    public void AServerNotFoundIsTheOneTyped()
    {
        var typed = SetupUses.Chosen(
            Offer(string.Empty, string.Empty), true,
            new PickedUse(true, " cal.example.org "), new PickedUse(true, "book.example.org"), null);
        Assert.Equal("cal.example.org", typed.CaldavUrl);
        Assert.Equal("book.example.org", typed.CarddavUrl);

        // A box typed into and then switched off stores nothing.
        var dropped = SetupUses.Chosen(
            Offer(string.Empty, string.Empty), true, new PickedUse(false, "cal.example.org"), Off, null);
        Assert.Empty(dropped.CaldavUrl);
    }

    [Fact]
    public void ACardThatOfferedNoChoiceLeavesTheUsesToTheServers()
    {
        var none = UseOffer.None with { CaldavUrl = CalDav };
        Assert.Null(SetupUses.Chosen(none, null, null, null, null).Uses);
    }

    [Fact]
    public void ContactsAreLookedForAtACalendarTypedInPlaceOfOneFound()
    {
        var typed = SetupUses.Chosen(
            Offer(string.Empty, string.Empty), true, new PickedUse(true, "cal.example.org"), On, null);
        Assert.Equal("cal.example.org", typed.CaldavUrl);
        Assert.Empty(typed.CarddavUrl);
        Assert.Equal([AccountCapability.Mail, AccountCapability.Calendar, AccountCapability.Contacts], typed.Uses);
    }

    [Fact]
    public void ColleaguesCountOnlyBesideContacts()
    {
        Assert.Equal(
            [AccountCapability.Contacts, AccountCapability.Colleagues],
            SetupUses.Chosen(Provider(), false, Off, On, true).Uses);
        Assert.Equal([AccountCapability.Mail], SetupUses.Chosen(Provider(), true, Off, Off, true).Uses);
        // A provider names no server, and none is stored.
        var everything = SetupUses.Chosen(Provider(), true, On, On, true);
        Assert.Empty(everything.CaldavUrl + everything.CarddavUrl);
    }

    [Fact]
    public void AUseSwitchedOnWithNoServerAsksForOne()
    {
        var nothingFound = Offer(string.Empty, string.Empty);
        Assert.Equal(MissingServer.Calendar, SetupUses.Missing(nothingFound, On, On));
        Assert.Equal(MissingServer.None, SetupUses.Missing(nothingFound, new PickedUse(true, "cal"), On));
        // Without a calendar, contacts need an address book of their own.
        Assert.Equal(MissingServer.Contacts, SetupUses.Missing(nothingFound, Off, On));
        Assert.Equal(MissingServer.None, SetupUses.Missing(nothingFound, Off, Off));
        // A server that was found needs nothing typed.
        Assert.Equal(MissingServer.None, SetupUses.Missing(Offer(CalDav, string.Empty), On, On));
    }

    [Fact]
    public void AManualSignInAsksOnlyForWhatTheAddressIsOffered()
    {
        AccountCapability[] chosen = [AccountCapability.Mail, AccountCapability.Contacts, AccountCapability.Colleagues];
        // A personal address is offered no colleagues (docs/accounts.md rule 3).
        var personal = Provider().Choices.Where(choice => choice.Capability != AccountCapability.Colleagues).ToList();
        Assert.Equal([AccountCapability.Mail, AccountCapability.Contacts], SetupUses.Allowed(chosen, personal));
        Assert.Equal(chosen, SetupUses.Allowed(chosen, Provider().Choices));
        Assert.Null(SetupUses.Allowed(null, Provider().Choices));
    }

    [Fact]
    public void TheServersTypedDecideWhatACalendarAndContactsAccountIsUsedFor()
    {
        Assert.Equal([AccountCapability.Calendar, AccountCapability.Contacts], SetupUses.DavUses("cloud.example", ""));
        Assert.Equal([AccountCapability.Calendar, AccountCapability.Contacts], SetupUses.DavUses("cloud.example", "book.example"));
        Assert.Equal([AccountCapability.Contacts], SetupUses.DavUses("", "book.example"));
        Assert.Null(SetupUses.DavUses(" ", ""));
    }

    [Fact]
    public void ADiscoveredEndpointIsShownByHost()
    {
        Assert.Equal("caldav.example.test", SetupUses.Host("https://caldav.example.test/dav/"));
        Assert.Equal("not a url", SetupUses.Host("not a url"));
    }
}
