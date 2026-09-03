// The contacts half of MailboxModel (split out to keep each file under the 500-line limit): the
// list snapshot, one person's detail, the search query, and the composer's recipient autosuggest.
//
// The list is an ordinary pushed snapshot (`Surface.Contacts` -> `ContactList()`), pulled on the UI
// thread like the mailbox and the agenda. **The other two are not.** `ContactDetail` and
// `RecipientSuggestions` are direct queries that block the calling thread on the core's runtime and
// land on the store's connection thread, so a call made while a sync holds that connection waits
// for it. The FFI's own module doc says as much: a host must keep them off its UI thread. They are
// therefore awaited off it here, and the per-keystroke one is debounced at its call site
// (Views/RecipientField.xaml.cs) rather than fired per character.
//
// Search runs in the CORE, not as a filter over the rows already on screen: the core matches name,
// email, phone, organisation and title, so every client narrows identically, and a person beyond
// the 200-row page cap is still findable.
//
// Nothing here logs contact content. Row counts, id-less durations and a query's *length* are
// diagnostics; a name, address, phone number or organisation is not (docs/logging.md).

using System.Collections.Generic;
using System.Collections.ObjectModel;
using System.Linq;
using System.Threading.Tasks;
using Allodia.Mailcal.ViewModels;
using Microsoft.UI.Xaml;
using uniffi.mailcal_bindings;

namespace Allodia.Mailcal.Services;

public sealed partial class MailboxModel
{
    /// <summary>The unified people the contacts list shows, A–Z, reflecting the active search.</summary>
    public ObservableCollection<ContactItem> Contacts { get; } = new();

    private string _contactQuery = string.Empty;

    /// <summary>
    /// The active contacts search text. Held here rather than only in the search box because the
    /// query lives in the <em>core</em>: the two empty states turn on it, and clearing it has to
    /// reset both halves as one action.
    /// </summary>
    public string ContactQuery
    {
        get => _contactQuery;
        private set
        {
            if (Set(ref _contactQuery, value))
            {
                RaiseContactsListState();
            }
        }
    }

    private ContactDetailItem? _openedContact;

    /// <summary>
    /// The person shown in the contacts detail pane, or <c>null</c> when none is picked (the pane
    /// shows its placeholder).
    /// </summary>
    public ContactDetailItem? OpenedContact
    {
        get => _openedContact;
        private set
        {
            if (Set(ref _openedContact, value))
            {
                Raise(nameof(ContactDetailVisibility));
                Raise(nameof(ContactPlaceholderVisibility));
            }
        }
    }

    /// <summary>Show the detail pane once a person is picked.</summary>
    public Visibility ContactDetailVisibility =>
        OpenedContact is null ? Visibility.Collapsed : Visibility.Visible;

    /// <summary>Show the detail pane's placeholder until then, so the column is never blank.</summary>
    public Visibility ContactPlaceholderVisibility =>
        OpenedContact is null ? Visibility.Visible : Visibility.Collapsed;

    /// <summary>Show the people list only when there is somebody in it.</summary>
    public Visibility ContactsListVisibility =>
        ContactsState == ContactsListState.Rows ? Visibility.Visible : Visibility.Collapsed;

    /// <summary>Show an empty state when there is not.</summary>
    public Visibility ContactsEmptyVisibility =>
        ContactsState == ContactsListState.Rows ? Visibility.Collapsed : Visibility.Visible;

    /// <summary>
    /// The empty state's headline. The two states are deliberately different sentences: telling
    /// someone who has just searched "No contacts yet" reads as though theirs had vanished.
    /// </summary>
    public string ContactsEmptyTitle =>
        ContactsState == ContactsListState.NoResults ? L10n.ContactsNoResults() : L10n.ContactsEmpty();

    /// <summary>The "they'll appear here once synced" line, only for the nothing-synced-yet state.</summary>
    public string ContactsEmptyBody => L10n.ContactsEmptyBody();

    /// <summary>Hide the explanatory line under a no-results headline, which needs none.</summary>
    public Visibility ContactsEmptyBodyVisibility =>
        ContactsState == ContactsListState.NoContacts ? Visibility.Visible : Visibility.Collapsed;

    private ContactsListState ContactsState => ContactSections.ListState(Contacts.Count, ContactQuery);

