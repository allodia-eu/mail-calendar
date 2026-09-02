// The intent-dispatch half of MailboxModel (split out to keep each file under the 500-line
// limit): the fire-and-forget host intents, mail/calendar actions, navigation, pagination,
// timezone, reset, plus the small account-form helpers. Every action dispatches into the
// Rust app and returns; the observer fires when the work completes and the projection pulls
// the new snapshot. State stays in Rust; this file owns only the outbound edge.

using Allodia.Mailcal.Calendar;
using Allodia.Mailcal.ViewModels;
using uniffi.mailcal_bindings;

namespace Allodia.Mailcal.Services;

public sealed partial class MailboxModel
{
    /// <summary>Sync the account's mail and refresh the snapshot.</summary>
    public void Refresh() => _app?.Dispatch(new Intent.RefreshMail());

    /// <summary>Switch the list between flat and threaded.</summary>
    public void SetMode(ViewModeKind mode) =>
        _app?.Dispatch(new Intent.SetViewMode(mode == ViewModeKind.Threaded ? ViewMode.Threaded : ViewMode.Flat));

    /// <summary>Run a ranked full-text search, or clear it (empty query).</summary>
    public void Search(string query) =>
        _app?.Dispatch(new Intent.Search(string.IsNullOrEmpty(query) ? null : query));

    /// <summary>
    /// Focus one account's folders (by id), or the unified "all inboxes" view (<c>null</c>).
    /// The core resets the selected folder when the account changes.
    /// </summary>
    public void SelectAccount(string? id)
    {
        CloseReading();
        Destination = AppDestination.Mail;
        _app?.Dispatch(new Intent.SelectAccount(id));
    }

    /// <summary>
    /// Opens or shuts one account's folder tree in the sidebar; the core persists it.
    /// </summary>
    /// <remarks>
    /// Deliberately does none of what <see cref="SelectAccount"/> does, no reading pane close, no
    /// destination change, no selection move. Expanding is not navigating, which is what lets two
    /// accounts stand open at once and what keeps the tree as it was across a visit to the calendar
    /// (docs/folder-pane.md).
    /// </remarks>
    /// <remarks>
    /// <para>
    /// The local <see cref="Accounts"/> entry moves **first**, before the dispatch. The core is
    /// still the owner, its next snapshot overwrites this, but the dispatch is asynchronous, and
    /// the collapse itself makes the shell reconcile the sidebar before that snapshot arrives. A
    /// reconcile re-applies whatever <see cref="Accounts"/> says, so without this it re-applies the
    /// value the user just changed and the tree springs back open within a frame. It reads as a
    /// chevron that does nothing at all.
    /// </para>
    /// </remarks>
    public void SetAccountExpanded(string id, bool expanded)
    {
        for (var i = 0; i < Accounts.Count; i++)
        {
            if (Accounts[i].Id == id && Accounts[i].Expanded != expanded)
            {
                var current = Accounts[i];
                Accounts[i] = new AccountItem
                {
                    Id = current.Id,
                    Email = current.Email,
                    Expanded = expanded,
                    Folders = current.Folders,
                };
                break;
            }
        }
        _app?.Dispatch(new Intent.SetAccountExpanded(id, expanded));
    }

    /// <summary>The email of account <paramref name="id"/>, for display (falls back to the id).</summary>
    public string AccountEmail(string id)
    {
        foreach (var account in Accounts)
        {
            if (account.Id == id)
            {
                return account.Email;
            }
        }
        return id;
    }

    /// <summary>
    /// The address an account offered by one of the person's other devices is for, so the setup
    /// form opens with the typing done. Empty for every other way in.
    /// </summary>
    public string SetupStartEmail { get; private set; } = string.Empty;

    /// <summary>
    /// The record behind that address, so the form takes the route the other device wrote down
    /// rather than re-deriving one from the address. Null for every other way in.
    /// </summary>
    // `internal`, not `public`: every generated UniFFI type is emitted internal, so a public
    // signature naming one is a CS0051/CS0053 accessibility error. The same reason `SubmitSetup`
    // and `DetectAsync` are internal, and its only callers are in this assembly.
    internal AllodiaAccountOffer? SetupStartOffer { get; private set; }

