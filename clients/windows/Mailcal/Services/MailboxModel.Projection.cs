// The snapshot projection half of MailboxModel (split out to keep each file under the
// 500-line limit). Reload pulls each surface's immutable snapshot and reconciles it into
// the bound ObservableCollections IN PLACE, matching rows by their stable id and replacing
// only what changed, rather than Clear()+Add. That keeps a list's scroll position, any
// open context menu, and the container for an unchanged row intact across a refresh (the
// reason MailRow.Id / "m:<account>:<key>" / "t:<account>:<thread>" exists, the account is
// part of the identity because a provider key/thread id is unique only WITHIN an account, so
// two accounts can collide on one in the unified view), the WinUI counterpart of the macOS
// list's stable `id: \.rowID`. An unchanged collection (e.g. the calendar during a mail
// action) reconciles to a no-op, so unrelated surfaces aren't disturbed either.

using System.Collections.Generic;
using System.Collections.ObjectModel;
using System.Linq;
using Allodia.Mailcal.Calendar;
using Allodia.Mailcal.ViewModels;
using uniffi.mailcal_bindings;

namespace Allodia.Mailcal.Services;

public sealed partial class MailboxModel
{
    // DEBUG-only launch hooks (MailboxModel.Debug.cs). The defining declaration is unconditional
    // so Reload can call it in any configuration; with no implementing declaration in Release the
    // call is elided by the compiler.
    partial void ApplyLaunchHooks();

