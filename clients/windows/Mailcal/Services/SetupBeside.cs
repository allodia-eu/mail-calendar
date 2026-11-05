// The calendar and address-book servers detection found beside a JMAP server, offered on the link
// step of the account set up from it as an account of their own, for what its session lacks and
// only that (docs/onboarding.md, "After the connect: linking another account";
// MailcalApp::dav_setup_beside). WinUI-free and L10n-free, so Mailcal.Tests drives it.

using uniffi.mailcal_bindings;

namespace Allodia.Mailcal.Services;

/// <summary>The servers detection found beside a JMAP server, for the address it was found for.</summary>
/// <param name="Nextcloud">They are Nextcloud, which signs in through the browser.</param>
internal sealed record FoundBeside(string Email, string? CaldavUrl, string? CarddavUrl, bool Nextcloud)
{
    /// <summary>What a detected JMAP card found beside its server; null for any other card, or one
    /// that found nothing beside it.</summary>
    internal static FoundBeside? Of(DetectedSetup setup) =>
        setup.Recommendation is SetupRecommendation.Jmap jmap && (setup.CaldavUrl is not null || setup.CarddavUrl is not null)
            ? new FoundBeside(jmap.Email, setup.CaldavUrl, setup.CarddavUrl, setup.NextcloudSignIn)
            : null;
}

/// <summary>The offer for one account: the servers found beside the JMAP server it was set up
/// from.</summary>
internal sealed record BesideOffer(string Account, FoundBeside Found)
{
    /// <summary>The host a person recognises the servers by.</summary>
    internal string Host => (Found.CaldavUrl ?? Found.CarddavUrl) is { } url ? SetupUses.Host(url) : string.Empty;
}

/// <summary>What the servers found beside a JMAP server fill of what its session lacks.</summary>
internal enum BesideFills
{
    Calendar,
    Contacts,
    Both,
}

/// <summary>The offer on a link step, and what it fills.</summary>
internal sealed record BesideStep(BesideOffer Offer, BesideFills Fills)
{
    /// <summary>What <paramref name="found"/> fills of what the account lacks; null when nothing.
    /// Contacts are looked for at the calendar's server when no address book was found, as
    /// <c>dav_setup_beside</c> does.</summary>
    internal static BesideFills? Of(FoundBeside found, bool lacksCalendar, bool lacksContacts)
    {
        var calendar = lacksCalendar && found.CaldavUrl is not null;
        var contacts = lacksContacts && (found.CarddavUrl is not null || found.CaldavUrl is not null);
        return (calendar, contacts) switch
        {
            (true, true) => BesideFills.Both,
            (true, false) => BesideFills.Calendar,
            (false, true) => BesideFills.Contacts,
            _ => null,
        };
    }
}

/// <summary>
/// What one setup flow knows: the servers found beside the JMAP server last detected, kept past
/// "Set up manually", and the offer kept for each JMAP account added from it until its link step
/// uses it.
/// </summary>
internal sealed class SetupBeside
{
    private FoundBeside? _found;
    private readonly List<BesideOffer> _offers = [];

    /// <summary>Detection answered, or a card was opened: what it found beside a JMAP server, or
    /// nothing.</summary>
    internal void Detected(DetectedSetup setup) => _found = FoundBeside.Of(setup);

    /// <summary><paramref name="account"/> was added. A JMAP account set up for the address the
    /// servers were found for, from the card or by hand from it, keeps them for its link step.</summary>
    internal void Added(string account, AccountKind kind, IEnumerable<string> addresses)
    {
        if (kind != AccountKind.Jmap || _found is not { } found
            || !addresses.Any(address => string.Equals(address.Trim(), found.Email, StringComparison.OrdinalIgnoreCase)))
        {
            return;
        }
        _offers.RemoveAll(offer => offer.Account == account);
        _offers.Add(new BesideOffer(account, found));
    }

    /// <summary>The offer kept for <paramref name="account"/>, while unused.</summary>
    internal BesideOffer? OfferFor(string account) => _offers.FirstOrDefault(offer => offer.Account == account);

    /// <summary>The link step's offer was taken: it is used up.</summary>
    internal BesideOffer? Take(string account)
    {
        var offer = OfferFor(account);
        if (offer is not null)
        {
            _offers.Remove(offer);
        }
        return offer;
    }

    /// <summary>The flow ended.</summary>
    internal void Reset()
    {
        _found = null;
        _offers.Clear();
    }
}