    /// <summary>Open the setup form to add another account over the running app.</summary>
    /// <param name="startEmail">
    /// An address to start from, when the form was opened by accepting an offer. It only fills the
    /// field: which route the form takes is still decided by detection, so an offer whose settings
    /// have since moved is corrected rather than believed.
    /// </param>
    internal void BeginAddAccount(string startEmail = "", AllodiaAccountOffer? startOffer = null)
    {
        SetupError = null;
        SetupStartEmail = startEmail;
        // The record behind that address, when the flow was opened from an offer: the setup form
        // takes the route the other device wrote down rather than re-deriving one.
        SetupStartOffer = startOffer;
        AddingAccount = true;
    }

    /// <summary>Back out of adding an account (the user dismissed the form).</summary>
    public void CancelAddAccount()
    {
        AddingAccount = false;
        SetupStartEmail = string.Empty;
        SetupStartOffer = null;
        SetupError = null;
    }

    /// <summary>
    /// Opens <paramref name="folder"/> in <paramref name="account"/>, the account whose tree the
    /// pane row sits under, which is not necessarily the selected one.
    /// </summary>
    /// <remarks>
    /// One intent carrying both halves, because a folder key is unique only within its account and
    /// the core takes no folder without one (docs/folder-pane.md, rule 14). An account's own
    /// all-mail view is <see cref="SelectAccount"/>, there is no folder-less form of this.
    /// </remarks>
    public void SelectFolder(string account, string folder)
    {
        CloseReading();
        Destination = AppDestination.Mail;
        _app?.Dispatch(new Intent.SelectFolder(account, folder));
    }

    /// <summary>Grow the visible window by one page, the view dispatches this as it scrolls
    /// toward the end. Guarded so a burst of scroll events issues one request: it's a no-op
    /// while a previous request is still in flight or every row is already shown.</summary>
    public void ShowMore()
    {
        if (_loadMorePending || !HasMore)
        {
            return;
        }
        _loadMorePending = true;
        _app?.Dispatch(new Intent.ShowMore());
    }

    /// <summary>
    /// Bring the mailbox back to the front, from the calendar or Contacts.
    /// </summary>
    /// <remarks>
    /// Selecting an account or a folder already does this on the way past; this is for the callers
    /// that have nothing to select and still need the mail surface visible, a mail link, or an
    /// assistant's draft. Both put a composer in the mail surface's detail column, and a composer
    /// opened behind the calendar is a click that looks like it did nothing.
    /// <para>
    /// Deliberately does not touch the selection or the reading pane: the user's place in their
    /// mail is not this method's to move.
    /// </para>
    /// </remarks>
    public void ShowMail() => Destination = AppDestination.Mail;

    /// <summary>Switch to the calendar agenda and sync it.</summary>
    public void ShowCalendar()
    {
        CloseReading();
        Destination = AppDestination.Calendar;
        Log.Info("calendar refresh requested");
        _app?.Dispatch(new Intent.RefreshCalendar());
    }

    /// <summary>Send a plain-text message through the durable outbox, then re-sync.</summary>
    public void Submit(string to, string subject, string body) =>
        _app?.Dispatch(new Intent.SubmitMail(to, subject, body));

    /// <summary>Mark a message read or unread (by owning account + key), then re-sync.</summary>
    public void MarkRead(string account, string key, bool read) => _app?.Dispatch(new Intent.MarkRead(account, key, read));

    /// <summary>Flag or unflag a message (by owning account + key), then re-sync.</summary>
    public void SetFlagged(string account, string key, bool flagged) => _app?.Dispatch(new Intent.SetFlagged(account, key, flagged));

    /// <summary>Delete a message, move it to Trash, recoverable (by owning account + key), then re-sync.</summary>
    public void Delete(string account, string key) => _app?.Dispatch(new Intent.Delete(account, key));

    /// <summary>Archive a message, move it to the account's Archive folder (by owning account + key), then re-sync.</summary>
    public void Archive(string account, string key) => _app?.Dispatch(new Intent.Archive(account, key));

