// The event detail sheet, what a tap on any event opens. Title, time, calendar, location, notes,
// and the reminder/recurrence summaries, with Edit + Delete (matching the platform calendar the user
// knows), and an invitee's answer where the core offers one (EventAnswerView.swift). Edit and
// Delete are gated on the event's `canWrite`: a read-only calendar's event opens read-only, with no
// actions (an affordance that can never fire is just a mystery).

import MailcalBindings
import SwiftUI

struct EventDetailView: View {
    let detail: EventDetail
    let calendars: [CalendarRow]
    let onEdit: () -> Void
    let onDelete: () -> Void
    /// Whether the screen will ask *This event · All events* before it deletes. That question is
    /// itself the confirmation, cancelling it writes nothing, so asking to confirm first would
    /// put two dialogs in a row for one delete.
    let asksAboutTheSeries: Bool
    /// Answers the meeting from here, with the reply subject; used only where `myAnswer` is set.
    var onAnswer: (InvitationResponse, String) -> Void = { _, _ in }
    /// Whether a calendar write is on its way, which holds the answer buttons back.
    var saving = false
    /// The "organiser wasn't told" question, put from here while the detail is open, as the window
    /// puts it otherwise; see `MailboxModel.eventDetailOpen`.
    var replyPrompt: ReplyPrompt?
    var onReplyPrompt: (Bool, Bool) -> Void = { _, _ in }

    @Environment(\.dismiss) private var dismiss
    @Environment(\.colorScheme) private var scheme
    @State private var confirmingDelete = false

    var body: some View {
        let row = calendars.row(account: detail.account, calendar: detail.calendar)
        NavigationStack {
            List {
                Section {
                    Text(detail.title.isEmpty ? L10n.event_no_title() : detail.title)
                        .font(.title2).bold()
                    VStack(alignment: .leading, spacing: 2) {
                        Text(detailTime(detail))
                        if !detail.timezone.isEmpty {
                            Text(detail.timezone).font(.caption).foregroundStyle(.secondary)
                        }
                    }
                    HStack(spacing: 10) {
                        Circle()
                            .fill(parseHexColor(row.swatchOrFallback(dark: scheme == .dark).background))
                            .frame(width: 14, height: 14)
                        Text(row?.name ?? detail.calendar)
                    }
                }
                if let answer = EventAnswer(detail: detail, saving: saving) {
                    EventAnswerSection(
                        answer: answer,
                        title: detail.title,
                        onAnswer: onAnswer
                    )
                }
                if let location = detail.location, !location.isEmpty {
                    labeled(L10n.event_location(), location)
                }
                if let notes = detail.notes, !notes.isEmpty {
                    labeled(L10n.event_notes(), LinkedText.text(linkingIn: notes))
                        .gatedLinkOpening()
                }
                labeled(L10n.event_reminder(), reminderText(detail.reminderMinutes))
                labeled(
                    L10n.event_repeat(),
                    recurrenceText(detail.repeatSummary, isRecurring: detail.isRecurring)
                )
                // No section at all for an appointment nobody was invited to, an empty
                // "Attendees" heading would read as "we looked and found none", which is a
                // different statement from "this is not a meeting".
                if !detail.attendees.isEmpty {
                    Section(L10n.event_attendees()) {
                        EventAttendeesSection(attendees: detail.attendees)
                    }
                }
            }
            .navigationTitle(Text(""))
            .toolbar {
                ToolbarItem(placement: .cancellationAction) {
                    Button(L10n.action_close()) { dismiss() }
                }
                if detail.canWrite {
                    #if os(macOS)
                        // A macOS sheet's bottom bar draws one primary action, so Delete takes the
                        // destructive slot of its own; grouped with Edit it was never drawn.
                        ToolbarItem(placement: .primaryAction) { editButton }
                        ToolbarItem(placement: .destructiveAction) { deleteButton }
                    #else
                        ToolbarItemGroup(placement: .primaryAction) {
                            editButton
                            deleteButton
                        }
                    #endif
                }
            }
            // An `alert`, not a `confirmationDialog`: iPadOS presents the latter as a popover, and a
            // popover DROPS the `.cancel`-role button, so this read as one destructive button with no
            // way out. See the remove-account alert in Mailcal.swift for the full note.
            .alert(
                L10n.event_delete_confirm(),
                isPresented: $confirmingDelete
            ) {
                Button(L10n.action_delete(), role: .destructive, action: onDelete)
                Button(L10n.action_cancel(), role: .cancel) {}
            } message: {
                if detail.isRecurring { Text(L10n.event_series_note()) }
            }
            // The same contract as the window's: the core owns the question, so the binding only
            // reads it and a swipe-away cannot orphan it.
            .sheet(isPresented: Binding(get: { replyPrompt != nil }, set: { _ in })) {
                if let replyPrompt {
                    InvitationReplyPromptView(prompt: replyPrompt, answer: onReplyPrompt)
                        .interactiveDismissDisabled()
                }
            }
        }
        // A macOS sheet sizes itself to its content, and a `List` has no intrinsic height to give it
        // so without this the sheet came up as a blank strip with only the toolbar on it: Close and
        // Edit floating over nothing. `CalendarManagerView` has always carried the same frame.
        //
        // The height is a *choice*, not a measurement, for that same reason: the list cannot tell the
        // sheet how tall it wants to be. 340 was right until attendees arrived and then cut the third
        // one off mid-row behind a scrollbar, so a meeting asks for more. It stays a fixed step
        // rather than a per-row calculation: a 40-person invitation must not open a sheet taller than
        // the screen, and past a handful of people scrolling is the right answer anyway. An invitee's
        // answer row adds one step more.
        .frame(minWidth: 360, minHeight: detail.attendees.isEmpty ? 340 : detail.myAnswer == nil ? 480 : 560)
    }

