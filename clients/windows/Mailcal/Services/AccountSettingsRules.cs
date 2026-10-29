// What Settings → Accounts says about an account, decided from its snapshot entry alone
// (docs/accounts.md rules 11 to 13), how it signs in and what it can switch to (rule 17), and
// what an expired sign-in and a refused duplicate offer. WinUI-free and L10n-free on purpose, so
// each rule is reachable from Mailcal.Tests: the words are the dialog's, the decisions are here.

using uniffi.mailcal_bindings;

namespace Allodia.Mailcal.Services;

/// <summary>Which line, if any, sits under a use's switch.</summary>
internal enum UseNote
{
    None,
    /// <summary>The last of mail, calendar and contacts: the account is removed instead.</summary>
    Last,
    /// <summary>Colleagues, while contacts are off.</summary>
    NeedsContacts,
    /// <summary>Shared mailboxes, while mail is off.</summary>
    NeedsMail,
    /// <summary>Chosen, and the provider has not granted it.</summary>
    Withheld,
    /// <summary>The server offers none, so nothing can switch it on.</summary>
    NotOffered,
    /// <summary>On, and its server could not be reached; the next refresh tries again.</summary>
    UnreachableCalendar,
    /// <summary>As <see cref="UnreachableCalendar"/>, for the address book.</summary>
    UnreachableContacts,
    /// <summary>On, and its server refused the account's sign-in.</summary>
    RefusedCalendar,
    /// <summary>As <see cref="RefusedCalendar"/>, for the address book.</summary>
    RefusedContacts,
    /// <summary>On, and its server needs a sign-in of its own.</summary>
    OwnSignInCalendar,
    /// <summary>As <see cref="OwnSignInCalendar"/>, for the address book.</summary>
    OwnSignInContacts,
    /// <summary>On, and paused for a server found on another site that the person has not
    /// allowed yet.</summary>
    ServerChangedCalendar,
    /// <summary>As <see cref="ServerChangedCalendar"/>, for the address book.</summary>
    ServerChangedContacts,
}

/// <summary>How one use's switch is drawn.</summary>
/// <param name="Active">On. A use waiting on a permission is drawn off: it is not working, and
/// switching it on is what asks the provider again.</param>
/// <param name="Asks">Whether switching it on asks the provider rather than the core.</param>
/// <param name="Enabled">Whether the person can flip it.</param>
internal sealed record UseSwitch(bool Active, bool Asks, bool Enabled, UseNote Note);

/// <summary>One link slot's picker: "none" first, then <see cref="Options"/>.</summary>
/// <param name="Selected">The index into <see cref="Options"/> of the account linked now; null
/// for "none".</param>
/// <param name="Suggested">The index of the account to suggest while nothing is linked: one whose
/// calendar server schedules as the mail account's address.</param>
internal sealed record LinkPicker(
    LinkSlot Slot, IReadOnlyList<LinkedAccount> Options, int? Selected, int? Suggested);

/// <summary>What the page does once switching a use has answered.</summary>
internal enum UseChangeOutcome
{
    /// <summary>Done; the page redraws from the next snapshot.</summary>
    Applied,
    /// <summary>The provider has not granted it: sign the account in again, asking for it.</summary>
    AskProvider,
    /// <summary>The account has no server for it.</summary>
    NeedsEndpoint,
}

/// <summary>The Sign-in section of a standards or JMAP account's page.</summary>
/// <param name="ThroughProvider">Whether it signs in through its provider now, rather than with a
/// password: the status line.</param>
/// <param name="SignInAgain">"Sign in again", from its stored grant.</param>
/// <param name="SwitchToProvider">Signing in through the provider in place of its password.</param>
/// <param name="ProviderLabel">The provider's name for that button, when the core has one.</param>
/// <param name="SwitchToPassword">A password in place of its grant.</param>
internal sealed record SignInSection(
    bool ThroughProvider, bool SignInAgain, bool SwitchToProvider, string? ProviderLabel, bool SwitchToPassword)
{
    /// <summary>Whether a switch is offered, which the line saying what it keeps goes with.</summary>
    internal bool OffersSwitch => SwitchToProvider || SwitchToPassword;
}

/// <summary>What an expired sign-in's banner button runs.</summary>
internal enum SignInRemedy
{
    /// <summary>No browser flow: the password or API token is replaced in Settings.</summary>
    Settings,
    /// <summary>Microsoft's or Google's own consent, in place.</summary>
    ProviderConsent,
    /// <summary>A JMAP account's stored grant, authorised again.</summary>
    JmapReauth,
    /// <summary>A standards account's stored grant, through the account sign-in.</summary>
    AccountSignIn,
}