    /// <summary>Pulls each surface's snapshot and reconciles it into the bound collections.</summary>
    private void Reload(Surface? changed = null)
    {
        if (_app is null)
        {
            return;
        }
        // A send signal only updates the "sending…" → "sent" hint; it doesn't touch the
        // mailbox projection (the post-send refresh fires its own MailboxList signal).
        if (changed == Surface.Sending)
        {
            UpdateSendStatus(_app.SendStatus());
            return;
        }
        // A sync-progress signal only updates the download bar; the rows it commits arrive
        // on their own MailboxList signal, so this doesn't touch the projection.
        if (changed == Surface.SyncProgress)
        {
            UpdateSyncProgress(_app.SyncProgress());
            return;
        }
        // A connectivity signal updates the offline banner + per-account outage badges; it
        // doesn't touch the mailbox projection.
        if (changed == Surface.Connectivity)
        {
            UpdateConnectivity(_app.Connectivity());
            return;
        }
        // A calendar-write-status signal only moves the small header badge (spinner -> check /
        // warning); the grid/agenda arrive on their own Surface.Calendar signal.
        if (changed == Surface.CalendarStatus)
        {
            UpdateCalendarWriteStatus(_app.CalendarWriteStatus());
            return;
        }
        // An invitation-reply signal only raises or clears the "the organiser wasn't told" prompt.
        // It arrives in both directions, the core clears the question the moment it is answered,
        // so this mirrors whatever it now holds rather than only setting it.
        if (changed == Surface.InvitationReply)
        {
            PullReplyPrompt();
            return;
        }
        // Likewise the missing-Sent-copy question: raised when a delivered message left no copy
        // behind, cleared when the copy is filed or the user accepts its absence.
        if (changed == Surface.UnfiledCopy)
        {
            PullUnfiledCopy();
            return;
        }
        // A contacts signal, an address-book sync landed, or the core answered a search, only
        // rebuilds the people list; it touches neither the mailbox projection nor the calendar.
        if (changed == Surface.Contacts)
        {
            PullContacts();
            return;
        }
        // A write-status signal only moves the contacts header's line; the list arrives on its
        // own Contacts signal.
        if (changed == Surface.ContactsStatus)
        {
            PullContactWriteStatus();
            return;
        }
        // Time the snapshot pull (the FFI marshalling of every row) separately from the
        // reconcile, so the "render leg" cost is attributable against the core's own timing.
        var reloadSw = System.Diagnostics.Stopwatch.StartNew();
        var tz = _app.TimezoneSettings();
        ActiveZone = tz.Active;
        // Device detection is region-aware (shared Rust), so a pending change now means a
        // genuine move, surfaced as a real prompt, no spurious-prompt workaround needed.
        PendingDeviceZone = tz.PendingDevice;
        var zone = tz.Active;

        var pullSw = System.Diagnostics.Stopwatch.StartNew();
        var snapshot = _app.MailboxList();
        var pullMs = pullSw.ElapsedMilliseconds;
        // Sync the sidebar collections first, then the selection scalars, so a selection change
        // rebuilds the sidebar against already-current accounts/folders (the shell rebuilds on
        // both), never against a half-updated set.
        SyncAccounts(snapshot.Accounts, snapshot.AccountFolders);
        SyncFolders(snapshot.Folders);
        UnifiedUnread = snapshot.UnifiedUnread;
        Mode = snapshot.Mode == ViewMode.Threaded ? ViewModeKind.Threaded : ViewModeKind.Flat;
        SelectedAccount = snapshot.SelectedAccount;
        SelectedFolder = snapshot.Selected;
        SearchHorizon = snapshot.SearchHorizon;

        var rows = snapshot.Rows.Select(row =>
        {
            var built = BuildRow(row, zone);
            // Restore inline-expansion state after a refresh (it's UI state, not in the snapshot),
            // so a background sync doesn't collapse a conversation the user has open.
            if (built.IsThread)
            {
                built.IsExpanded = _expandedThreads.Contains(built.Id);
            }
            return built;
        })
        // Withhold the rows a swipe is hiding. A deferred Delete/Archive dispatches nothing until
        // its undo window closes, so the core still projects the row, this is the only place it
        // can be kept off the list (MailboxModel.SwipeSettings.cs).
        .Where(built => !IsRowHidden(built.Id))
        .ToList();
        Reconcile(Rows, rows, row => row.Id, SameRow);
        // Record the full count and release the "show more" guard: this snapshot is the answer
        // to any in-flight request, so the view may ask for the next page once it scrolls again.
        _total = snapshot.Total;
        _loadMorePending = false;

        // The agenda and the grid reflect CALENDAR data, which a mail refresh never touches. Rebuilding
        // the full event list (a real diary is ~10k events) and reconciling it into the UI-bound
        // `Events` collection on the UI thread, on *every* mailbox signal, and an account sync fires
        // those dozens of times a second, froze the app for seconds. So do it only when the calendar,
        // or the timezone it is formatted against, actually changed (§5: "a refresh that changed
        // nothing signals nothing"). A mailbox refresh leaves the agenda and the grid untouched.
        if (changed is null or Surface.Calendar or Surface.Settings)
        {
            var events = _app.CalendarList().Events.Select(item => new EventItem
            {
                Account = item.Account,
                Key = item.Key,
                Title = string.IsNullOrEmpty(item.Title) ? L10n.EventNoTitle() : item.Title,
                StartText = TimeZones.LocalDateTime(item.Start, zone),
                OffersDelete = CalendarWriteGating.OffersDelete(item),
                // An unanswered invitation is a hold, and the agenda has no border to dash, so it
                // is the row's own words that say so (docs/invitations.md).
                AwaitingText = InvitationFormat.IsAwaitingResponse(item.Participation)
                    ? L10n.A11yInvitationAwaitingResponse()
                    : string.Empty,
            }).ToList();
            Reconcile(Events, events, item => item.Account + ":" + item.Key, SameEvent);

            // The "New event" gate rides the same signal: the calendar list (with each one's write
            // flag) is read off a cheap in-memory page pull, and the button is DISABLED, never
            // hidden, the header keeps its shape, while no calendar accepts writes.
            NewEventEnabled = CalendarWriteGating.CanCreate(Calendars());

            // The agenda is a snapshot; the GRID is a pull (MailboxModel.CalendarGrid.cs). One snapshot
            // slot cannot hold the five pages a pager keeps in hand, so the grid gets a version bump
            // instead and re-pulls whatever it is showing.
            CalendarVersion++;
        }

        // A settings change is its own signal to the calendar: re-apply the display settings (horizon,
        // clock, week-start alignment) and re-seat, which the plain calendar-data version bump above
        // deliberately does NOT do, so a background sync cannot jerk the grid back to today.
        if (changed == Surface.Settings)
        {
            BumpDisplaySettings();
        }

        // The derived mail count/name labels may have changed with the collections.
        Raise(nameof(CurrentFolderName));
        Raise(nameof(MailCountText));

        // The reading body (a potentially large HTML string) only changes on a Reading
        // signal, pull it just then, not on every mailbox/calendar/settings refresh (mirrors
        // macOS's `if case .reading = surface`).
        if (changed == Surface.Reading)
        {
            PullReading();
        }

        Log.Info($"reload: rows={Rows.Count} ({Mode}), folders={Folders.Count}, "
            + $"events={Events.Count}, zone={zone} "
            + $"(FFI pull {pullMs}ms, total {reloadSw.ElapsedMilliseconds}ms)");

        // DEBUG-only: honour any MAILCAL_* launch hook now that this surface has populated
        // (e.g. the first row exists for MAILCAL_OPEN_FIRST). Elided in Release.
        ApplyLaunchHooks();
    }

