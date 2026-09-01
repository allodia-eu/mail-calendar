// The event editor, one form for both create and edit, matching the flow of the platform calendar
// the user knows: title, all-day, start/end, calendar, location, notes, with reminder and recurrence
// shown but not yet editable. Presented as a sheet; the calendar picker is a
// nested sheet grouped by account. The state and every decision live in EventEditorState.

import MailcalBindings
import SwiftUI

/// A Save that has been answered "all events" and is waiting on the warning, holding both the
/// payload it will send and the sentence it is putting first.
private struct PendingSeriesSave {
    let args: UpdateArgs
    let warning: SeriesEditWarning
}

struct EventEditorView: View {
    @Binding var editor: EventEditorState
    let calendars: [CalendarRow]
    let onCancel: () -> Void
    let onCreate: (CreateArgs) -> Void
    let onUpdate: (UpdateArgs) -> Void
    /// What a whole-series save of this payload would cost, the core's decision, asked with the
    /// edit in hand. A closure so the editor stays a pure form with no reach into the app.
    let warningFor: (UpdateArgs) -> SeriesEditWarning?

    @State private var picking = false
    /// A Save on an occurrence of a series, waiting for the user to say which occurrences they
    /// meant. Nothing is written until they answer.
    @State private var askingScope = false
    /// A Save waiting on the series-edit warning, holding the payload it will send if confirmed.
    /// Nothing is written until the user answers, and cancelling leaves the editor open with the
    /// form untouched, so the way out is never "lose what I typed".
    @State private var confirmingSave: PendingSeriesSave?
    @Environment(\.colorScheme) private var scheme
    @FocusState private var titleFocused: Bool

