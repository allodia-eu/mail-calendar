// What the account is used for, chosen on the setup form: mail, calendar and contacts on the IMAP
// and calendar-and-contacts cards, and colleagues too before a Microsoft or Google sign-in
// (docs/account-autodetect.md rule 8, docs/accounts.md rule 10). Which uses are offered and how
// each starts is the core's; what the toggles make of the account is SetupUses, which the unit
// suite drives. This partial draws the toggles and reads them back.

using Allodia.Mailcal;
using Allodia.Mailcal.Services;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Automation;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Media;
using uniffi.mailcal_bindings;

namespace Allodia.Mailcal.Views;

public sealed partial class AccountSetupView
{
    // The route detection took, and what its card offered; null on the manual form.
    private DetectTab? _detectedTab;
    private UseOffer _detectedUses = UseOffer.None;

    // What the panel is drawn for, and its toggles.
    private UseOffer _uses = UseOffer.None;
    private CheckBox? _mailUse;
    private CheckBox? _calendarUse;
    private CheckBox? _contactsUse;
    private CheckBox? _colleaguesUse;
    private TextBox? _calendarTyped;
    private TextBox? _contactsTyped;

    /// <summary>
    /// Draws the choices the route on screen offers: the detected card's own, a provider's for the
    /// address typed when the manual form chose Microsoft or Google, and none otherwise.
    /// </summary>
    private void RefreshUses(DetectTab tab)
    {
        var offer = tab switch
        {
            _ when _detectedTab == tab => _detectedUses,
            DetectTab.Microsoft => ProviderOffer(AccountKind.Microsoft),
            DetectTab.Google => ProviderOffer(AccountKind.Google),
            _ => UseOffer.None,
        };
        DrawUses(offer);
        // The servers the panel decides are not typed beside it, and the mail servers follow the
        // mail toggle.
        var choosing = offer.Choices.Count > 0;
        CaldavUrl.Visibility = choosing ? Visibility.Collapsed : Visibility.Visible;
        CarddavUrl.Visibility = choosing ? Visibility.Collapsed : Visibility.Visible;
        ShowMailServers(_mailUse?.IsChecked != false);
    }

    private UseOffer ProviderOffer(AccountKind kind) =>
        new(MailcalBindingsMethods.ProviderSetupChoices(kind, DetectEmail.Text.Trim()), string.Empty, string.Empty);

    private void DrawUses(UseOffer offer)
    {
        _uses = offer;
        _mailUse = _calendarUse = _contactsUse = _colleaguesUse = null;
        _calendarTyped = _contactsTyped = null;
        UsesPanel.Children.Clear();
        UsesPanel.Visibility = offer.Choices.Count > 0 ? Visibility.Visible : Visibility.Collapsed;
        foreach (var choice in offer.Choices)
        {
            switch (choice.Capability)
            {
                case AccountCapability.Mail:
                    UsesPanel.Children.Add(Section(L10n.SetupDetectSectionEmail()));
                    _mailUse = UseBox(L10n.SetupDetectMailEnable(), choice.On, "SetupUseMail");
                    _mailUse.Checked += (_, _) => ShowMailServers(true);
                    _mailUse.Unchecked += (_, _) => ShowMailServers(false);
                    UsesPanel.Children.Add(_mailUse);
                    break;
                case AccountCapability.Calendar:
                    UsesPanel.Children.Add(Section(L10n.SetupDetectSectionCalendar()));
                    (_calendarUse, _calendarTyped) = DavUse(
                        choice, offer.CaldavUrl, L10n.SetupDetectCalendarEnable(), L10n.SetupDetectCalendarAdd(),
                        L10n.SetupHintCaldav(), "SetupUseCalendar");
                    break;
                case AccountCapability.Contacts:
                    UsesPanel.Children.Add(Section(L10n.SetupDetectSectionContacts()));
                    (_contactsUse, _contactsTyped) = DavUse(
                        choice, offer.CarddavUrl.Length > 0 ? offer.CarddavUrl : offer.CaldavUrl,
                        L10n.SetupDetectContactsEnable(), L10n.SetupDetectContactsAdd(),
                        L10n.SetupHintCarddav(), "SetupUseContacts");
                    break;
                case AccountCapability.Colleagues:
                    _colleaguesUse = UseBox(L10n.SetupDetectColleaguesEnable(), choice.On, "SetupUseColleagues");
                    _colleaguesUse.Margin = new Thickness(28, 0, 0, 0);
                    UsesPanel.Children.Add(_colleaguesUse);
                    FollowContacts();
                    break;
            }
        }
        KeepOneOn();
    }

    private static TextBlock Section(string text) => new()
    {
        Text = text,
        Style = (Style)Application.Current.Resources["BodyStrongTextBlockStyle"],
    };

    private CheckBox UseBox(string label, bool on, string automationId)
    {
        var box = new CheckBox { Content = label, IsChecked = on };
        AutomationProperties.SetAutomationId(box, automationId);
        box.Checked += (_, _) => UpdateCanConnect();
        box.Unchecked += (_, _) => UpdateCanConnect();
        return box;
    }