    /// <summary>
    /// Rebuilds <see cref="Accounts"/> only when the account set actually changes, so the
    /// sidebar switcher isn't churned on every snapshot refresh (the rows that change are the
    /// mail rows). Order is preserved (the core keeps add order).
    /// </summary>
    /// <remarks>
    /// Each account carries its **own** folders, taken from the snapshot's `account_folders`,
    /// which the core populates in every view, including the unified one. The sidebar used to be
    /// fed `snapshot.Folders`, the *selected* account's folders alone, which is why picking All
    /// Inboxes emptied the pane (docs/folder-pane.md).
    /// <para>
    /// "Actually changes" now includes the unread counts and the expansion, since both are drawn.
    /// A count that moved with no other change still has to reach the pane, or the badge sits at
    /// the number it had when the folder set last changed.
    /// </para>
    /// </remarks>
    private void SyncAccounts(AccountRow[] accounts, AccountFolderRow[] accountFolders)
    {
        var byAccount = accountFolders.ToDictionary(row => row.AccountId, row => row.Folders);
        var wanted = accounts
            .Select(account => new AccountItem
            {
                Id = account.Id,
                Email = account.Email,
                Expanded = account.Expanded,
                Folders = byAccount.TryGetValue(account.Id, out var folders)
                    ? folders.Select(ToFolderItem).ToArray()
                    : [],
            })
            .ToArray();
        if (Accounts.Count == wanted.Length && Accounts.Zip(wanted).All(pair => Same(pair.First, pair.Second)))
        {
            return;
        }
        Accounts.Clear();
        foreach (var account in wanted)
        {
            Accounts.Add(account);
        }
    }

    private static FolderItem ToFolderItem(FolderRow folder)
    {
        var role = ToSidebarRole(folder.Role);
        return new FolderItem
        {
            Key = folder.Key,
            // Named here, once, so every surface that renders a FolderItem, the pane, and the
            // list header through CurrentFolderName, agrees on what the folder is called.
            Name = FolderLabel.For(role, folder.Name),
            Role = role,
            Unread = folder.Unread,
        };
    }

    /// <summary>
    /// Maps the core's folder role onto the sidebar's own mirror of it.
    /// </summary>
    /// <remarks>
    /// The mirror exists because the generated bindings are <c>internal</c> and the sidebar types
    /// are linked into a second assembly (see <see cref="SidebarFolderRole"/>).
    /// <para>
    /// The trailing arm is not laziness and cannot be dropped: a C# switch over an enum is never
    /// exhaustive to the compiler (an enum variable may hold any underlying value), so this cannot
    /// be made to break the build when the core gains a role, the way the core's own
    /// <c>role_rank</c> match does. It maps to <see cref="SidebarFolderRole.Other"/> rather than
    /// <c>None</c>, so an unrecognized *special* folder is still recorded as special, it draws the
    /// plain folder icon either way, but it does not start claiming to be an ordinary folder.
    /// </para>
    /// </remarks>
    internal static SidebarFolderRole ToSidebarRole(FolderRole? role) => role switch
    {
        null => SidebarFolderRole.None,
        FolderRole.Inbox => SidebarFolderRole.Inbox,
        FolderRole.Drafts => SidebarFolderRole.Drafts,
        FolderRole.Sent => SidebarFolderRole.Sent,
        FolderRole.Archive => SidebarFolderRole.Archive,
        FolderRole.Junk => SidebarFolderRole.Junk,
        FolderRole.Trash => SidebarFolderRole.Trash,
        _ => SidebarFolderRole.Other,
    };

