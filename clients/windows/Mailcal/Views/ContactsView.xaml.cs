// The contacts surface's wiring: the search field, the list selection, and opening one person.
//
// Deliberately thin. Everything that can be *wrong* about this screen, which row starts an A–Z
// section, whether a row must disclose that it is a merge, which of the two empty states applies,
// and how an account id becomes an address the user recognises, lives in Services/ContactSections.cs
// and is tested headlessly in Mailcal.Tests. What is here is wiring.
//
// Opening a person is awaited OFF the UI thread (MailboxModel.OpenContactAsync): `contact_detail` is
// network-free but blocks on the core's runtime and lands on the store's connection thread, so a
// click made while a sync holds that connection would otherwise stall the window.

using Allodia.Mailcal.Services;
using Allodia.Mailcal.ViewModels;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;

namespace Allodia.Mailcal.Views;

/// <summary>The contacts detail view: the A–Z people list beside one person's detail.</summary>
public sealed partial class ContactsView : UserControl
{
    /// <summary>The row the user picked, so the highlight survives a snapshot reconcile.</summary>
    private string? _selectedRowId;

    /// <summary>Suppresses the search dispatch while the field is being cleared programmatically.</summary>
    private bool _settingSearchText;

    /// <summary>The shared app model (set by the host via <see cref="Init"/>).</summary>
    public MailboxModel? Model { get; private set; }

    /// <summary>Initialises the control.</summary>
    public ContactsView() => this.InitializeComponent();

    /// <summary>Binds the view to the shared model.</summary>
    public void Init(MailboxModel model)
    {
        Model = model;
        // A refresh replaces row objects in place, which drops the ListView's selection; re-apply it
        // from the id the user actually picked, so a background address-book sync doesn't unhighlight
        // the person whose detail is on screen beside it.
        model.Contacts.CollectionChanged += (_, _) => RestoreSelection();
    }

    /// <summary>
    /// Called as Contacts comes on screen: empties the search field.
    /// </summary>
    /// <remarks>
    /// The other half of the same action, clearing the query in the <b>core</b>, is
    /// <see cref="MailboxModel.ShowContacts"/>'s. Both are needed: the field is view state, the query
    /// is the core's, and either left behind is a narrowing the user can no longer see. The dispatch
    /// is suppressed here so exactly one clear reaches the core.
    /// </remarks>
    public void OnShown()
    {
        _settingSearchText = true;
        SearchBox.Text = string.Empty;
        _settingSearchText = false;
        ClearSearchButton.Visibility = Visibility.Collapsed;
    }

    private void OnSearchChanged(object sender, TextChangedEventArgs e)
    {
        ClearSearchButton.Visibility =
            SearchBox.Text.Length == 0 ? Visibility.Collapsed : Visibility.Visible;
        if (_settingSearchText)
        {
            return;
        }
        // Into the CORE, not a filter over the loaded rows: the core matches name, email, phone,
        // organisation and title, so every client narrows identically and a person beyond the page
        // cap is still findable.
        Model?.SearchContacts(SearchBox.Text);
    }

    private void OnClearSearch(object sender, RoutedEventArgs e) => SearchBox.Text = string.Empty;

    private void OnPersonSelected(object sender, SelectionChangedEventArgs e)
    {
        if (PeopleList.SelectedItem is not ContactItem person)
        {
            return;
        }
        _selectedRowId = person.Id;
        // Fire-and-forget: the lookup hops off the UI thread and settles the model's OpenedContact,
        // which the detail pane binds to.
        _ = Model?.OpenContactAsync(person.Id);
    }

    private void RestoreSelection()
    {
        if (Model is null)
        {
            return;
        }
        // The row may be gone, searched away, or merged into another person. The detail stays up
        // either way: the core keeps retired ids pointing at the survivor, so what is on screen is
        // still a real person, and blanking it would look like the click had failed.
        foreach (var person in Model.Contacts)
        {
            if (person.Id == _selectedRowId)
            {
                PeopleList.SelectedItem = person;
                return;
            }
        }
    }
}