    private var editButton: some View {
        Button(L10n.action_edit(), systemImage: "pencil", action: onEdit)
    }

    private var deleteButton: some View {
        Button(L10n.action_delete(), systemImage: "trash", role: .destructive) {
            if asksAboutTheSeries { onDelete() } else { confirmingDelete = true }
        }
    }

    private func labeled(_ label: String, _ value: String) -> some View {
        labeled(label, Text(value))
    }

    private func labeled(_ label: String, _ value: Text) -> some View {
        VStack(alignment: .leading, spacing: 2) {
            Text(label).font(.caption).foregroundStyle(.tint)
            value.textSelection(.enabled)
        }
    }
}

/// The event's time as one line, in its own wall clock. All-day shows the inclusive day(s); a timed
/// event shows the date and a start–end time range, collapsing the date when start and end share one.
func detailTime(_ detail: EventDetail) -> String {
    let dateStyle = Date.FormatStyle(date: .complete, time: .omitted)
    let timeStyle = Date.FormatStyle(date: .omitted, time: .shortened)
    let start = EventEditorState.parseWall(detail.start)
    if detail.allDay {
        // The stored end is exclusive; show the inclusive last day.
        let lastDay = EventEditorState.previousDay(EventEditorState.parseWall(detail.end))
        let cal = Calendar.current
        if cal.isDate(lastDay, inSameDayAs: start) {
            return start.formatted(dateStyle)
        }
        return "\(start.formatted(dateStyle)) – \(lastDay.formatted(dateStyle))"
    }
    let end = EventEditorState.parseWall(detail.end)
    if Calendar.current.isDate(start, inSameDayAs: end) {
        return "\(start.formatted(dateStyle)), \(start.formatted(timeStyle)) – \(end.formatted(timeStyle))"
    }
    return "\(start.formatted(dateStyle)) \(start.formatted(timeStyle)) – \(end.formatted(dateStyle)) \(end.formatted(timeStyle))"
}