    var body: some View {
        NavigationStack {
            Form {
                TextField(L10n.event_title_label(), text: $editor.title)
                    .focused($titleFocused)

                // All-day is set at create and frozen on edit (the patcher refuses a form change).
                Toggle(L10n.calendar_all_day(), isOn: $editor.allDay)
                    .disabled(!editor.canEditForm)

                DatePicker(
                    L10n.event_start(),
                    selection: $editor.start,
                    displayedComponents: editor.allDay ? [.date] : [.date, .hourAndMinute]
                )
                DatePicker(
                    L10n.event_end(),
                    selection: $editor.end,
                    displayedComponents: editor.allDay ? [.date] : [.date, .hourAndMinute]
                )

                // Calendar, a picker on create, display-only on edit (no cross-calendar move yet).
                Button { if editor.canEditForm { picking = true } } label: { calendarRow }
                    .disabled(!editor.canEditForm)
                    .buttonStyle(.plain)

                // Location: settable on create and edit alike, the engine's create draft carries it.
                TextField(L10n.event_location(), text: $editor.location)
                TextField(L10n.event_notes(), text: $editor.notes, axis: .vertical)

                // Reminder: shown, not yet editable. The repeat is a set of controls when the
                // core handed over a draft, and the sentence it already decided when it did not.
                Section {
                    LabeledContent(L10n.event_reminder(), value: reminderText(editor.editing?.reminderMinutes))

                    if editor.canEditRepeat {
                        EventRepeatSection(
                            draft: $editor.repeatDraft,
                            start: editor.start,
                            opensOnOneOccurrence: !(editor.editing?.occurrence.isEmpty ?? true)
                        )
                    } else {
                        LabeledContent(
                            L10n.event_repeat(),
                            value: recurrenceText(
                                editor.editing?.repeatSummary,
                                isRecurring: editor.editing?.isRecurring ?? false
                            )
                        )
                        Text(L10n.event_repeat_locked())
                            .font(.footnote)
                            .foregroundStyle(.secondary)
                    }

                    // Only when the answer is settled. An editor opened on one occurrence asks
                    // at Save which occurrences were meant, so stating the answer up here would
                    // be telling the user something the next dialog contradicts.
                    if editor.editing?.isRecurring == true, !editor.asksAboutTheSeries,
                        editor.editing?.occurrence.isEmpty == true
                    {
                        Text(L10n.event_series_note())
                            .font(.footnote)
                            .foregroundStyle(.secondary)
                    }
                }

                // Attendees: shown so an edit is not made blind to who is coming, and stated to be
                // read-only rather than offered as a field that would quietly drop the change:
                // editing them means sending iTIP updates, which is its own feature.
                if let attendees = editor.editing?.attendees, !attendees.isEmpty {
                    Section {
                        EventAttendeesSection(attendees: attendees)
                        Text(L10n.event_attendees_read_only())
                            .font(.footnote)
                            .foregroundStyle(.secondary)
                    } header: {
                        Text(L10n.event_attendees())
                    }
                }
            }
            .formStyle(.grouped)
            .navigationTitle(editor.isEditing ? L10n.event_edit_title() : L10n.event_new_title())
            .toolbar {
                ToolbarItem(placement: .cancellationAction) {
                    Button(L10n.action_cancel(), action: onCancel)
                }
                ToolbarItem(placement: .confirmationAction) {
                    Button(L10n.action_save()) { save() }
                        .disabled(!editor.isValid)
                }
            }
            // The caret opens where the work starts, the empty title on a new event, the same rule
            // the composer's To follows (docs/calendar.md, docs/contacts.md §4). Not on edit: the
            // event already has a title, and raising the keyboard over the form would hide the
            // dates that are usually what the user came to change.
            //
            // Deferred by one turn rather than set here: this is a sheet, and focus assigned while
            // the presentation is still animating is dropped on the floor, leaving a field that
            // only *looks* ready.
            .onAppear {
                if !editor.isEditing {
                    Task { @MainActor in titleFocused = true }
                }
            }
            .onChange(of: editor.start) { _, newStart in
                // Keep the end at or after the start when the start is dragged past it.
                if editor.end < newStart { editor.end = newStart }
            }
            .sheet(isPresented: $picking) {
                CalendarPickerView(calendars: calendars, selected: editor.calendar) { choice in
                    editor.calendar = choice
                    picking = false
                }
            }
            // Which occurrences this save meant. Asked before the warning, because the answer
            // decides whether a warning is owed at all: *This event* writes an override of its
            // own and costs no other occurrence anything.
            //
            // An `alert` rather than a confirmation dialog, for the reason the delete question is
            // one: iPadOS draws a confirmation dialog as a popover, and a popover drops the
            // `.cancel`-role button, leaving a destructive question with no way out.
            .alert(L10n.event_series_scope_title(), isPresented: $askingScope) {
                Button(L10n.event_series_scope_this()) {
                    onUpdate(editor.updateArgs(thisOccurrenceOnly: true))
                }
                Button(L10n.event_series_scope_all()) {
                    commitSeries(editor.updateArgs(thisOccurrenceOnly: false))
                }
                Button(L10n.action_cancel(), role: .cancel) {}
            }
            // What this save costs the occurrences the user singled out. `presenting:` so Save
            // acts on the payload the alert was raised for.
            .alert(
                L10n.event_series_warning_title(),
                isPresented: Binding(
                    get: { confirmingSave != nil }, set: { if !$0 { confirmingSave = nil } }
                ),
                presenting: confirmingSave
            ) { pending in
                Button(L10n.action_save()) {
                    confirmingSave = nil
                    onUpdate(pending.args)
                }
                Button(L10n.action_cancel(), role: .cancel) { confirmingSave = nil }
            } message: { pending in
                if let text = seriesWarningText(pending.warning) { Text(text) }
            }
        }
        // See EventDetailView: a macOS sheet takes its size from its content, and a `Form` gives it
        // none. Taller than the detail sheet because this one is a full form.
        .frame(minWidth: 380, minHeight: 460)
    }

    /// Save, straight through on a create, and on an edit of something that can only mean the
    /// whole series. An occurrence of a series is asked about first: which occurrences, and then,
    /// if the answer was all of them, what that costs.
    private func save() {
        guard editor.isEditing else {
            onCreate(editor.createArgs())
            return
        }
        guard editor.asksAboutTheSeries else {
            commitSeries(editor.updateArgs(thisOccurrenceOnly: false))
            return
        }
        askingScope = true
    }