    private void RaiseContactsListState()
    {
        Raise(nameof(ContactsListVisibility));
        Raise(nameof(ContactsEmptyVisibility));
        Raise(nameof(ContactsEmptyTitle));
        Raise(nameof(ContactsEmptyBodyVisibility));
    }

    /// <summary>
    /// Switch to Contacts: clear any stale search, paint what is already cached, then sync.
    /// </summary>
    /// <remarks>
    /// The query is cleared in the <b>core</b> first. The search box is view state that dies with
    /// the view, but the query lives in the core, so without this, leaving Contacts mid-search and
    /// coming back shows a filtered list under an empty search box: a narrowing the user can no
    /// longer see, which is the failure docs/search.md exists to prevent.
    /// </remarks>
    public void ShowContacts()
    {
        Destination = AppDestination.Contacts;
        ContactQuery = string.Empty;
        _app?.Dispatch(new Intent.SearchContacts(string.Empty));
        // Paint the cached list immediately, then sync, so switching destinations never shows an
        // empty screen while the address books are consulted.
        PullContacts();
        Log.Info("contacts refresh requested");
        _app?.Dispatch(new Intent.RefreshContacts());
    }

    /// <summary>
    /// Narrows the contacts list. The core answers with a <c>Surface.Contacts</c> signal; an empty
    /// query resets it to the whole list.
    /// </summary>
    public void SearchContacts(string query)
    {
        ContactQuery = query;
        // The query is a name or an address the user is looking for, content. Only its length is
        // diagnostic (docs/logging.md).
        Log.Info($"contacts search: {query.Length} char(s)");
        _app?.Dispatch(new Intent.SearchContacts(query));
    }

    /// <summary>Clears the contacts search in the core and in this model, as one action.</summary>
    public void ClearContactSearch() => SearchContacts(string.Empty);

    /// <summary>Empties the contacts detail pane (nothing picked).</summary>
    public void CloseContact() => OpenedContact = null;

    /// <summary>
    /// Opens one person's detail, by the id a row carries.
    /// </summary>
    /// <remarks>
    /// Awaited off the UI thread: the call is network-free but blocks on the core's runtime and
    /// reaches the store's connection thread, so one made while a sync holds that connection waits
    /// for it. A <c>null</c> answer means the person is genuinely gone, never merely renumbered:
    /// merging retires ids and the core keeps the retired ones pointing at the survivor, so a row
    /// still held after a background sync merged it opens fine without refreshing the list first.
    /// </remarks>
    public async Task OpenContactAsync(string id)
    {
        if (_app is null)
        {
            return;
        }
        var app = _app;
        var labels = ContactAccountLabels();
        try
        {
            var detail = await Task.Run(() => app.ContactDetail(id));
            if (detail is null)
            {
                Log.Info("contact detail: person no longer exists");
                OpenedContact = null;
                return;
            }
            OpenedContact = BuildDetail(detail, labels);
        }
        catch (Exception ex)
        {
            Log.Warn($"contact detail lookup failed: {ex.GetType().Name}");
        }
    }

    /// <summary>
    /// Ranked address suggestions for a partially-typed recipient, off the UI thread.
    /// </summary>
    /// <remarks>
    /// Draws on synced contacts <b>and</b> on people the user has written to before (the engine
    /// mines the Sent mailbox), so it is useful on an account with no address book at all, which is
    /// most accounts. A blank query returns nothing: a dropdown of everyone you have ever emailed,
    /// the moment To takes focus, is noise rather than help.
    /// </remarks>
    internal async Task<IReadOnlyList<RecipientMatch>> RecipientSuggestionsAsync(string query)
    {
        if (_app is null || string.IsNullOrWhiteSpace(query))
        {
            return [];
        }
        var app = _app;
        try
        {
            return await Task.Run(() => app.RecipientSuggestions(query));
        }
        catch (Exception ex)
        {
            Log.Warn($"recipient suggestions failed: {ex.GetType().Name}");
            return [];
        }
    }

    /// <summary>
    /// Account id → the address the user knows that account by, for the detail pane's provenance
    /// labels.
    /// </summary>
    private Dictionary<string, string> ContactAccountLabels()
    {
        var labels = new Dictionary<string, string>();
        foreach (var account in Accounts)
        {
            labels.TryAdd(account.Id, account.Email);
        }
        return labels;
    }