/// <summary>The account an add was refused for, and the config the add was given, which the
/// refusal offers to put on it (docs/accounts.md rule 17).</summary>
/// <param name="Unreached">The calendar and contacts the sign-in that gave the config said it would
/// not reach, which taking the offer asks about first (rule 18).</param>
internal sealed record ExistingAccountOffer(string AccountId, string ConfigToml, IReadOnlyList<AccountCapability> Unreached);

internal static class AccountSettingsRules
{
    private static readonly AccountCapability[] Primary =
        [AccountCapability.Mail, AccountCapability.Calendar, AccountCapability.Contacts];

    /// <summary>The uses the account is used for, working or waiting on a permission, in the
    /// snapshot's order.</summary>
    internal static IReadOnlyList<AccountCapability> InUse(AccountEntry entry) =>
        entry.Uses.Where(use => Works(use.State)).Select(use => use.Capability).ToList();

    /// <summary>Whether the account is used for a use in <paramref name="state"/>: on, or waiting
    /// on the provider's permission. A use the server does not offer is not one, whatever was
    /// chosen.</summary>
    private static bool Works(CapabilityState state) =>
        state is CapabilityState.On or CapabilityState.NeedsPermission;

    /// <summary>The links the account holds, in the order the list row names them.</summary>
    internal static IReadOnlyList<(AccountCapability Use, LinkedAccount Account)> Linked(AccountEntry entry)
    {
        var linked = new List<(AccountCapability, LinkedAccount)>();
        if (entry.Links.Calendar is { } calendar)
        {
            linked.Add((AccountCapability.Calendar, calendar));
        }
        if (entry.Links.Contacts is { } contacts)
        {
            linked.Add((AccountCapability.Contacts, contacts));
        }
        if (entry.Links.Mail is { } mail)
        {
            linked.Add((AccountCapability.Mail, mail));
        }
        return linked;
    }

    /// <summary>Whether a use the account is used for waits on the provider's permission.</summary>
    internal static bool NeedsPermission(AccountEntry entry) =>
        entry.Uses.Any(use => use.State == CapabilityState.NeedsPermission);

    /// <summary>The uses waiting on the provider's permission, for the notice a sign-in that
    /// withheld some of them ends with.</summary>
    internal static IReadOnlyList<AccountCapability> Withheld(AccountEntry entry) =>
        entry.Uses.Where(use => use.State == CapabilityState.NeedsPermission)
            .Select(use => use.Capability).ToList();

    /// <summary>Whether the account signs in at its provider's own page, which "Sign in again"
    /// opens.</summary>
    internal static bool SignsInAtProvider(AccountKind kind) =>
        kind is AccountKind.Microsoft or AccountKind.Google;

    /// <summary>Whether an account of <paramref name="kind"/> has the Sign-in section that switches
    /// between a password and its provider: one with a mailbox server to ask, a standards or JMAP
    /// account.</summary>
    internal static bool SwitchesSignIn(AccountKind kind) => kind is AccountKind.Imap or AccountKind.Jmap;

    /// <summary>
    /// The Sign-in section for <paramref name="entry"/>, given what its server accepts
    /// (<paramref name="options"/>, null until <c>account_sign_in_options</c> answers). Null for a
    /// Microsoft or Google account, which signs in again through its own consent, and for a
    /// calendar or address book used for no mail.
    /// </summary>
    internal static SignInSection? SignIn(AccountEntry entry, AccountSignInOptions? options)
    {
        if (!SwitchesSignIn(entry.Kind))
        {
            return null;
        }
        var throughProvider = entry.SignIn switch
        {
            SignInMethod.Provider => true,
            _ => false,
        };
        var signIn = options?.SignIn ?? false;
        var password = options?.Password ?? false;
        return new SignInSection(
            ThroughProvider: throughProvider,
            SignInAgain: throughProvider && signIn,
            SwitchToProvider: !throughProvider && signIn,
            ProviderLabel: !throughProvider && signIn ? options?.ProviderLabel : null,
            SwitchToPassword: throughProvider && password);
    }