    /// <summary>
    /// A calendar or contacts toggle, and beneath it, following it so a use switched off leaves
    /// nothing behind claiming otherwise: the server detection found, by host, or a field for one
    /// when it found none. A provider's sign-in covers the use with no server to name.
    /// </summary>
    private (CheckBox, TextBox?) DavUse(
        SetupChoice choice, string found, string enable, string add, string hint, string automationId)
    {
        var box = UseBox(choice.ServerFound ? enable : add, choice.On, automationId);
        UsesPanel.Children.Add(box);
        FrameworkElement? detail = null;
        TextBox? typed = null;
        if (!choice.ServerFound)
        {
            typed = new TextBox { PlaceholderText = hint };
            AutomationProperties.SetName(typed, add);
            AutomationProperties.SetAutomationId(typed, automationId + "Server");
            detail = typed;
        }
        else if (found.Length > 0)
        {
            detail = new TextBlock
            {
                Text = SetupUses.Host(found),
                Style = (Style)Application.Current.Resources["CaptionTextBlockStyle"],
                Foreground = (Brush)Application.Current.Resources["TextFillColorSecondaryBrush"],
                Margin = new Thickness(28, 0, 0, 0),
            };
        }
        if (detail is not null)
        {
            UsesPanel.Children.Add(detail);
            void Follow() => detail.Visibility = box.IsChecked == true ? Visibility.Visible : Visibility.Collapsed;
            Follow();
            box.Checked += (_, _) => Follow();
            box.Unchecked += (_, _) => Follow();
        }
        return (box, typed);
    }

    /// <summary>Colleagues come from the organisation's directory beside the person's own
    /// contacts, so they are offered only while contacts are on, as on the account's page.</summary>
    private void FollowContacts()
    {
        if (_contactsUse is not { } contacts || _colleaguesUse is not { } colleagues)
        {
            return;
        }
        colleagues.IsEnabled = contacts.IsChecked == true;
        contacts.Checked += (_, _) => colleagues.IsEnabled = true;
        contacts.Unchecked += (_, _) =>
        {
            colleagues.IsChecked = false;
            colleagues.IsEnabled = false;
        };
    }

    /// <summary>The last use switched on cannot be switched off: an account used for nothing is
    /// not one.</summary>
    private void KeepOneOn()
    {
        var all = new[] { _mailUse, _calendarUse, _contactsUse }.OfType<CheckBox>().ToList();
        void Settle()
        {
            var on = all.Count(box => box.IsChecked == true);
            foreach (var box in all)
            {
                box.IsEnabled = on > 1 || box.IsChecked != true;
            }
        }
        Settle();
        foreach (var box in all)
        {
            box.Checked += (_, _) => Settle();
            box.Unchecked += (_, _) => Settle();
        }
    }

    /// <summary>The mail servers are on screen while the account is to be used for mail.</summary>
    private void ShowMailServers(bool shown)
    {
        var visibility = shown ? Visibility.Visible : Visibility.Collapsed;
        ImapServerRow.Visibility = visibility;
        AdvancedHeading.Visibility = visibility;
        SmtpServerRow.Visibility = visibility;
        PortNote.Visibility = visibility;
    }

    private static PickedUse? Picked(CheckBox? box, TextBox? typed) =>
        box is null ? null : new PickedUse(box.IsChecked == true, typed?.Text ?? string.Empty);

    /// <summary>What the toggles on screen make of the account.</summary>
    private ChosenUses ChosenUses() => SetupUses.Chosen(
        _uses,
        _mailUse?.IsChecked,
        Picked(_calendarUse, _calendarTyped),
        Picked(_contactsUse, _contactsTyped),
        _colleaguesUse?.IsChecked);

    /// <summary>Points at a server still to be typed for a use switched on, and says whether there
    /// was one; nothing can be set up until it is filled in.</summary>
    private bool FlagMissingServer()
    {
        var field = SetupUses.Missing(
                _uses, Picked(_calendarUse, _calendarTyped), Picked(_contactsUse, _contactsTyped)) switch
        {
            MissingServer.Calendar => _calendarTyped,
            MissingServer.Contacts => _contactsTyped,
            _ => null,
        };
        if (field is null)
        {
            return false;
        }
        Flag(field);
        return true;
    }

    /// <summary>Marks <paramref name="field"/> as the one still to fill in and moves to it; typing
    /// in it clears the mark.</summary>
    private static void Flag(TextBox field)
    {
        field.BorderBrush = (Brush)Application.Current.Resources["SystemFillColorCriticalBrush"];
        AutomationProperties.SetIsRequiredForForm(field, true);
        field.TextChanged += (_, _) => field.ClearValue(Control.BorderBrushProperty);
        field.Focus(FocusState.Programmatic);
    }

    /// <summary>
    /// What a Microsoft or Google sign-in asks for: the uses chosen, less any the address it signs
    /// in with is not offered, so a personal address is never asked for colleagues. Null where
    /// the form offered no choice, which asks for everything.
    /// </summary>
    private AccountCapability[]? ProviderUses(AccountKind kind)
    {
        var chosen = _uses.Choices.Count > 0 ? ChosenUses().Uses : null;
        return SetupUses.Allowed(
            chosen, MailcalBindingsMethods.ProviderSetupChoices(kind, DetectEmail.Text.Trim()));
    }
}
