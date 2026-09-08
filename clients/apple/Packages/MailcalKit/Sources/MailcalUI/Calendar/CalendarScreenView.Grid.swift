// The time grid's half of the calendar screen: the geometry a zoom implies, the weeks the strip is
// showing, and what a scroll does to it. Split out of CalendarScreenView.swift to keep it under 500
// lines.
//
// **The screen owns the strip; the grid only reports what the pointer did.** One owner for the
// pointer stream is the rule the whole gesture layer is built on (docs/calendar.md §6), and it is
// what a delta-reporting grid buys: a pan, a flick, a wheel notch, a pinch and a jump home all move
// the same one number, so none of them can disagree with the others about where the grid is.

import MailcalBindings
import SwiftUI

extension CalendarScreenView {

    /// The grid, at the size the window currently gives it.
    @ViewBuilder
    func gridContent(pager: CalendarPager, calendar: Calendar, today: Date) -> some View {
        GeometryReader { geometry in
            let dayViewport = max(geometry.size.width - calendarGutter, 1)
            let hourHeight = zoom.hourHeight(viewport: geometry.size.height)
            let dayWidth = zoom.dayWidth(viewport: dayViewport)
            let lanes = bannerLanes(
                pager: pager, calendar: calendar, dayViewport: dayViewport, dayWidth: dayWidth
            )
            let maxHour = calendarMaxHourOffset(
                hourHeight: hourHeight,
                gridHeight: calendarGridHeight(viewportHeight: geometry.size.height, lanes: lanes)
            )
            CalendarGridView(
                weekAt: { stripWeek($0, pager: pager, calendar: calendar) },
                calendarVersion: model.calendarVersion,
                strip: strip,
                today: today,
                nowMinutes: nowMinutes(calendar: calendar),
                use24Hour: model.use24Hour,
                hourHeight: hourHeight,
                dayWidth: dayWidth,
                calendar: calendar,
                dayViewport: dayViewport,
                viewportHeight: geometry.size.height,
                hourOffset: hourOffset,
                onScroll: { dx, dy in scroll(dx, dy, dayWidth: dayWidth, maxHour: maxHour) },
                onScrollEnded: { dx, dy in coast(dx, dy, dayWidth: dayWidth, maxHour: maxHour) },
                onOpen: open,
                canCreateEvent: calendarSupportsNewEvent(
                    currentCalendars(
                        pager: pager, anchor: pager.anchor(forPage: strip.anchorWeek)
                    )
                ),
                onDrop: { week, drag in drop(drag, week: week, pager: pager) }
            )
            .modifier(
                CalendarZoomGesture(
                    zoom: $zoom,
                    strip: $strip,
                    hourOffset: $hourOffset,
                    viewportWidth: dayViewport,
                    viewportHeight: geometry.size.height,
                    onSettled: settleZoom
                )
            )
            // **The offsets have to be put back inside the content when the content changes size.**
            //
            // They are held by hand rather than by a `ScrollView`, and every other clamp lives inside
            // a *gesture*, so until the user touched the grid an offset could stay parked outside the
            // content it addresses. What is left on screen then looks like a calendar that failed to
            // load: the gutter, the all-day band, and nothing else.
            //
            // Three things resize the content under a settled offset, and **none of them is a
            // gesture**: the **window** (shrink it and the hour height shrinks with it), the **zoom
            // arriving late** (the horizon and column count are persisted core settings, so a client
            // that renders before they land frames against the defaults and then re-seeds), and the
            // **all-day banner** growing a lane.
            //
            // The day axis needs no such net any more: a strip has no end to fall off, and its anchor
            // re-derives itself from its position. This is the Apple half of docs/calendar.md §1.
            .onChange(of: maxHour) { _, limit in
                hourOffset = hourOffset.clamped(to: 0...limit)
            }
            // **The grid keeps framing itself on now until the user moves it.**
            //
            // Not "frame once, on the first pass". SwiftUI lays this out several times before the
            // window has settled, and the first pass here measures 620x59: an hour is five points
            // tall, so the offset that frames on 09:15 is 37pt, and the grid opens at 01:00 when the
            // real 759x479 arrives a frame later. Any test for "is this viewport real yet" is a
            // guess at a number only the window knows.
            //
            // So the seat is not a moment, it is a **state**: the grid belongs to the app until a
            // hand takes it (`gridHeld`), and every layout before that re-frames. This is also what
            // covers the zoom landing late, which the offsets contract names as its own trap: a
            // persisted horizon arriving after the first frame changes the hour height under an
            // offset measured against the old one.
            // A seat that earns a framing: a shape from the menu, a week-start change, and "back to
            // today". Never a zoom, and never a sync landing.
            .onChange(of: recentreToken) { _, _ in
                recentre(
                    pager: pager, calendar: calendar, today: today,
                    dayWidth: dayWidth, hourHeight: hourHeight, maxHour: maxHour
                )
            }
        }
        // Measured on the `GeometryReader`'s own box, and by `onGeometryChange` rather than by an
        // `onChange(of: geometry.size)` inside it. Both halves of that were wrong once and each
        // failed quietly:
        //
        // - `onChange(of: geometry.size)` never fired for the resize that mattered. The body
        //   evaluated five times at 1119x827 while that comparison still held 759x479, so the grid
        //   drew a 69pt hour against an offset framed for a 40pt one and opened three hours early. A
        //   `GeometryProxy` read inside a body is not a value SwiftUI tracks; this modifier is.
        // - Attached *inside*, it measures the grid's own frame, which is far taller than the box it
        //   is laid out in (the hour ruler's natural height is a whole day: 1778pt against 827pt).
        //   Framing against that put the grid at the bottom of the day, at 17:00.
        .onGeometryChange(for: CGSize.self) { $0.size } action: { size in
            frameOnToday(pager: pager, calendar: calendar, today: today, size: size)
        }
    }