    /// <summary>What the expired-sign-in banner runs for an account of <paramref name="provider"/>.
    /// Null, a provider the core could not name, has nothing to run.</summary>
    internal static SignInRemedy Remedy(AccountProvider? provider) => provider switch
    {
        AccountProvider.Microsoft or AccountProvider.Google => SignInRemedy.ProviderConsent,
        AccountProvider.JmapOauth => SignInRemedy.JmapReauth,
        AccountProvider.ImapOauth => SignInRemedy.AccountSignIn,
        AccountProvider.Password or AccountProvider.Jmap => SignInRemedy.Settings,
        _ => SignInRemedy.Settings,
    };

    /// <summary>What a refused add offers: putting <paramref name="configToml"/> on the account
    /// already there, when the refusal is that it is set up already.</summary>
    internal static ExistingAccountOffer? UpdateOffer(
        Exception error, string configToml, IReadOnlyList<AccountCapability>? unreached = null) => error switch
    {
        MailcalException.AlreadySetUp twice when !string.IsNullOrEmpty(configToml) =>
            new ExistingAccountOffer(twice.accountId, configToml, UnreachedUsesRules.Dav(unreached)),
        _ => null,
    };

    /// <summary>What a use that did not connect says beneath its switch, and on the screen that
    /// shows it (<see cref="UseOutageLines"/>). Only a calendar or an address book fails on its own;
    /// mail decides whether the account connected at all.</summary>
    internal static UseNote ProblemNote(AccountCapability capability, UseProblem problem)
    {
        var calendar = capability == AccountCapability.Calendar;
        return problem switch
        {
            UseProblem.Unreachable => calendar ? UseNote.UnreachableCalendar : UseNote.UnreachableContacts,
            UseProblem.SignInRefused => calendar ? UseNote.RefusedCalendar : UseNote.RefusedContacts,
            UseProblem.NeedsOwnSignIn => calendar ? UseNote.OwnSignInCalendar : UseNote.OwnSignInContacts,
            UseProblem.ServerChanged => calendar ? UseNote.ServerChangedCalendar : UseNote.ServerChangedContacts,
            _ => calendar ? UseNote.OwnSignInCalendar : UseNote.OwnSignInContacts,
        };
    }

    /// <summary>Whether <paramref name="note"/> offers a review of the server change, which is
    /// what allows the new server.</summary>
    internal static bool ReviewsServerChange(UseNote note) =>
        note is UseNote.ServerChangedCalendar or UseNote.ServerChangedContacts;

    /// <summary>
    /// The switch for <paramref name="capability"/>. The last of mail, calendar and contacts
    /// cannot be switched off, and colleagues cannot be switched on without contacts, nor shared
    /// mailboxes without mail: the core refuses each, so the switch says why rather than letting
    /// the person find out.
    /// </summary>
    internal static UseSwitch Switch(AccountEntry entry, AccountCapability capability)
    {
        bool Used(AccountCapability wanted) =>
            entry.Uses.Any(use => use.Capability == wanted && Works(use.State));
        var state = entry.Uses.FirstOrDefault(use => use.Capability == capability)?.State
            ?? CapabilityState.Off;
        // A use waiting on a permission is drawn off, so it is never the one held on: switching it
        // on asks the provider, which the core does not refuse.
        var last = state == CapabilityState.On
            && Primary.Contains(capability)
            && Primary.Count(Used) == 1;
        bool Without(AccountCapability sub, AccountCapability within) =>
            capability == sub && state == CapabilityState.Off && !Used(within);
        var withoutContacts = Without(AccountCapability.Colleagues, AccountCapability.Contacts);
        var withoutMail = Without(AccountCapability.SharedMailboxes, AccountCapability.Mail);
        var notOffered = state == CapabilityState.NotOffered;
        var problem = entry.Uses.FirstOrDefault(use => use.Capability == capability)?.Problem;
        var note = notOffered ? UseNote.NotOffered
            : problem is { } failed ? ProblemNote(capability, failed)
            : last ? UseNote.Last
            : withoutContacts ? UseNote.NeedsContacts
            : withoutMail ? UseNote.NeedsMail
            : state == CapabilityState.NeedsPermission ? UseNote.Withheld
            : UseNote.None;
        return new UseSwitch(
            Active: state == CapabilityState.On,
            Asks: state == CapabilityState.NeedsPermission,
            Enabled: !last && !withoutContacts && !withoutMail && !notOffered,
            Note: note);
    }

