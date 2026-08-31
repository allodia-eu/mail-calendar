// Which top-level surface the shell is showing.
//
// This used to be a bool per screen (`ShowingCalendar`). With three destinations a boolean each
// admits states that cannot exist, "the calendar and contacts at once", and every reader has to
// prove they don't happen. An enum makes the invalid states unrepresentable instead, which is the
// same move the Android and Apple clients made for the same reason when Contacts arrived.

namespace Allodia.Mailcal.Services;

/// <summary>The shell's top-level surfaces. Exactly one is on screen at a time.</summary>
public enum AppDestination
{
    /// <summary>The mailbox: the message list beside the reading pane (or the composer).</summary>
    Mail,

    /// <summary>The calendar grid / agenda.</summary>
    Calendar,

    /// <summary>The contacts list beside one person's detail.</summary>
    Contacts,
}
