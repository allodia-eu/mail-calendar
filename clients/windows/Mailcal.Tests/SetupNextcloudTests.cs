// Where setup's sign-in with Nextcloud signs in (docs/nextcloud-sign-in.md rule 2): the found
// card's choices name the server and the uses, and the manual calendar and contacts form asks its
// server once per server, drawing the offer only while it still names the one that said yes.

using Allodia.Mailcal.Services;
using uniffi.mailcal_bindings;
using Xunit;

namespace Allodia.Mailcal.Tests;

public class SetupNextcloudTests
{
    private const string Dav = "https://cloud.example.org/remote.php/dav/";

    [Fact]
    public void The_found_card_signs_in_at_the_calendar_server_for_what_was_chosen()
    {
        var start = SetupNextcloudOffer.Found(new ChosenUses([AccountCapability.Calendar], Dav, Dav));
        Assert.Equal(Dav, start!.Server);
        Assert.Equal([AccountCapability.Calendar], start.Uses);
    }

    [Fact]
    public void A_card_that_offered_no_choice_is_used_for_what_its_servers_hold()
    {
        var start = SetupNextcloudOffer.Found(new ChosenUses(null, string.Empty, Dav));
        // The address book's server, when the card found no calendar.
        Assert.Equal(Dav, start!.Server);
        Assert.Equal([AccountCapability.Contacts], start.Uses);
        Assert.Null(SetupNextcloudOffer.Found(new ChosenUses(null, string.Empty, string.Empty)));
    }

    [Fact]
    public void The_manual_form_asks_once_per_server_when_a_field_is_left()
    {
        var offer = new SetupNextcloudOffer();
        Assert.Null(offer.Ask(string.Empty, string.Empty));
        Assert.Equal(Dav, offer.Ask($" {Dav} ", string.Empty));
        // Leaving the other field, or the same one again, asks nothing new.
        Assert.Null(offer.Ask(Dav, string.Empty));
        Assert.Null(offer.Ask(Dav, "https://contacts.example.org/"));
        Assert.Equal("https://other.example.org/", offer.Ask("https://other.example.org/", string.Empty));
    }

    [Fact]
    public void A_yes_is_drawn_only_while_the_form_names_that_server()
    {
        var offer = new SetupNextcloudOffer();
        var server = offer.Ask(Dav, string.Empty)!;
        Assert.Null(offer.OfferedFor(Dav, string.Empty));
        offer.Answered(server, offers: true);
        Assert.Equal(Dav, offer.OfferedFor(Dav, string.Empty));
        Assert.Null(offer.OfferedFor("https://other.example.org/", string.Empty));
        // Typed back, it is the same server, which has answered already.
        Assert.Equal(Dav, offer.OfferedFor(Dav, string.Empty));
    }

    [Fact]
    public void Only_the_answer_for_the_server_asked_last_counts()
    {
        var offer = new SetupNextcloudOffer();
        offer.Ask(Dav, string.Empty);
        offer.Ask("https://other.example.org/", string.Empty);
        offer.Answered(Dav, offers: true);
        Assert.Null(offer.OfferedFor(Dav, string.Empty));
        offer.Answered("https://other.example.org/", offers: false);
        Assert.Null(offer.OfferedFor("https://other.example.org/", string.Empty));
    }

    [Fact]
    public void A_form_opened_from_the_card_offers_it_without_asking_again()
    {
        var offer = new SetupNextcloudOffer();
        offer.Known(Dav);
        Assert.Equal(Dav, offer.OfferedFor(Dav, Dav));
        Assert.Null(offer.Ask(Dav, Dav));
        offer.Reset();
        Assert.Null(offer.OfferedFor(Dav, Dav));
    }

    [Fact]
    public void The_manual_press_is_used_for_what_the_servers_typed_make_it()
    {
        var start = SetupNextcloudOffer.Manual(Dav, Dav, string.Empty);
        Assert.Equal([AccountCapability.Calendar, AccountCapability.Contacts], start!.Uses);
        start = SetupNextcloudOffer.Manual(Dav, string.Empty, Dav);
        Assert.Equal([AccountCapability.Contacts], start!.Uses);
    }
}