    /// <summary>Pulls the contacts snapshot from the core (on a <c>Surface.Contacts</c> signal).</summary>
    private void PullContacts()
    {
        if (_app is null)
        {
            return;
        }
        var rows = _app.ContactList().Rows;
        var items = new List<ContactItem>(rows.Length);
        for (var index = 0; index < rows.Length; index++)
        {
            var row = rows[index];
            items.Add(new ContactItem
            {
                Id = row.Id,
                // A card may legitimately carry an address and no name. The core leaves the name
                // EMPTY rather than filling in English text a Dutch reader would be stuck with,
                // supplying the placeholder is the client's job (docs/contacts.md §2).
                DisplayName = string.IsNullOrEmpty(row.DisplayName) ? L10n.ContactsNoName() : row.DisplayName,
                Email = row.PrimaryEmail,
                Avatar = AvatarItem.From(row.Avatar),
                SectionHeader = ContactSections.HeaderFor(rows, index) ?? string.Empty,
                AccountCountText = ContactSections.DisclosesAccounts(row.AccountCount)
                    ? L10n.ContactsInAccounts((int)row.AccountCount)
                    : string.Empty,
            });
        }
        Reconcile(Contacts, items, item => item.Id, SameContact);
        RaiseContactsListState();
        // The list is a snapshot and the detail is a pull, so refreshing the rows leaves the pane
        // beside them showing what the person held before the save that published this. Off the
        // UI thread like every other detail read; a person whose last card has gone reads as null
        // and closes the pane.
        if (OpenedContact is { } open)
        {
            _ = OpenContactAsync(open.Id);
        }
        Log.Info($"contacts: {Contacts.Count} row(s)");
    }

    // The avatar is part of the comparison because a photo arrives in a LATER snapshot than the
    // row (docs/avatars.md): left out, every other field matches, the reconcile keeps the row it
    // already had, and a face that exists is never drawn.
    private static bool SameContact(ContactItem a, ContactItem b) =>
        a.DisplayName == b.DisplayName && a.Email == b.Email
        && a.SectionHeader == b.SectionHeader && a.AccountCountText == b.AccountCountText
        && a.Avatar == b.Avatar;

    /// <summary>Projects one person's FFI detail into the render-ready view model.</summary>
    private static ContactDetailItem BuildDetail(
        ContactDetail detail,
        IReadOnlyDictionary<string, string> labels)
    {
        // Suppressed for a single-account person: repeating the same account name down the screen
        // disambiguates nothing.
        var spans = ContactSections.SpansSeveralAccounts(detail);
        var groups = new List<ContactValueGroup>();
        AddGroup(groups, L10n.ContactsSectionEmails(), detail.Emails, labels, spans);
        AddGroup(groups, L10n.ContactsSectionPhones(), detail.Phones, labels, spans);
        AddGroup(groups, L10n.ContactsSectionOrganizations(), detail.Organizations, labels, spans);
        AddGroup(groups, L10n.ContactsSectionTitles(), detail.Titles, labels, spans);
        return new ContactDetailItem
        {
            Id = detail.Id,
            DisplayName = string.IsNullOrEmpty(detail.DisplayName)
                ? L10n.ContactsNoName()
                : detail.DisplayName,
            Avatar = AvatarItem.From(detail.Avatar),
            Groups = groups,
            // "Also in" is the *explanation* of the list row's "In 2 accounts" badge, so it exists
            // only where that badge does.
            Accounts = spans
                ? [.. detail.Accounts.Select(id => ContactSections.AccountLabel(id, labels))]
                : [],
            EditableCards = detail.EditableCards,
        };
    }

    private static void AddGroup(
        List<ContactValueGroup> groups,
        string heading,
        ContactValue[] values,
        IReadOnlyDictionary<string, string> labels,
        bool spansSeveralAccounts)
    {
        if (values.Length == 0)
        {
            return;
        }
        groups.Add(new ContactValueGroup
        {
            Heading = heading,
            Values = [.. values.Select(value => new ContactValueItem
            {
                Value = value.Value,
                AccountsText = spansSeveralAccounts
                    ? ContactSections.AccountLabels(value.Accounts, labels)
                    : string.Empty,
            })],
        });
    }
}