    /// <summary>Archive a whole conversation, the core moves every message on the thread to
    /// Archive except any in Sent (a sent copy never leaves Sent), then re-syncs.</summary>
    public void ArchiveThread(string account, string threadId) =>
        _app?.Dispatch(new Intent.ArchiveThread(account, threadId));

    /// <summary>Permanently delete a message, irreversible (by owning account + key), then re-sync.</summary>
    public void PermanentlyDelete(string account, string key) => _app?.Dispatch(new Intent.PermanentlyDelete(account, key));

    /// <summary>
    /// Create a calendar event from the editor's payload, then refresh.
    /// </summary>
    /// <remarks>
    /// The <see cref="CreateArgs"/> already carry the wall-clock strings the core wants, a timed event
    /// is a wall clock in the device zone (so it reads back the same clock on edit), an all-day event a
    /// bare date with an exclusive end. Routing to the chosen (or default) writable calendar is the
    /// core's job; this only marshals the payload.
    /// </remarks>
    internal void CreateEvent(CreateArgs args) =>
        _app?.Dispatch(new Intent.CreateEvent(
            args.Title, args.Start, args.End, args.Account, args.Calendar, args.AllDay, args.Timezone,
            args.Notes, args.Location, args.Recurrence));

    /// <summary>
    /// Edit a stored calendar event from the editor's payload, then refresh.
    /// </summary>
    /// <remarks>
    /// A provider-neutral patch: only the present fields change (the recurrence rule, attendees and
    /// alarms survive). Notes/location are three-state, empty clears, a value sets. The write settles
    /// through the same <c>CalendarWriteStatus</c> surface as create and delete.
    /// </remarks>
    internal void UpdateEvent(UpdateArgs args) =>
        _app?.Dispatch(new Intent.UpdateEvent(
            args.Account, args.Key, args.Title, args.Start, args.End, args.Notes, args.Location,
            args.Occurrence, args.Recurrence, args.TimesFromOccurrence));

    /// <summary>
    /// What saving <paramref name="args"/> over the whole series would cost the occurrences the
    /// user changed on their own, or <c>null</c> when there is nothing to say.
    /// </summary>
    /// <remarks>
    /// Asked with the payload about to be dispatched, so the answer is about <em>this</em> edit: on
    /// a server that folds a moved occurrence back only when the series moves, a retitle costs
    /// nothing and is not warned about. A local read, off the path the user waits on.
    /// </remarks>
    internal SeriesEditWarning? SeriesEditWarning(UpdateArgs args) =>
        _app?.SeriesEditWarning(args.Account, args.Key, new ProposedEdit(
            // The real one: a rule change is the edit two of the four providers answer by
            // discarding every override, so the warning has to be asked knowing about it.
            args.Title, args.Start, args.End, args.Notes, args.Location, args.Recurrence));

    /// <summary>Delete a calendar event by its owning account + provider key, then refresh the
    /// agenda. <paramref name="occurrence"/> names a single occurrence of a repeating event,
    /// the token the surface that drew it carried; <c>null</c> removes the whole series.</summary>
    public void DeleteEvent(string account, string key, string? occurrence = null) =>
        _app?.Dispatch(new Intent.DeleteEvent(account, key, occurrence));

    /// <summary>
    /// Report the device's current OS zone (an IANA id) to the core, which adopts it on
    /// first boot or raises it as a pending change when it differs from the active zone.
    /// </summary>
    public void ReportDeviceTimeZone(string id)
    {
        Log.Info($"report device zone: {id} (active {ActiveZone})");
        _app?.Dispatch(new Intent.ReportDeviceTimeZone(id));
    }

    /// <summary>Set the active display time zone (an IANA id) via the selector.</summary>
    public void SetTimeZone(string id) => _app?.Dispatch(new Intent.SetTimeZone(id));

    /// <summary>Adopt the device's reported zone (the user accepted the change prompt).</summary>
    public void AcceptTimeZoneChange() => _app?.Dispatch(new Intent.AcceptTimeZoneChange());

    /// <summary>Keep the current zone (the user dismissed the change prompt).</summary>
    public void DismissTimeZoneChange() => _app?.Dispatch(new Intent.DismissTimeZoneChange());

    /// <summary>Reset the local cache and re-fetch everything (destructive).</summary>
    public void Reset() => _app?.Reset();
}
