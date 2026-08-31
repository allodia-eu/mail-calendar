// The calendar's event CRUD, create, tap-to-open detail, edit, delete, split out of
// CalendarView.xaml.cs to keep each file under the 500-line cap (AGENTS.md); it is the same partial
// class. The seam is a real responsibility boundary: the other file is the grid/agenda/month plumbing
// (seating, paging, view switching), this one is everything a tap on an event, or the "New event"
// button, sets in motion.
//
// The write path is deliberately the same one create and delete already used, an inline dispatch that
// settles through the CalendarWriteStatus surface (Saving → Saved/Failed), so an edit is no more (and
// no less) durable than a create until the shared offline outbox makes all three durable at once.
using System;
using System.Linq;
using System.Threading.Tasks;
using Allodia.Mailcal.Calendar;
using Allodia.Mailcal.Dialogs;
using Allodia.Mailcal.Services;
using Allodia.Mailcal.ViewModels;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using uniffi.mailcal_bindings;

namespace Allodia.Mailcal.Views;

public sealed partial class CalendarView
{
    private async void OnNewEvent(object sender, RoutedEventArgs e)
    {
        if (Model is null)
        {
            return;
        }
        // A fresh editor, defaulting to the first writable calendar and the device's own zone (so the
        // created event reads back the same wall clock when it is opened to edit).
        var editor = EventEditorState.Create(DefaultChoice(), Model.ActiveZone, DateTime.Now);
        var dialog = new EventEditorDialog(Model, editor) { XamlRoot = this.XamlRoot };
        if (await DialogHelper.ShowAsync(dialog) == ContentDialogResult.Primary)
        {
            Model.CreateEvent(editor.CreateArgs());
        }
    }

    // A tap on an event (grid block, all-day bar, month chip, or agenda row) opens its detail.
    private async void OnOpenEvent(EventOpen open)
    {
        if (Model?.EventDetail(open.Account, open.Key, open.Occurrence) is not { } detail)
        {
            // The event is gone, the store changed under a stale page. Nothing to open.
            return;
        }
        Log.Info($"cal: open detail id={open.Key}");
        var dialog = new EventDetailDialog(detail, Model) { XamlRoot = this.XamlRoot };
        switch (await DialogHelper.ShowAsync(dialog))
        {
            case ContentDialogResult.Primary:
                await ShowEditor(detail);
                break;
            case ContentDialogResult.Secondary:
                await ConfirmDelete(detail);
                break;
        }
    }

    // Opens the editor prefilled from a stored event's detail, and dispatches the edit on Save.
    //
    // A loop, and that is the whole point of the shape. WinUI permits one ContentDialog at a time,
    // so a question about a save cannot be raised over the editor the way the other four clients
    // raise it, the editor has already closed by the time it is asked. Backing out of a question
    // therefore has to *reopen* the editor, which costs nothing because the dialog reads and
    // writes the same EventEditorState instance: what the user typed is still in it. Without the
    // loop, "cancel" and "discard everything I just typed" are the same button.
    private async Task ShowEditor(EventDetail detail)
    {
        if (Model is null)
        {
            return;
        }
        var editor = EventEditorState.Edit(detail, CalendarNameFor(detail.Account, detail.Calendar));
        while (true)
        {
            var dialog = new EventEditorDialog(Model, editor) { XamlRoot = this.XamlRoot };
            if (await DialogHelper.ShowAsync(dialog) != ContentDialogResult.Primary)
            {
                return;
            }
            // Which occurrences the save meant. Asked before the warning, because the answer
            // decides whether a warning is owed at all: one occurrence's edit writes an override
            // of its own and costs no other occurrence anything.
            if (editor.AsksAboutTheSeries)
            {
                var scope = await DialogHelper.ScopeAsync(
                    this.XamlRoot, L10n.EventSeriesScopeTitle());
                if (scope == ContentDialogResult.Primary)
                {
                    Model.UpdateEvent(editor.UpdateArgs(thisOccurrenceOnly: true));
                    return;
                }
                if (scope != ContentDialogResult.Secondary)
                {
                    continue;
                }
            }
            // What a whole-series save costs the occurrences the user changed on their own, asked
            // with the payload in hand, so a retitle is not told it will move anything. The edit
            // is the only moment anything can be done about it.
            var args = editor.UpdateArgs(thisOccurrenceOnly: false);
            if (SeriesWarningText.For(Model.SeriesEditWarning(args)) is { } warning)
            {
                var answer = await DialogHelper.ConfirmAsync(
                    this.XamlRoot, L10n.EventSeriesWarningTitle(), warning, L10n.ActionSave());
                if (answer != ContentDialogResult.Primary)
                {
                    continue;
                }
            }
            Model.UpdateEvent(args);
            return;
        }
    }