    /// <summary>Whether two projected accounts are the same in every way the sidebar draws.</summary>
    private static bool Same(AccountItem a, AccountItem b) =>
        a.Id == b.Id
        && a.Email == b.Email
        && a.Expanded == b.Expanded
        && a.Folders.Count == b.Folders.Count
        && a.Folders.Zip(b.Folders).All(pair =>
            pair.First.Key == pair.Second.Key
            && pair.First.Name == pair.Second.Name
            && pair.First.Role == pair.Second.Role
            && pair.First.Unread == pair.Second.Unread);

    /// <summary>
    /// Rebuilds <see cref="Folders"/> only when the folder set actually changes, so the
    /// sidebar selection isn't reset on every snapshot refresh. The synthetic "All Mail"
    /// head (a null key) always leads.
    /// </summary>
    private void SyncFolders(FolderRow[] folders)
    {
        var unchanged = Folders.Count == folders.Length + 1
            && Folders[0].Key is null
            && folders.Select((f, i) => Folders[i + 1].Key == f.Key && Folders[i + 1].Name == f.Name).All(same => same);
        if (unchanged)
        {
            return;
        }
        Folders.Clear();
        Folders.Add(new FolderItem { Key = null, Name = L10n.SidebarAllMail() });
        foreach (var folder in folders)
        {
            Folders.Add(ToFolderItem(folder));
        }
    }

    private static MailRow BuildRow(SnapshotRow row, string zone) => row switch
    {
        SnapshotRow.Flat flat => new MailRow
        {
            Id = "m:" + flat.Row.Account + ":" + flat.Row.Key,
            Account = flat.Row.Account,
            IsThread = false,
            Key = flat.Row.Key,
            LatestKey = flat.Row.Key,
            Title = string.IsNullOrEmpty(flat.Row.Subject) ? L10n.MailNoSubject() : flat.Row.Subject,
            Subtitle = flat.Row.From,
            Avatar = AvatarItem.From(flat.Row.Avatar),
            DateText = TimeZones.RelativeDate(flat.Row.Date, zone),
            FullDateText = TimeZones.LocalDateTime(flat.Row.Date, zone),
            Unread = flat.Row.Unread,
            Flagged = flat.Row.Flagged,
            HasAttachment = flat.Row.HasAttachment,
            MessageCount = 1,
        },
        SnapshotRow.Thread thread => new MailRow
        {
            Id = "t:" + thread.Row.Account + ":" + thread.Row.ThreadId,
            Account = thread.Row.Account,
            IsThread = true,
            Key = thread.Row.ThreadId,
            LatestKey = thread.Row.LatestKey,
            Title = string.IsNullOrEmpty(thread.Row.Subject) ? L10n.MailNoSubject() : thread.Row.Subject,
            Subtitle = thread.Row.LatestFrom,
            Avatar = AvatarItem.From(thread.Row.Avatar),
            DateText = TimeZones.RelativeDate(thread.Row.LatestDate, zone),
            FullDateText = TimeZones.LocalDateTime(thread.Row.LatestDate, zone),
            // A conversation is unread if anything in it is, the header is a summary, and the
            // unread dot and the bold weight both read off this.
            Unread = thread.Row.UnreadCount > 0,
            HasAttachment = thread.Row.HasAttachment,
            MessageCount = thread.Row.MessageCount,
            // The whole conversation as sub-rows the expanded thread reveals (newest first).
            Messages = thread.Row.Messages.Select(m => new ThreadMessageItem
            {
                Account = m.Account,
                Key = m.Key,
                Subject = string.IsNullOrEmpty(thread.Row.Subject) ? L10n.MailNoSubject() : thread.Row.Subject,
                FromText = m.From,
                Avatar = AvatarItem.From(m.Avatar),
                DateText = TimeZones.RelativeDate(m.Date, zone),
                FullDateText = TimeZones.LocalDateTime(m.Date, zone),
                PreviewText = m.Preview,
                Unread = m.Unread,
                Outgoing = m.Outgoing,
                HasAttachment = m.HasAttachment,
            }).ToList(),
        },
        _ => throw new InvalidOperationException("unknown snapshot row kind"),
    };

