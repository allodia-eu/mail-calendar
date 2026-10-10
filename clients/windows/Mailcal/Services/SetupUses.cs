// What a setup card offers an account for, and what the person's choice makes of it
// (docs/account-autodetect.md rule 8, docs/accounts.md rules 2 and 10). Which uses are offered,
// and how each starts, is the core's (`detect_account_setup`, `provider_setup_choices`); this file
// turns the toggles on screen into the uses and servers the account is set up with. WinUI-free so
// the rules are reachable from Mailcal.Tests.

using uniffi.mailcal_bindings;

namespace Allodia.Mailcal.Services;

/// <summary>The choices the core offered, in its order, and the servers detection found beside the
/// route; an empty URL is one it did not find.</summary>
internal sealed record UseOffer(IReadOnlyList<SetupChoice> Choices, string CaldavUrl, string CarddavUrl)
{
    public static readonly UseOffer None = new([], string.Empty, string.Empty);

    /// <summary>The offer a detection run answered with.</summary>
    public static UseOffer Of(DetectedSetup setup) =>
        new(setup.Choices, setup.CaldavUrl ?? string.Empty, setup.CarddavUrl ?? string.Empty);

    /// <summary>The choice the core offered for <paramref name="capability"/>, if any.</summary>
    public SetupChoice? For(AccountCapability capability) =>
        Choices.FirstOrDefault(choice => choice.Capability == capability);
}

/// <summary>A calendar or contacts toggle as the person left it, and the server typed beneath it
/// when detection found none.</summary>
internal readonly record struct PickedUse(bool On, string Typed);

/// <summary>What the person chose, as the account is set up with it.</summary>
/// <param name="Uses">Null when the card offered no choice, so the servers given decide.</param>
internal sealed record ChosenUses(AccountCapability[]? Uses, string CaldavUrl, string CarddavUrl);

/// <summary>Which typed server a choice still needs before the account can be set up.</summary>
internal enum MissingServer
{
    None,
    Calendar,
    Contacts,
}

internal static class SetupUses
{
    /// <summary>
    /// The account a card's toggles describe. A calendar stores the endpoint detection found, or
    /// the one typed in its place. Contacts store an address book found apart from the calendar,
    /// or the one typed. Contacts found only through the calendar's server keep that server even
    /// with the calendar off, because that is where they are looked for, and the stored uses keep
    /// the calendar closed. Colleagues count only beside contacts.
    /// </summary>
    internal static ChosenUses Chosen(
        UseOffer offer, bool? mail, PickedUse? calendar, PickedUse? contacts, bool? colleagues)
    {
        var calendarOn = calendar is { On: true };
        var contactsOn = contacts is { On: true };
        static string FoundOrTyped(string found, PickedUse? picked) =>
            found.Length > 0 ? found : picked?.Typed.Trim() ?? string.Empty;
        var carddav = contactsOn && (offer.CarddavUrl.Length > 0 || offer.CaldavUrl.Length == 0)
            ? FoundOrTyped(offer.CarddavUrl, contacts)
            : string.Empty;
        var throughCalendar = contactsOn && carddav.Length == 0 && offer.CaldavUrl.Length > 0;
        var caldav = calendarOn || throughCalendar ? FoundOrTyped(offer.CaldavUrl, calendar) : string.Empty;
        AccountCapability[]? uses = null;
        if (offer.Choices.Count > 0)
        {
            var chosen = new List<AccountCapability>();
            if (mail == true)
            {
                chosen.Add(AccountCapability.Mail);
            }
            if (calendarOn)
            {
                chosen.Add(AccountCapability.Calendar);
            }
            if (contactsOn)
            {
                chosen.Add(AccountCapability.Contacts);
                if (colleagues == true)
                {
                    chosen.Add(AccountCapability.Colleagues);
                }
            }
            uses = [.. chosen];
        }
        return new ChosenUses(uses, caldav, carddav);
    }

    /// <summary>
    /// The server still to be typed for a use switched on: the calendar's, or the address book's
    /// when no calendar gives contacts a server to be looked for at.
    /// </summary>
    internal static MissingServer Missing(UseOffer offer, PickedUse? calendar, PickedUse? contacts)
    {
        static bool Empty(SetupChoice? choice, PickedUse? picked) =>
            choice is { ServerFound: false } && picked is { On: true } && picked.Value.Typed.Trim().Length == 0;
        if (Empty(offer.For(AccountCapability.Calendar), calendar))
        {
            return MissingServer.Calendar;
        }
        return calendar is not { On: true } && Empty(offer.For(AccountCapability.Contacts), contacts)
            ? MissingServer.Contacts
            : MissingServer.None;
    }

    /// <summary>
    /// What a manual form's provider sign-in asks for: the uses chosen on screen, less any the
    /// address it signs in with is not offered, so a personal address is never asked for
    /// colleagues.
    /// </summary>
    internal static AccountCapability[]? Allowed(AccountCapability[]? chosen, IReadOnlyList<SetupChoice> offered) =>
        chosen?.Where(capability => offered.Any(choice => choice.Capability == capability)).ToArray();

    /// <summary>
    /// What the servers typed on the manual calendar-and-contacts form make the account used for:
    /// its calendar beside a calendar server, and contacts beside either, since they are looked
    /// for at the calendar's server when no address book is given. Null when neither is typed,
    /// which is no account.
    /// </summary>
    internal static AccountCapability[]? DavUses(string caldav, string carddav) =>
        (caldav.Trim().Length > 0, carddav.Trim().Length > 0) switch
        {
            (true, _) => [AccountCapability.Calendar, AccountCapability.Contacts],
            (false, true) => [AccountCapability.Contacts],
            _ => null,
        };

    /// <summary>The host of a discovered URL, for a line the person can recognise; the whole URL
    /// when it does not parse.</summary>
    internal static string Host(string url) =>
        Uri.TryCreate(url, UriKind.Absolute, out var parsed) && parsed.Host.Length > 0 ? parsed.Host : url;
}
