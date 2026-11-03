// An event's video meetings: the decisions behind the detail's meeting section and the camera on
// every event row, with no WinUI and no catalog in them, so Mailcal.Tests can drive them
// (docs/conferencing.md). The twin of Apple's EventConferenceView helpers; the section itself is
// Dialogs/EventConferenceSection.cs.
//
// Which meetings exist, which link Join opens and which numbers are dialled is the core's
// (`EventDetail.Conferences`): `JoinUrl` and a dial-in's `Uri` have already passed the link
// policy, so they are opened as they are.
using System;
using uniffi.mailcal_bindings;

namespace Allodia.Mailcal.Calendar;

/// <summary>The meeting section's decisions, and the camera every event row carries.</summary>
internal static class EventMeeting
{
    /// <summary>Segoe Fluent Icons "Video": the meeting's heading, and the mark on an event that has one.</summary>
    internal const string CameraGlyph = "";

    /// <summary>Segoe Fluent Icons "Phone", beside a dial-in number.</summary>
    internal const string PhoneGlyph = "";

    /// <summary>Segoe Fluent Icons "Copy", on Copy link.</summary>
    internal const string CopyGlyph = "";

    /// <summary>
    /// The meeting's heading: the service's own name, a brand the same in every language, or the
    /// catalog's words for a video meeting (<paramref name="videoMeeting"/>) when nothing names one.
    /// </summary>
    internal static string Title(EventConference conference, string videoMeeting) =>
        string.IsNullOrEmpty(conference.Service) ? videoMeeting : conference.Service;

    /// <summary>
    /// The link Join opens, or <c>null</c> when there is nothing to join. The core's
    /// <c>JoinUrl</c>, which has already passed the link policy and is set only for a meeting that
    /// is ready, so nothing more is decided here than whether it parses.
    /// </summary>
    internal static Uri? JoinUri(EventConference conference) =>
        Uri.TryCreate(conference.JoinUrl, UriKind.Absolute, out var uri) ? uri : null;

    /// <summary>
    /// The <c>tel:</c> URI a dial-in row opens: the core's, through the same policy, or
    /// <c>null</c> for a number it refused, which the row then shows as text.
    /// </summary>
    internal static Uri? DialInUri(ConferenceDialIn number) =>
        Uri.TryCreate(number.Uri, UriKind.Absolute, out var uri) ? uri : null;

    /// <summary>
    /// What the section says in place of Join while the server is still making the meeting
    /// (<paramref name="preparing"/>), or after it could not (<paramref name="failed"/>);
    /// <c>null</c> for a meeting that is ready.
    /// </summary>
    internal static string? Status(EventConference conference, string preparing, string failed) =>
        conference.State switch
        {
            ConferenceState.Preparing => preparing,
            ConferenceState.Failed => failed,
            _ => null,
        };

    /// <summary>Whether a row for <paramref name="value"/> is drawn: one with no value is left out.</summary>
    internal static bool Shows(string? value) => !string.IsNullOrEmpty(value);

    /// <summary>
    /// An event's spoken label with "video meeting" (<paramref name="videoMeeting"/>) after it when
    /// it has one: the camera is hidden from a screen reader, so the label says what it shows.
    /// </summary>
    internal static string Spoken(string label, string videoMeeting, bool hasConference) =>
        hasConference ? label + ", " + videoMeeting : label;
}