    // The avatar is part of what makes two rows the same row, because a photo arrives in a LATER
    // snapshot than the row it belongs to (docs/avatars.md, "Resolution never blocks a row"). Left
    // out, every other field would match, the reconcile would keep the container it already had,
    // and no face would ever appear on a row that has one, with nothing about the result looking
    // wrong.
    private static bool SameRow(MailRow a, MailRow b) =>
        a.IsThread == b.IsThread && a.Title == b.Title && a.Subtitle == b.Subtitle
        && a.DateText == b.DateText && a.Unread == b.Unread && a.Flagged == b.Flagged
        && a.HasAttachment == b.HasAttachment
        && a.MessageCount == b.MessageCount
        && a.Avatar == b.Avatar
        && SameFaces(a.Messages, b.Messages);

    // The sub-rows' faces, for the same reason, and it is not covered by the line above: a
    // thread's own avatar is its LATEST sender's, so an earlier sender's photo arriving in that
    // second snapshot moves nothing on the header row while the expanded conversation under it
    // still shows initials.
    private static bool SameFaces(
        IReadOnlyList<ThreadMessageItem> a, IReadOnlyList<ThreadMessageItem> b)
    {
        if (a.Count != b.Count)
        {
            return false;
        }
        for (var index = 0; index < a.Count; index++)
        {
            if (a[index].Avatar != b[index].Avatar)
            {
                return false;
            }
        }
        return true;
    }

    private static bool SameEvent(EventItem a, EventItem b) =>
        a.Title == b.Title && a.StartText == b.StartText && a.OffersDelete == b.OffersDelete;

    /// <summary>
    /// Reconciles <paramref name="target"/> toward <paramref name="next"/> in place: items
    /// matched by <paramref name="key"/> are kept (and replaced only when
    /// <paramref name="equal"/> reports a content change), gone items removed, new items
    /// inserted, and survivors moved into order, so unchanged rows keep their container.
    /// </summary>
    private static void Reconcile<T>(
        ObservableCollection<T> target,
        IReadOnlyList<T> next,
        Func<T, string> key,
        Func<T, T, bool> equal)
    {
        var keep = new HashSet<string>(next.Select(key));
        for (var i = target.Count - 1; i >= 0; i--)
        {
            if (!keep.Contains(key(target[i])))
            {
                target.RemoveAt(i);
            }
        }
        // Earlier positions are already final, so the item for position i is at i or ahead.
        for (var i = 0; i < next.Count; i++)
        {
            var wanted = key(next[i]);
            if (i < target.Count && key(target[i]) == wanted)
            {
                if (!equal(target[i], next[i]))
                {
                    target[i] = next[i];
                }
                continue;
            }
            var found = -1;
            for (var j = i + 1; j < target.Count; j++)
            {
                if (key(target[j]) == wanted)
                {
                    found = j;
                    break;
                }
            }
            if (found >= 0)
            {
                target.Move(found, i);
                if (!equal(target[i], next[i]))
                {
                    target[i] = next[i];
                }
            }
            else
            {
                target.Insert(i, next[i]);
            }
        }
        while (target.Count > next.Count)
        {
            target.RemoveAt(target.Count - 1);
        }
    }
}
