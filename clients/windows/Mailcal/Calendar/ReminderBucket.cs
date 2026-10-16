// An event's reminder as a display bucket; the words are Dialogs/CalendarEventText.cs's.
namespace Allodia.Mailcal.Calendar;

/// <summary>A reminder offset, bucketed for display, pure, so a locale quirk can't reach it.</summary>
internal abstract record ReminderBucket
{
    internal sealed record None : ReminderBucket;

    internal sealed record AtStart : ReminderBucket;

    internal sealed record Minutes(int N) : ReminderBucket;

    internal sealed record Hours(int N) : ReminderBucket;

    internal sealed record Days(int N) : ReminderBucket;
}

/// <summary>The reminder summary, as a pure bucket decision (the L10n string assembly lives in the
/// dialog, which is not part of this assembly). The repeat rule's own parts are
/// <see cref="EventRepeatFormat"/>.</summary>
internal static class CalendarEventSummary
{
    /// <summary>Buckets minutes-before into the coarsest exact unit (a day, an hour, else minutes).</summary>
    internal static ReminderBucket ReminderBucketOf(int? minutes)
    {
        if (minutes is not { } m)
        {
            return new ReminderBucket.None();
        }
        if (m <= 0)
        {
            return new ReminderBucket.AtStart();
        }
        if (m % 1440 == 0)
        {
            return new ReminderBucket.Days(m / 1440);
        }
        if (m % 60 == 0)
        {
            return new ReminderBucket.Hours(m / 60);
        }
        return new ReminderBucket.Minutes(m);
    }
}
