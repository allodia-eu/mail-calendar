// Which of the core's notices this client draws where (docs/notices.md): the one prominent on the
// screen on show, how many more stand behind it, and the mark each account's row carries. The core
// has already ranked them, so every rule here keeps its order and decides nothing about rank.
//
// WinUI-free and L10n-free, so Mailcal.Tests links it: the words arrive as a parameter.

using Allodia.Mailcal.ViewModels;
using uniffi.mailcal_bindings;

namespace Allodia.Mailcal.Services;

internal static class NoticeRules
{
    /// <summary>The screens a notice can name, for the shell's destination.</summary>
    internal static NoticeScreens ScreenOf(AppDestination destination) => destination switch
    {
        AppDestination.Calendar => NoticeScreens.Calendar,
        AppDestination.Contacts => NoticeScreens.Contacts,
        _ => NoticeScreens.Mail,
    };

    /// <summary>Whether <paramref name="notice"/> is drawn on <paramref name="screen"/>.</summary>
    internal static bool Belongs(Notice notice, NoticeScreens screen) =>
        notice.Screens == NoticeScreens.Everywhere || notice.Screens == screen;

    /// <summary>The notice drawn in the bar on <paramref name="screen"/>: the first, in the core's
    /// order, that belongs there (rule 2). None when nothing does.</summary>
    internal static Notice? Prominent(IReadOnlyList<Notice> notices, NoticeScreens screen) =>
        notices.FirstOrDefault(notice => Belongs(notice, screen));

    /// <summary>How many notices besides the bar's are in force, which the bar's "more" entry
    /// counts. None while the screen has no bar: an account's notices are on its row then.</summary>
    internal static int More(IReadOnlyList<Notice> notices, NoticeScreens screen) =>
        Prominent(notices, screen) is null ? 0 : notices.Count - 1;

    /// <summary>
    /// The mark each account with a notice carries in the folder pane (rule 6): the glyph of its
    /// first notice, which the core ranked most severe, and one short line beside it, that
    /// notice's mark, or how many there are when there are several (<paramref name="count"/>).
    /// Every one of the account's sentences, in order, is the detail a screen reader hears.
    /// </summary>
    internal static IReadOnlyDictionary<string, SidebarMark> Marks(
        IReadOnlyList<Notice> notices, Func<NoticeText, string> words, Func<int, string> count)
    {
        var marks = new Dictionary<string, SidebarMark>();
        foreach (var group in notices.Where(notice => notice.Account is not null).GroupBy(notice => notice.Account!))
        {
            var all = group.ToList();
            var first = all[0];
            marks[group.Key] = new SidebarMark(
                ToneOf(first.Severity),
                all.Count > 1 ? count(all.Count) : words(first.Mark ?? first.Title),
                string.Join("\n", all.Select(notice => words(notice.Title))));
        }
        return marks;
    }

    /// <summary>The pane's mirror of a severity, which a public view model can carry.</summary>
    internal static SidebarMarkTone ToneOf(NoticeSeverity severity) => severity switch
    {
        NoticeSeverity.Blocked => SidebarMarkTone.Blocked,
        NoticeSeverity.Degraded => SidebarMarkTone.Degraded,
        _ => SidebarMarkTone.Informational,
    };

    /// <summary>
    /// Whether a sign-in started from a notice is a failed remedy (rule 10): it ended without
    /// signing in and with something to say, and the browser answered. A sign-in whose wait for
    /// the browser ran out is not one: closing the tab is all it takes to get there, and a closed
    /// browser is not a failure.
    /// </summary>
    internal static bool RemedyFailed(bool signedIn, string? error, bool unanswered) =>
        !signedIn && error is not null && !unanswered;

    /// <summary>
    /// What the core runs for a notice's action, or null for a flow this client runs itself. A
    /// Retry is the core's: it refreshes the notice's screen, which dials the account again, and
    /// marks the notice when that dial ends with the notice still standing, which only it can tell
    /// (rules 7 and 10). So the client never reports a Retry as failed.
    /// </summary>
    internal static NoticeIntent? CoreRemedy(Notice notice) =>
        notice.Action == NoticeAction.Retry ? new NoticeIntent.Retry(notice.Id) : null;

    /// <summary>Whether a notice has something to run, which only an action label can say: the
    /// core sends both or neither.</summary>
    internal static bool Offers(Notice notice) => notice.Action is not null && notice.ActionLabel is not null;

    /// <summary>The accounts whose stored sign-in the server refuses, in the core's order, which an
    /// account's page in Settings draws its sign-in section for. A grant that is merely narrower
    /// is not one: the account still signs in.</summary>
    internal static IReadOnlyList<string> SignInExpired(IReadOnlyList<Notice> notices) =>
        AccountsOf<NoticeKind.SignInExpired>(notices);

    /// <summary>Whether the account service refused the Allodia sign-in, which the Allodia card
    /// draws.</summary>
    internal static bool AllodiaSignInEnded(IReadOnlyList<Notice> notices) =>
        notices.Any(notice => notice.Kind is NoticeKind.AllodiaSignInEnded);

    /// <summary>Whether the device has no network, which the folder pane's connection status
    /// says.</summary>
    internal static bool Offline(IReadOnlyList<Notice> notices) =>
        notices.Any(notice => notice.Kind is NoticeKind.Offline);

    /// <summary>The accounts that cannot reach their server while the device is online, in the
    /// core's order, which the folder pane's connection status names. None while offline: the
    /// device is the fault then.</summary>
    internal static IReadOnlyList<string> Unreachable(IReadOnlyList<Notice> notices) =>
        Offline(notices) ? [] : AccountsOf<NoticeKind.Unreachable>(notices);

    private static List<string> AccountsOf<TKind>(IReadOnlyList<Notice> notices)
        where TKind : NoticeKind =>
        notices.Where(notice => notice.Kind is TKind && notice.Account is not null)
            .Select(notice => notice.Account!)
            .Distinct()
            .ToList();

    /// <summary>What two snapshots differ in for a reader of them, so a signal that changed
    /// nothing redraws nothing.</summary>
    internal static string Fingerprint(IEnumerable<Notice> notices) =>
        string.Join("|", notices.Select(notice =>
            $"{notice.Id}:{notice.RemedyFailed}:{notice.Action}:{Text(notice.Title)}:{Text(notice.Detail)}"));

    private static string Text(NoticeText? text) => text is null
        ? string.Empty
        : text.Key + "(" + string.Join(",", text.Args.OrderBy(arg => arg.Key, StringComparer.Ordinal)
            .Select(arg => arg.Key + "=" + arg.Value)) + ")";
}

/// <summary>
/// Which notices are new since the last snapshot, so each is announced to assistive technology
/// once, when its id first appears (rule 12), and never again on a redraw.
/// </summary>
internal sealed class NoticeAnnouncements
{
    private HashSet<string> _inForce = [];

    /// <summary>The notices in <paramref name="notices"/> that were not in force last time. A
    /// notice that goes and comes back is new again.</summary>
    internal IReadOnlyList<Notice> Arrived(IReadOnlyList<Notice> notices)
    {
        var arrived = notices.Where(notice => !_inForce.Contains(notice.Id)).ToList();
        _inForce = notices.Select(notice => notice.Id).ToHashSet();
        return arrived;
    }
}
