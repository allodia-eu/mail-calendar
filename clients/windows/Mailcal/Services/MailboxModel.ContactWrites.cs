// Creating and editing a contact: the destinations a create may offer, the card an editor is
// seeded from, and the two writes themselves.
//
// Split from MailboxModel.Contacts.cs, which reads, and because this is the one place the rule
// docs/contacts.md §3 binds turns into control flow: **an edit names a card, never a person.** The
// list and the detail show people, which the core assembled from the cards several accounts hold,
// so an editor is seeded from ONE of those cards, chosen by the user when there is more than one,
// and never from the merged detail on screen.
//
// The reads here are network-free but not free: each blocks on the core's runtime and lands on the
// store's connection thread, so a call made while a sync holds it waits for it. Off the UI thread,
// therefore, exactly like ContactDetail and RecipientSuggestions next door.

using System;
using System.Collections.Generic;
using System.Threading.Tasks;
using Allodia.Mailcal.ViewModels;
using Microsoft.UI.Xaml;
using uniffi.mailcal_bindings;

namespace Allodia.Mailcal.Services;

public sealed partial class MailboxModel
{
    private IReadOnlyList<ContactBookChoice> _contactBooks = [];

    /// <summary>
    /// Whether there is anywhere at all to save a contact.
    /// </summary>
    /// <remarks>
    /// No writable address book, no create button: offering one produces a save that fails on the
    /// server after the user has typed everything in.
    /// </remarks>
    public bool CanCreateContact => _contactBooks.Count > 0;

    /// <summary>Show the "new contact" button only where a contact could actually be filed.</summary>
    public Visibility CreateContactVisibility =>
        CanCreateContact ? Visibility.Visible : Visibility.Collapsed;

    /// <summary>The writable address books, as the create form offers them.</summary>
    internal IReadOnlyList<ContactBookChoice> ContactBooks => _contactBooks;

    /// <summary>
    /// Reads the writable address books, off the UI thread.
    /// </summary>
    /// <remarks>
    /// Read on entering the surface rather than when the create button is pressed, because the
    /// answer decides whether that button exists at all.
    /// </remarks>
    public async Task LoadContactBooksAsync()
    {
        if (_app is null)
        {
            return;
        }
        var app = _app;
        var labels = ContactAccountLabels();
        try
        {
            var targets = await Task.Run(() => app.ContactTargets());
            _contactBooks = ContactEditing.Books(targets, labels);
            Raise(nameof(CanCreateContact));
            Raise(nameof(CreateContactVisibility));
            Log.Info($"contacts: {_contactBooks.Count} writable book(s)");
        }
        catch (Exception ex)
        {
            Log.Warn($"contact targets read failed: {ex.GetType().Name}");
        }
    }

    /// <summary>
    /// The editable values of one source card, for seeding an editor.
    /// </summary>
    /// <remarks>
    /// Read from the <b>card</b>, never from the person the detail pane showed: the person is a
    /// merge, so seeding an editor from it would offer another account's values for saving into
    /// this one's address book. <c>null</c> when the card has gone since the tap.
    /// </remarks>
    internal async Task<ContactEdit?> ContactCardAsync(string person, string account, string card)
    {
        if (_app is null)
        {
            return null;
        }
        var app = _app;
        try
        {
            return await Task.Run(() => app.ContactCard(person, account, card));
        }
        catch (Exception ex)
        {
            Log.Warn($"contact card read failed: {ex.GetType().Name}");
            return null;
        }
    }

    /// <summary>The cards the open person can be edited through, labelled by their accounts.</summary>
    internal IReadOnlyList<ContactCardChoice> EditableCardsOf(ContactDetailItem person) =>
        ContactEditing.Cards(person.EditableCards, ContactAccountLabels());

    /// <summary>
    /// Dispatches a contact write the editor already built and validated.
    /// </summary>
    /// <remarks>
    /// <c>internal</c>, like every member here whose signature names a generated FFI type: those
    /// are <c>internal</c>, and a public member carrying one does not compile.
    /// </remarks>
    internal void SaveContact(Intent intent)
    {
        Log.Info(
            intent is Intent.CreateContact ? "contact create requested" : "contact edit requested");
        _app?.Dispatch(intent);
    }

    /// <summary>Pulls the contact-write status (on a <c>Surface.ContactsStatus</c> signal).</summary>
    private void PullContactWriteStatus()
    {
        if (_app is null)
        {
            return;
        }
        ContactWriteStatus = _app.ContactWriteStatus();
    }

    private ContactWriteStatus _contactWriteStatus = uniffi.mailcal_bindings.ContactWriteStatus.Idle;

    /// <summary>
    /// The outcome of the most recent contact create or edit.
    /// </summary>
    /// <remarks>
    /// <c>Failed</c> means "we could not confirm this saved", never "rejected": a write whose
    /// server call succeeded and whose reconcile did not has already landed, and the next sync
    /// heals the local copy. <c>Invalid</c> is stated under the form the user is still looking at,
    /// so nothing repeats it out of context here.
    ///
    /// <c>internal</c> because the generated enum is; the view binds the two public members
    /// below, which are a string and a <c>Visibility</c>.
    /// </remarks>
    internal ContactWriteStatus ContactWriteStatus
    {
        get => _contactWriteStatus;
        private set
        {
            if (Set(ref _contactWriteStatus, value))
            {
                Raise(nameof(ContactWriteStatusText));
                Raise(nameof(ContactWriteStatusVisibility));
            }
        }
    }

    /// <summary>What the contacts list says about the most recent write.</summary>
    public string ContactWriteStatusText => ContactWriteStatus switch
    {
        uniffi.mailcal_bindings.ContactWriteStatus.Saving => L10n.ContactsSaving(),
        uniffi.mailcal_bindings.ContactWriteStatus.Saved => L10n.ContactsSaved(),
        uniffi.mailcal_bindings.ContactWriteStatus.Failed => L10n.ContactsSaveUnconfirmed(),
        _ => string.Empty,
    };

    /// <summary>Show that line only when there is something to say.</summary>
    public Visibility ContactWriteStatusVisibility =>
        ContactWriteStatusText.Length == 0 ? Visibility.Collapsed : Visibility.Visible;
}
