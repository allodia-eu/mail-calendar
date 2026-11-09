// What Settings → Accounts' list says about an account, decided from its snapshot entry alone
// (docs/accounts.md rules 11 to 13), and what an expired sign-in and a refused duplicate offer.
// An account's page, its Sign-in section included (rule 17), is the core's (`account_page`) and
// carries no rule here. WinUI-free and L10n-free on purpose, so each
// rule is reachable from Mailcal.Tests: the words are the dialog's, the decisions are here.

using uniffi.mailcal_bindings;

namespace Allodia.Mailcal.Services;

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
    /// <summary>A calendar and contacts account a Nextcloud sign-in made, signed in with
    /// Nextcloud again (docs/nextcloud-sign-in.md rule 8).</summary>
    Nextcloud,
}

/// <summary>The account an add was refused for, and the config the add was given, which the
/// refusal offers to put on it (docs/accounts.md rule 17).</summary>
/// <param name="Unreached">The calendar and contacts the sign-in that gave the config said it would
/// not reach, which taking the offer asks about first (rule 18).</param>
/// <param name="NextcloudGrant">In place of a config, the grant of a Nextcloud sign-in as the
/// person that account is (docs/nextcloud-sign-in.md rule 7).</param>
internal sealed record ExistingAccountOffer(
    string AccountId, string ConfigToml, IReadOnlyList<AccountCapability> Unreached, string? NextcloudGrant = null)
{
    /// <summary>Never the config or the grant: each carries a credential.</summary>
    public override string ToString() => $"ExistingAccountOffer({AccountId})";
}

internal static class AccountSettingsRules
{
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

    /// <summary>What the expired-sign-in banner runs for an account of <paramref name="provider"/>,
    /// or for one a Nextcloud sign-in made (<paramref name="nextcloud"/>), which reports a provider
    /// as well. Null, a provider the core could not name, has nothing to run.</summary>
    internal static SignInRemedy Remedy(AccountProvider? provider, bool nextcloud = false) =>
        nextcloud ? SignInRemedy.Nextcloud : provider switch
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

    /// <summary>The accounts that lose their link when this one is removed, joined for the
    /// confirmation; null when none does.</summary>
    internal static string? Unlinked(AccountEntry entry, AccountNameWords words) =>
        entry.LinkedFrom.Length == 0
            ? null
            : string.Join(", ", entry.LinkedFrom.Select(linked => AccountNames.Of(linked, words)));

    /// <summary>
    /// What the page compares to decide whether a core signal changed anything on it: every field
    /// of every entry, and <paramref name="drawnBeside"/>,
    /// what else the page draws from other calls (which accounts have a mailbox listed, how each
    /// is shared). A signal that changed none of it leaves the page, and whatever is being typed
    /// on it, alone.
    /// </summary>
    internal static string Fingerprint(AccountsSnapshot snapshot, IEnumerable<string>? drawnBeside = null)
    {
        var parts = new List<string>();
        foreach (var entry in snapshot.Accounts)
        {
            parts.Add(entry.Id);
            parts.Add(entry.Address);
            parts.Add(entry.Kind.ToString());
            parts.Add(entry.SignIn.ToString());
            parts.Add(entry.Nextcloud ? "nextcloud" : "-");
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
        parts.Add("beside");
        parts.AddRange((drawnBeside ?? []).Order(StringComparer.Ordinal));
        return string.Join('\n', parts);
    }

    /// <summary>What the page draws from the core's record of it, for <see cref="Fingerprint"/>'s
    /// <c>drawnBeside</c>: every control's value and state, and the notice. A change starting or
    /// ending redraws the page, since its controls hold and spin from this alone.</summary>
    internal static IEnumerable<string> PageFingerprint(AccountPage? page)
    {
        if (page is null)
        {
            yield break;
        }
        yield return $"page:{page.Account}";
        foreach (var use in page.Uses)
        {
            yield return $"use:{use.Capability}={use.On}/{Control(use.Control)}/{use.Blocked}/{use.Status}/"
                + $"{use.AsksProvider}/{use.ConfirmsOff}/{use.ReviewsServerChange}";
        }
        foreach (var picker in page.Links)
        {
            yield return $"link:{picker.Slot}={picker.Selected}/{picker.Suggested}/{Control(picker.Control)}/"
                + string.Join(',', picker.Options.Select(Linked));
        }
        if (page.SenderName is { } name)
        {
            yield return $"name:{name.Name}/{name.Editable}/{Control(name.Control)}";
        }
        if (page.Mail is { } mail)
        {
            yield return $"depth:{Count(mail.Depth)}";
            yield return $"size:{Count(mail.Size)}";
            yield return $"push:{mail.Push?.On}/{(mail.Push is { } push ? Control(push.Control) : "")}";
            yield return $"interval:{(mail.Interval is { } interval ? Count(interval) : "")}";
            foreach (var folder in mail.Folders)
            {
                yield return $"folder:{folder.Key}={folder.Name}/{folder.Role}/{folder.On}/{Control(folder.Control)}";
            }
        }
        if (page.Servers is { } servers)
        {
            // The form, password included: a refused edit is drawn as it was typed.
            var form = servers.Form;
            yield return string.Join('/', "servers", form.ImapHost, form.ImapSecurity, form.SmtpHost,
                form.SmtpSecurity, form.CaldavUrl, form.CarddavUrl, form.Username, form.Password,
                Control(servers.Save), servers.SignInRefused);
        }
        if (page.SignIn is { } signIn)
        {
            yield return $"sign-in:{signIn.Line}/{Control(signIn.Control)}/{signIn.Switches}/"
                + string.Join(',', signIn.Routes.Select(route => route switch
                {
                    AccountSignInRoute.ToProvider provider => $"provider={provider.Label}",
                    _ => route.GetType().Name,
                }));
            if (signIn.Password is { } field)
            {
                yield return $"password:{field.Kind}/{field.Replaces}/{Control(field.Save)}";
            }
        }
        if (page.Shared is { } shared)
        {
            foreach (var mailbox in shared.Mailboxes)
            {
                yield return $"shared:{mailbox.Handle}={mailbox.Title}/{mailbox.Label}/{mailbox.Withdrawn}/{mailbox.On}/"
                    + Control(mailbox.Control);
            }
            yield return $"shared-note:{shared.Note}";
            if (shared.Address is { } address)
            {
                yield return $"shared-address:{address.Text}/{Control(address.Add)}";
            }
        }
        if (page.ReplyFallback is { } replies)
        {
            yield return $"replies:{replies.Choice}/{Control(replies.Control)}";
        }
        yield return $"remove:{Control(page.Remove)}";
        yield return $"notice:{page.Notice?.Id}";
    }

    private static string Control(ControlState control) => $"{control.Enabled}|{control.Running}";

    private static string Count(AccountCountChoice choice) =>
        $"{string.Join(',', choice.Options)}@{choice.Selected}/{Control(choice.Control)}";

    private static string Linked(LinkedAccount? linked) =>
        linked is null ? "-" : $"{linked.Id}={linked.Address}/{Distinction(linked.Distinction)}";

    private static string Distinction(AccountDistinction? distinction) => distinction switch
    {
        AccountDistinction.Kind kind => $"kind:{kind.KindValue}",
        AccountDistinction.Server server => $"server:{server.Host}",
        _ => "-",
    };
}