    /// <summary>
    /// The pickers an account's page offers: one per slot it can hold, which is a slot with
    /// candidates or one already linked. A link the candidates no longer offer is kept as the
    /// first option, so the picker shows what is stored rather than "none".
    /// </summary>
    internal static IReadOnlyList<LinkPicker> LinkPickers(AccountEntry entry)
    {
        var slots = new (LinkSlot Slot, LinkedAccount[] Candidates, LinkedAccount? Current)[]
        {
            (LinkSlot.Calendar, entry.LinkCandidates.Calendar, entry.Links.Calendar),
            (LinkSlot.Contacts, entry.LinkCandidates.Contacts, entry.Links.Contacts),
            (LinkSlot.Mail, entry.LinkCandidates.Mail, entry.Links.Mail),
        };
        var pickers = new List<LinkPicker>();
        foreach (var (slot, candidates, current) in slots)
        {
            if (candidates.Length == 0 && current is null)
            {
                continue;
            }
            var options = candidates.ToList();
            if (current is not null && !options.Contains(current))
            {
                options.Insert(0, current);
            }
            int? selected = current is null ? null : options.IndexOf(current);
            int? suggested = null;
            if (selected is null)
            {
                var at = options.FindIndex(option => entry.LinkCandidates.Suggested.Contains(option.Id));
                suggested = at < 0 ? null : at;
            }
            pickers.Add(new LinkPicker(slot, options, selected, suggested));
        }
        return pickers;
    }

    /// <summary>The accounts that lose their link when this one is removed, joined for the
    /// confirmation; null when none does.</summary>
    internal static string? Unlinked(AccountEntry entry, AccountNameWords words) =>
        entry.LinkedFrom.Length == 0
            ? null
            : string.Join(", ", entry.LinkedFrom.Select(linked => AccountNames.Of(linked, words)));

    /// <summary>What switching a use on or off answered.</summary>
    internal static UseChangeOutcome Outcome(CapabilityChange change) => change switch
    {
        CapabilityChange.NeedsConsent => UseChangeOutcome.AskProvider,
        CapabilityChange.NeedsEndpoint => UseChangeOutcome.NeedsEndpoint,
        _ => UseChangeOutcome.Applied,
    };

    /// <summary>
    /// What the page compares to decide whether a core signal changed anything on it: every field
    /// of every entry, which accounts' sign-in has expired, and <paramref name="drawnBeside"/>,
    /// what else the page draws from other calls (which accounts have a mailbox listed, how each
    /// is shared). A signal that changed none of it leaves the page, and whatever is being typed
    /// on it, alone.
    /// </summary>
    internal static string Fingerprint(
        AccountsSnapshot snapshot, IEnumerable<string> expired, IEnumerable<string>? drawnBeside = null)
    {
        var parts = new List<string>();
        foreach (var entry in snapshot.Accounts)
        {
            parts.Add(entry.Id);
            parts.Add(entry.Address);
            parts.Add(entry.Kind.ToString());
            parts.Add(entry.SignIn.ToString());
            parts.AddRange(entry.Uses.Select(use => $"{use.Capability}={use.State}/{use.Problem}"));
            parts.Add(Linked(entry.Links.Calendar));
            parts.Add(Linked(entry.Links.Contacts));
            parts.Add(Linked(entry.Links.Mail));
            parts.AddRange(entry.LinkedFrom.Select(Linked));
            parts.AddRange(entry.LinkCandidates.Calendar.Select(Linked));
            parts.AddRange(entry.LinkCandidates.Contacts.Select(Linked));
            parts.AddRange(entry.LinkCandidates.Mail.Select(Linked));
            parts.AddRange(entry.LinkCandidates.Suggested);
            parts.Add(entry.Endpoints is { } e
                ? $"{e.ImapHost}|{e.ImapSecurity}|{e.SmtpHost}|{e.SmtpSecurity}|{e.CaldavUrl}|{e.CarddavUrl}|{e.Username}"
                : "-");
            parts.Add(entry.SeparateServer is { } separate
                ? $"{string.Join(',', separate.Uses)}|{separate.Host}|{separate.Username}"
                : "-");
            parts.Add(Distinction(entry.Distinction));
        }
        parts.Add("expired");
        parts.AddRange(expired.Order(StringComparer.Ordinal));
        parts.Add("beside");
        parts.AddRange((drawnBeside ?? []).Order(StringComparer.Ordinal));
        return string.Join('\n', parts);
    }

    private static string Linked(LinkedAccount? linked) =>
        linked is null ? "-" : $"{linked.Id}={linked.Address}/{Distinction(linked.Distinction)}";

    private static string Distinction(AccountDistinction? distinction) => distinction switch
    {
        AccountDistinction.Kind kind => $"kind:{kind.KindValue}",
        AccountDistinction.Server server => $"server:{server.Host}",
        _ => "-",
    };
}
