// The calendar-and-contacts route: a server that holds a calendar or address book and no mail
// (docs/accounts.md, docs/account-autodetect.md rule 8). Detection reaches it for a domain with a
// calendar or address-book server and no mail server, signed in with the typed address; the manual
// form reaches it as its own account type, with the servers typed. Either way it is a standards
// account without a mailbox, connected through the same password route as IMAP.

using Allodia.Mailcal;
using Allodia.Mailcal.Services;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using uniffi.mailcal_bindings;

namespace Allodia.Mailcal.Views;

public sealed partial class AccountSetupView
{
    private void OnDavFieldChanged(object sender, TextChangedEventArgs e) => UpdateCanConnect();

    /// <summary>
    /// Lays the section out for the found card or the manual form. On the card the uses panel
    /// says which servers are wanted and the login is the address typed; on the form the login and
    /// both servers are the person's to type.
    /// </summary>
    private void ShowDavSection(bool detected)
    {
        DavNote.Text = detected ? L10n.SetupDetectDavNote() : L10n.SetupDavNote();
        var manual = detected ? Visibility.Collapsed : Visibility.Visible;
        DavLogin.Visibility = manual;
        DavCaldavUrl.Visibility = manual;
        DavCarddavUrl.Visibility = manual;
    }

    private void ConnectDav(RejectedCertificate? acceptedCertificate)
    {
        string caldav;
        string carddav;
        AccountCapability[]? uses;
        if (_detectedTab == DetectTab.Dav)
        {
            if (FlagMissingServer())
            {
                return;
            }
            var chosen = ChosenUses();
            (caldav, carddav, uses) = (chosen.CaldavUrl, chosen.CarddavUrl, chosen.Uses);
        }
        else
        {
            (caldav, carddav) = (DavCaldavUrl.Text.Trim(), DavCarddavUrl.Text.Trim());
            uses = SetupUses.DavUses(caldav, carddav);
            if (uses is null)
            {
                // No server typed is no account: the calendar's is the one to start with.
                Flag(DavCaldavUrl);
                return;
            }
        }
        Model?.SubmitSetup(
            imapHost: string.Empty,
            username: DavLogin.Text.Trim(),
            password: DavPassword.Password,
            smtpHost: string.Empty,
            caldavUrl: caldav,
            acceptedCertificate: acceptedCertificate,
            carddavUrl: carddav,
            uses: uses);
    }
}
