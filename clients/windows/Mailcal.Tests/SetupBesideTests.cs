// The servers detection found beside a JMAP server, offered on the link step of the account set up
// from that card (docs/onboarding.md, "After the connect: linking another account"): remembered for
// a JMAP account of that address only, offered only for what its session lacks, saying which, and
// used up once taken; and the step drawn for a server lacking a calendar or contacts with nothing to
// pick.

using Allodia.Mailcal.Services;
using uniffi.mailcal_bindings;
using Xunit;

namespace Allodia.Mailcal.Tests;

public class SetupBesideTests
{
    private const string Calendar = "https://cloud.example.org/remote.php/dav/";
    private const string Book = "https://book.example.org/";

    private static DetectedSetup Jmap(string? caldav, string? carddav, bool nextcloud = false) => new(
        new SetupRecommendation.Jmap("alice@example.org", "https://jmap.example.org", true, "autoconfig"),
        false, caldav, carddav, [], NextcloudSignIn: nextcloud);

    private static AccountsSnapshot Snapshot(AccountKind kind, params (AccountCapability, CapabilityState)[] uses) =>
        new([AccountSettingsRulesTests.Entry(kind, uses) with { Id = "alice@jmap" }]);

    private static bool Links(AccountKind kind) => kind is not (AccountKind.Microsoft or AccountKind.Google);

    [Fact]
    public void WhatACardFoundBesideAJmapServerIsKeptForAJmapAccountOfThatAddressOnly()
    {
        var beside = new SetupBeside();
        beside.Detected(Jmap(Calendar, null, nextcloud: true));
        beside.Added("bob@jmap", AccountKind.Jmap, ["bob@example.org"]);
        beside.Added("alice@imap", AccountKind.Imap, ["alice@example.org"]);
        Assert.Null(beside.OfferFor("bob@jmap"));
        Assert.Null(beside.OfferFor("alice@imap"));

        // Set up by hand from the card is the same address, whichever case it was typed in.
        beside.Added("alice@jmap", AccountKind.Jmap, ["Alice@Example.org"]);
        var offer = beside.OfferFor("alice@jmap");
        Assert.Equal(new FoundBeside("alice@example.org", Calendar, null, true), offer?.Found);
        Assert.Equal("cloud.example.org", offer!.Host);
    }

    [Fact]
    public void ACardWithNothingBesideItOrNotJmapKeepsNothing()
    {
        var beside = new SetupBeside();
        beside.Detected(Jmap(null, null));
        beside.Added("alice@jmap", AccountKind.Jmap, ["alice@example.org"]);
        Assert.Null(beside.OfferFor("alice@jmap"));

        beside.Detected(new DetectedSetup(new SetupRecommendation.Manual(MissReason.NothingFound), true, Calendar, null, []));
        beside.Added("alice@jmap", AccountKind.Jmap, ["alice@example.org"]);
        Assert.Null(beside.OfferFor("alice@jmap"));
    }

    [Fact]
    public void TheOfferIsUsedUpOnceTakenAndGoneWhenTheFlowEnds()
    {
        var beside = new SetupBeside();
        beside.Detected(Jmap(Calendar, Book));
        beside.Added("alice@jmap", AccountKind.Jmap, ["alice@example.org"]);
        Assert.NotNull(beside.Take("alice@jmap"));
        Assert.Null(beside.Take("alice@jmap"));
        Assert.Null(beside.OfferFor("alice@jmap"));

        beside.Added("alice@jmap", AccountKind.Jmap, ["alice@example.org"]);
        beside.Reset();
        Assert.Null(beside.OfferFor("alice@jmap"));
        beside.Added("alice@jmap", AccountKind.Jmap, ["alice@example.org"]);
        Assert.Null(beside.OfferFor("alice@jmap"));
    }

    [Fact]
    public void TheServersFillOnlyWhatTheSessionLacks()
    {
        var both = new FoundBeside("alice@example.org", Calendar, Book, false);
        Assert.Equal(BesideFills.Both, BesideStep.Of(both, lacksCalendar: true, lacksContacts: true));
        // A session with contacts of its own is offered a calendar, never a second address book.
        Assert.Equal(BesideFills.Calendar, BesideStep.Of(both, lacksCalendar: true, lacksContacts: false));
        Assert.Equal(BesideFills.Contacts, BesideStep.Of(both, lacksCalendar: false, lacksContacts: true));
        Assert.Null(BesideStep.Of(both, lacksCalendar: false, lacksContacts: false));
        // Contacts are looked for at the calendar's server when no address book was found.
        var calendarOnly = both with { CarddavUrl = null };
        Assert.Equal(BesideFills.Contacts, BesideStep.Of(calendarOnly, lacksCalendar: false, lacksContacts: true));
        Assert.Null(BesideStep.Of(both with { CaldavUrl = null }, lacksCalendar: true, lacksContacts: false));
    }

    [Fact]
    public void AJmapServerWithoutACalendarGetsTheStepWithTheOfferForWhatItLacks()
    {
        var snapshot = Snapshot(AccountKind.Jmap,
            (AccountCapability.Mail, CapabilityState.On),
            (AccountCapability.Calendar, CapabilityState.NotOffered),
            (AccountCapability.Contacts, CapabilityState.On));
        var offer = new BesideOffer("alice@jmap", new FoundBeside("alice@example.org", Calendar, Book, false));
        var step = SetupLinkStep.For(snapshot, "alice@jmap", Links, offer);
        Assert.NotNull(step);
        Assert.Empty(step.Pickers);
        Assert.Equal(BesideFills.Calendar, step.Beside?.Fills);

        // Without servers found beside it, the step is drawn all the same, for "Add another account".
        step = SetupLinkStep.For(snapshot, "alice@jmap", Links);
        Assert.NotNull(step);
        Assert.Null(step.Beside);
    }

    [Fact]
    public void AServerWithItsOwnCalendarAndContactsAndNothingToPickIsNoStep()
    {
        var snapshot = Snapshot(AccountKind.Jmap,
            (AccountCapability.Mail, CapabilityState.On),
            (AccountCapability.Calendar, CapabilityState.On),
            (AccountCapability.Contacts, CapabilityState.On));
        var offer = new BesideOffer("alice@jmap", new FoundBeside("alice@example.org", Calendar, Book, false));
        Assert.Null(SetupLinkStep.For(snapshot, "alice@jmap", Links, offer));
    }
}