    // Confirms then dispatches a delete.
    //
    // Two different questions, and which one is asked turns on whether the user opened a single
    // occurrence. If they did, it is "this event, or all of them?", and that replaces the generic
    // confirm rather than following it, because it already carries a way out and one delete should
    // raise one dialog. If they did not (a one-off event, or an agenda row, which *is* the series),
    // there is no occurrence to name and the ordinary confirmation stands.
    //
    // Which occurrence is read off the **detail**, not off the reference that opened it: the detail
    // names what the core actually resolved, so a token that has gone stale asks nothing and
    // removes the series, which is what its times say it is describing.
    private async Task ConfirmDelete(EventDetail detail)
    {
        if (Model is null)
        {
            return;
        }
        if (!string.IsNullOrEmpty(detail.OccurrenceStart))
        {
            var scope = await DialogHelper.ScopeAsync(
                this.XamlRoot, L10n.EventSeriesScopeDeleteTitle());
            switch (scope)
            {
                case ContentDialogResult.Primary:
                    Model.DeleteEvent(detail.Account, detail.Key, detail.OccurrenceStart);
                    break;
                case ContentDialogResult.Secondary:
                    Model.DeleteEvent(detail.Account, detail.Key);
                    break;
            }
            return;
        }
        var content = detail.IsRecurring ? L10n.EventSeriesNote() : string.Empty;
        var result = await DialogHelper.ConfirmAsync(
            this.XamlRoot, L10n.EventDeleteConfirm(), content, L10n.ActionDelete());
        if (result == ContentDialogResult.Primary)
        {
            Model.DeleteEvent(detail.Account, detail.Key);
        }
    }

    // The first writable calendar, as the create editor's default target (null if none, the New
    // event button is disabled in that case, so this only runs when at least one exists).
    private CalendarChoice? DefaultChoice()
    {
        var row = Model?.Calendars().FirstOrDefault(c => c.CanWrite);
        return row is null ? null : new CalendarChoice(row.Account, row.Id, row.Name);
    }

    private string CalendarNameFor(string account, string calendar) =>
        Model?.Calendars().FirstOrDefault(c => c.Account == account && c.Id == calendar)?.Name ?? calendar;

    private void OnAgendaEventClick(object sender, ItemClickEventArgs e)
    {
        if (e.ClickedItem is EventItem item)
        {
            // An agenda row *is* the series, one row per event, not per occurrence, so there is
            // no single day to name and a write from here is a series write.
            OnOpenEvent(new EventOpen(item.Account, item.Key, string.Empty));
        }
    }

    private void OnDeleteEvent(object sender, RoutedEventArgs e)
    {
        if ((sender as FrameworkElement)?.Tag is EventItem item)
        {
            Model?.DeleteEvent(item.Account, item.Key);
        }
    }

    // Re-authenticate the first Microsoft account whose calendar is withheld for lack of the
    // calendar scope: re-runs its sign-in (login_hint = its address), upgrading its token in place.
    // The banner clears once the calendar connects; if several are affected it re-renders for the
    // next after each completes.
    private void OnCalendarReauth(object sender, RoutedEventArgs e)
    {
        if (Model?.CalendarReauthEmail is { } email)
        {
            Model.SignInWithMicrosoft(email);
        }
    }
}