    /// Dispatch a whole-series save, putting the warning first when there is one, the edit is
    /// the only moment anything can be done about the work it would discard.
    ///
    /// The warning is raised on the next runloop turn because it is often the *second* alert in a
    /// row: everything that reaches it through *All events* asks it while the scope alert is still
    /// going away, and SwiftUI drops a presentation requested during another one's dismissal. The
    /// hop costs nothing on the path that raises no scope question first.
    private func commitSeries(_ args: UpdateArgs) {
        guard let warning = warningFor(args) else {
            onUpdate(args)
            return
        }
        DispatchQueue.main.async {
            confirmingSave = PendingSeriesSave(args: args, warning: warning)
        }
    }

    private var calendarRow: some View {
        let row = calendars.row(account: editor.calendar?.account ?? "", calendar: editor.calendar?.id ?? "")
        return HStack(spacing: 10) {
            Circle()
                .fill(parseHexColor(row.swatchOrFallback(dark: scheme == .dark).background))
                .frame(width: 14, height: 14)
            VStack(alignment: .leading, spacing: 1) {
                Text(L10n.event_calendar()).font(.caption).foregroundStyle(.secondary)
                Text(row?.name ?? editor.calendar?.name ?? "").foregroundStyle(.primary)
            }
            Spacer()
        }
        .contentShape(Rectangle())
    }
}

/// The calendar picker, choose which calendar a new event lands in, grouped by account (a calendar
/// id is only unique within its account) and filtered to the ones we can write to.
struct CalendarPickerView: View {
    let calendars: [CalendarRow]
    let selected: CalendarChoice?
    let onPick: (CalendarChoice) -> Void

    @Environment(\.dismiss) private var dismiss
    @Environment(\.colorScheme) private var scheme

    private var byAccount: [(account: String, rows: [CalendarRow])] {
        var order: [String] = []
        var groups: [String: [CalendarRow]] = [:]
        for row in calendars where row.canWrite {
            if groups[row.account] == nil { order.append(row.account) }
            groups[row.account, default: []].append(row)
        }
        return order.map { ($0, groups[$0] ?? []) }
    }

    var body: some View {
        NavigationStack {
            List {
                ForEach(byAccount, id: \.account) { group in
                    Section(group.account) {
                        ForEach(group.rows, id: \.id) { calendar in
                            Button {
                                onPick(CalendarChoice(account: calendar.account, id: calendar.id, name: calendar.name))
                            } label: {
                                HStack(spacing: 12) {
                                    Circle()
                                        .fill(parseHexColor(calendar.color.swatch(dark: scheme == .dark).background))
                                        .frame(width: 16, height: 16)
                                    Text(calendar.name).foregroundStyle(.primary)
                                    Spacer()
                                    if selected?.account == calendar.account && selected?.id == calendar.id {
                                        Image(systemName: "checkmark").foregroundStyle(.tint)
                                    }
                                }
                                .contentShape(Rectangle())
                            }
                            .buttonStyle(.plain)
                        }
                    }
                }
            }
            .navigationTitle(L10n.event_pick_calendar())
            .toolbar {
                ToolbarItem(placement: .cancellationAction) {
                    Button(L10n.action_cancel()) { dismiss() }
                }
            }
        }
        // Same reason as the two sheets above, this one is nested inside the editor.
        .frame(minWidth: 340, minHeight: 320)
    }
}

/// The reminder summary, localised. The bucketing (pure) is `reminderBucket`.
func reminderText(_ minutes: Int32?) -> String {
    switch reminderBucket(minutes) {
    case .none: return L10n.event_reminder_none()
    case .atStart: return L10n.event_reminder_at_start()
    case .minutes(let n): return L10n.event_reminder_minutes(count: n)
    case .hours(let n): return L10n.event_reminder_hours(count: n)
    case .days(let n): return L10n.event_reminder_days(count: n)
    }
}
