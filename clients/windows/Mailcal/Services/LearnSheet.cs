// The learn sheet's state (docs/ai.md, "Learning"): its pages, which account, which sent mail, and
// the report the consent is given against. WinUI-free so Mailcal.Tests can pin the rules that are
// silent when wrong: a report that arrives after the person changed the account or the range must
// not stand for the new choice, since consent is given against what it says; and "up to a date"
// takes in the whole of that day, in the zone the app shows dates in.

using System;
using System.Collections.Generic;
using uniffi.mailcal_bindings;

namespace Allodia.Mailcal.Services;

/// <summary>A page of the learn sheet.</summary>
internal enum LearnStep
{
    /// <summary>Which account to learn from.</summary>
    Account,

    /// <summary>Which of its sent mail.</summary>
    Range,

    /// <summary>What the device found, and what is sent where; Learn is the consent.</summary>
    Consent,

    /// <summary>The run, reached only by the consent.</summary>
    Progress,
}

/// <summary>What the learn sheet has chosen, and what the device found for that choice.</summary>
internal sealed class LearnSheet
{
    // Which read is the current one; a read that finishes under an older number is dropped.
    private int _read;

    /// <summary>Opens the sheet on <paramref name="account"/>, with everything on the device.</summary>
    /// <param name="account">The account to learn from.</param>
    /// <param name="today">The date the "up to a date" picker opens on.</param>
    /// <param name="accounts">How many accounts there are to choose from.</param>
    internal LearnSheet(string account, DateOnly today, int accounts)
    {
        Account = account;
        UntilDay = today;
        Steps = StepsFor(accounts);
        Pager = new WizardPager(Steps.Count);
    }

    /// <summary>The sheet's pages.</summary>
    internal IReadOnlyList<LearnStep> Steps { get; }

    /// <summary>The page on screen.</summary>
    internal WizardPager Pager { get; }

    /// <summary>The step on screen.</summary>
    internal LearnStep Step => Steps[Pager.Index];

    /// <summary>The account to learn from.</summary>
    internal string Account { get; set; }

    /// <summary>Whether the range stops at <see cref="UntilDay"/>; otherwise it is everything.</summary>
    internal bool UpToDate { get; set; }

    /// <summary>The last day the range takes in, when <see cref="UpToDate"/>.</summary>
    internal DateOnly UntilDay { get; set; }

    /// <summary>Whether the device is reading the Sent folder for the current choice.</summary>
    internal bool Reading { get; private set; }

    /// <summary>What the device found for the current choice, once read.</summary>
    internal CorpusReport? Report { get; private set; }

    /// <summary>Why the current choice could not be read.</summary>
    internal AiFailure? Failure { get; private set; }

    /// <summary>
    /// Whether consent can be given: the report for the current choice is in and has something to
    /// learn from.
    /// </summary>
    internal bool CanLearn => !Reading && Report is { Usable: > 0 };

    /// <summary>Whether the sheet asks which account; with one there is nothing to choose.</summary>
    internal static bool AsksForAccount(int accounts) => accounts > 1;

    /// <summary>The pages for <paramref name="accounts"/>: the account only when there is a
    /// choice, and the run last.</summary>
    internal static LearnStep[] StepsFor(int accounts) =>
        AsksForAccount(accounts)
            ? [LearnStep.Account, LearnStep.Range, LearnStep.Consent, LearnStep.Progress]
            : [LearnStep.Range, LearnStep.Consent, LearnStep.Progress];

    /// <summary>The range's newest instant in Unix seconds, or <c>null</c> for everything.</summary>
    internal long? Until(TimeZoneInfo zone) => UpToDate ? EndOfDay(UntilDay, zone) : null;

    /// <summary>Starts a read for the current choice, and returns its number.</summary>
    internal int BeginRead()
    {
        Reading = true;
        Report = null;
        Failure = null;
        return ++_read;
    }

    /// <summary>
    /// Takes the answer to read <paramref name="read"/>, when it is still the current one. Returns
    /// whether it was.
    /// </summary>
    internal bool Finish(int read, CorpusReport? report, AiFailure? failure)
    {
        if (read != _read)
        {
            return false;
        }
        Reading = false;
        Report = report;
        Failure = failure;
        return true;
    }

    /// <summary>
    /// The last second of <paramref name="day"/> in <paramref name="zone"/>, in Unix seconds. The
    /// core's range is inclusive, so this is one second before the next day begins.
    /// </summary>
    /// <remarks>
    /// Where a zone skips its midnight, the offset of the missing hour is the standard one, which
    /// is the instant the clock jumped: the next day's real beginning.
    /// </remarks>
    internal static long EndOfDay(DateOnly day, TimeZoneInfo zone)
    {
        var next = day.AddDays(1).ToDateTime(TimeOnly.MinValue, DateTimeKind.Unspecified);
        return new DateTimeOffset(next, zone.GetUtcOffset(next)).ToUnixTimeSeconds() - 1;
    }
}