    /// Frames the grid on today, for as long as the grid is still the app's to frame.
    private func frameOnToday(
        pager: CalendarPager, calendar: Calendar, today: Date, size: CGSize
    ) {
        guard !gridHeld, size.width > calendarGutter, size.height > 1 else { return }
        let dayViewport = size.width - calendarGutter
        let hourHeight = zoom.hourHeight(viewport: size.height)
        let dayWidth = zoom.dayWidth(viewport: dayViewport)
        let lanes = bannerLanes(
            pager: pager, calendar: calendar, dayViewport: dayViewport, dayWidth: dayWidth
        )
        recentre(
            pager: pager,
            calendar: calendar,
            today: today,
            dayWidth: dayWidth,
            hourHeight: hourHeight,
            maxHour: calendarMaxHourOffset(
                hourHeight: hourHeight,
                gridHeight: calendarGridHeight(viewportHeight: size.height, lanes: lanes)
            )
        )
    }

    /// The week at `index`, or nil when the core has no page for it.
    func stripWeek(_ index: Int, pager: CalendarPager, calendar: Calendar) -> CalendarStripWeek? {
        let anchor = pager.anchor(forPage: index)
        guard let page = model.calendarPage(from: anchor, columns: daysInWeek) else { return nil }
        return CalendarStripWeek(
            index: index,
            page: page,
            days: page.days.compactMap { parseISODate($0.date, calendar: calendar) }
        )
    }

    /// The largest lane count across the weeks on screen, which is the banner's height for all of
    /// them: with the ruler pinned there is one content top for the whole surface.
    private func bannerLanes(
        pager: CalendarPager, calendar: Calendar, dayViewport: CGFloat, dayWidth: CGFloat
    ) -> Int {
        strip.visibleWeeks(dayViewport: dayViewport, dayWidth: dayWidth)
            .compactMap { stripWeek($0, pager: pager, calendar: calendar) }
            .map { Int($0.page.allDayLanes) }
            .max() ?? 0
    }

