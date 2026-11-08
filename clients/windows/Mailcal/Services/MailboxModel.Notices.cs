// The core's notices (docs/notices.md): pulled after Surface.Notices, drawn by the window's one
// bar, each account's row in the folder pane and the account's page in Settings, and each action
// run through a flow this client already has, or by the core for a Retry. Which notice is drawn
// where is NoticeRules.

using System.Threading.Tasks;
using Allodia.Mailcal.ViewModels;
using uniffi.mailcal_bindings;

namespace Allodia.Mailcal.Services;

public sealed partial class MailboxModel
{
    private IReadOnlyList<Notice> _notices = [];
    private IReadOnlyDictionary<string, SidebarMark> _marks = new Dictionary<string, SidebarMark>();
    private string _noticesDrawn = string.Empty;

    /// <summary>Every notice in force, in the core's order.</summary>
    internal IReadOnlyList<Notice> Notices => _notices;

    /// <summary>Raised on the UI thread when the notices in force, or what one says, changed.</summary>
    public event Action? NoticesChanged;

    /// <summary>Raised with an account's id when a notice asks for its page in Settings → Accounts;
    /// the window opens the dialog, which needs its XamlRoot.</summary>
    public event Action<string?>? AccountSettingsRequested;

    /// <summary>Raised with an account's id when a notice asks to review a server change; the
    /// window opens the question, which needs its XamlRoot.</summary>
    public event Action<string>? ServerChangeRequested;

    /// <summary>The mark <paramref name="accountId"/>'s row carries, or null without a notice.</summary>
    public SidebarMark? MarkOf(string accountId) => _marks.GetValueOrDefault(accountId);

    /// <summary>A notice's words, through the lookup generated from the catalog.</summary>
    internal static string Words(NoticeText text) => L10n.Notice(text.Key, text.Args);

    private void PullNotices()
    {
        if (_app is null)
        {
            return;
        }
        var notices = _app.Notices().Notices;
        UpdateConnectionStatus(notices);
        // Settings → Accounts on every signal, not only when a notice's words moved: an account's
        // page says when a use did not connect, from the accounts snapshot, and redraws only what
        // changed. Its top carries the account's notices.
        RaiseAccountsChanged();
        var drawn = NoticeRules.Fingerprint(notices);
        if (drawn == _noticesDrawn)
        {
            return;
        }
        _noticesDrawn = drawn;
        _notices = notices;
        Log.Info($"notices: {notices.Length} in force");
        _marks = NoticeRules.Marks(notices, Words, L10n.NoticesAccountCount);
        NoticesChanged?.Invoke();
    }

    /// <summary>Closes a notice the core offered to close.</summary>
    internal void DismissNotice(Notice notice)
    {
        if (notice.Dismissible)
        {
            _app?.Dispatch(new Intent.Notices(new NoticeIntent.Dismiss(notice.Id)));
        }
    }

    /// <summary>Runs <paramref name="notice"/>'s action: a Retry through the core, which judges
    /// it, and every other through the flow this client already has (rule 7), reporting one that
    /// did not complete (rule 10).</summary>
    internal void RunNoticeAction(Notice notice)
    {
        if (NoticeRules.CoreRemedy(notice) is { } core)
        {
            _app?.Dispatch(new Intent.Notices(core));
            return;
        }
        switch (notice.Action)
        {
            case NoticeAction.SignInAgain when notice.Account is { } account:
                _ = ReportUnlessDoneAsync(notice, SignInAgainFromNoticeAsync(notice.Kind, account));
                break;
            case NoticeAction.SignInToAllodia:
                _ = ReportUnlessDoneAsync(notice, SignInToAllodiaFromNoticeAsync());
                break;
            case NoticeAction.OpenAccountSettings:
                AccountSettingsRequested?.Invoke(notice.Account);
                break;
            case NoticeAction.Review when notice.Account is { } account:
                ServerChangeRequested?.Invoke(account);
                break;
            default:
                break;
        }
    }

    /// <summary>
    /// The account's sign-in again, in place: it keeps its id, mail and settings. A grant that
    /// withholds mail or the calendar is widened at its provider; a dead sign-in runs whatever
    /// flow the account signs in by.
    /// </summary>
    /// <returns>Whether it failed. A browser the person closed, or never came back from, is not a
    /// failure.</returns>
    private async Task<bool> SignInAgainFromNoticeAsync(NoticeKind kind, string accountId)
    {
        var remedy = kind is NoticeKind.MailReauth or NoticeKind.CalendarReauth
            ? SignInRemedy.ProviderConsent
            : AccountSettingsRules.Remedy(
                _app?.AccountProvider(accountId),
                AccountsSnapshot().Accounts.Any(entry => entry.Id == accountId && entry.Nextcloud));
        switch (remedy)
        {
            case SignInRemedy.Nextcloud:
                // Nothing to refresh: a new app password, through the browser.
                return Failed(await AccountNextcloudSignInAsync(accountId));
            case SignInRemedy.ProviderConsent:
                return Failed(await SignInAgainAsync(accountId, []));
            case SignInRemedy.JmapReauth:
                // Addressed to the account id, not the address: the core re-authorises this
                // account's own stored grant, so there is no discovery and no second registration.
                return await ReconnectJmapAsync(accountId);
            case SignInRemedy.AccountSignIn:
                return Failed(await AccountSignInAsync(accountId));
            default:
                // No browser flow to run: the password is entered again on the account's page.
                AccountSettingsRequested?.Invoke(accountId);
                return false;
        }
    }

    private static bool Failed(ConsentEnded ended) =>
        NoticeRules.RemedyFailed(ended.SignedIn, ended.Error, ended.Unanswered);

    private async Task ReportUnlessDoneAsync(Notice notice, Task<bool> remedy)
    {
        if (await remedy)
        {
            _ui.TryEnqueue(() => _app?.Dispatch(new Intent.Notices(new NoticeIntent.RemedyFailed(notice.Id))));
        }
    }
}