    // MARK: - Scrolling

    /// A pointer moved the content: the strip takes the sideways half, the hours the rest.
    private func scroll(_ dx: CGFloat, _ dy: CGFloat, dayWidth: CGFloat, maxHour: CGFloat) {
        guard dx != 0 || dy != 0 else { return }
        // The grid is the user's from here: a window resize must move the days they chose, not
        // re-frame the calendar on now underneath them.
        gridHeld = true
        if dx != 0 { strip.pan(dx, dayWidth: dayWidth) }
        if dy != 0 { hourOffset = (hourOffset - dy).clamped(to: 0...maxHour) }
    }

    /// The pointer let go: spend what momentum is left, then land the day axis on a **day**.
    ///
    /// There is no threshold here and nothing to judge. The strip goes as far as the coast is worth
    /// and comes to rest on whichever day it ends nearest, so a second flick arriving mid-coast
    /// simply adds its own, which is what a hand expects, and there is no banked decision for a later
    /// event to disagree with.
    private func coast(_ dx: CGFloat, _ dy: CGFloat, dayWidth: CGFloat, maxHour: CGFloat) {
        var landed = strip
        landed.pan(dx, dayWidth: dayWidth)
        landed.weeks = landed.nearestDay
        let hours = (hourOffset - dy).clamped(to: 0...maxHour)
        let travel = max(
            abs(landed.weeks - strip.weeks) * dayWidth * CGFloat(daysInWeek),
            abs(hours - hourOffset)
        )
        withAnimation(.easeOut(duration: calendarCoastDuration(points: travel))) {
            strip = landed
            hourOffset = hours
        }
    }

    /// Frames the grid on the **origin week**, and scrolls to just before now.
    ///
    /// Always week 0, because every seat that calls this has already put its target there: opening
    /// and "back to today" jump the origin to today's week, and a shape from the menu re-origins on
    /// the week the user was reading. Chasing *today's* week here instead is a bug with a very
    /// plausible face: switching Week to Day while browsing next month would teleport you home,
    /// which is exactly what a shape change must not do (docs/calendar.md §3, "alignment is applied
    /// on a seat").
    ///
    /// Which column is a shared product decision (`calendarFramingColumn`): the two wide shapes open
    /// on the week's first day, the two narrow ones on today. Today's column is offered **only when
    /// today is in the week being framed**; anywhere else there is no such column and the week opens
    /// on its first day. With the strip there is no clamp left to fix a wrong answer, so the helper
    /// has to be given the right question.
    private func recentre(
        pager: CalendarPager,
        calendar: Calendar,
        today: Date,
        dayWidth: CGFloat,
        hourHeight: CGFloat,
        maxHour: CGFloat
    ) {
        guard dayWidth > 0 else { return }
        // A jump home hands the grid back: it frames itself again until the next hand takes it.
        gridHeld = false
        let origin = pager.anchor(forPage: 0)
        let column = calendar.dateComponents(
            [.day], from: origin, to: calendar.startOfDay(for: today)
        ).day
        var framed = strip
        framed.frame(
            week: 0,
            column: calendarFramingColumn(
                mode: pager.mode, todayColumn: calendarTodayColumn(daysFromWeekStart: column)
            )
        )
        let hours = (CGFloat(max(nowMinutes(calendar: calendar) - 90, 0)) / 60 * hourHeight)
            .clamped(to: 0...maxHour)
        // **A jump home that is a journey cuts; one that is a step slides.** The grid is animatable
        // on the strip position, so an animated jump draws every week in between: from three months
        // out that is a strobe, not a transition, and it pulls a page per frame to draw it. Sliding
        // is worth having only where the eye can follow, which is about a week.
        if abs(framed.weeks - strip.weeks) > 1 {
            strip = framed
            hourOffset = hours
        } else {
            withAnimation(.easeOut(duration: 0.2)) {
                strip = framed
                hourOffset = hours
            }
        }
    }
}
