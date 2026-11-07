// The notices about an account, at the top of its page (docs/notices.md rule 6). Said here, not
// acted on: the page's own sections below are where each remedy is, and a Review from here would
// open a second dialog over this one, which WinUI does not allow.

using Allodia.Mailcal.Services;
using Allodia.Mailcal.Views;
using Microsoft.UI.Xaml.Automation;
using Microsoft.UI.Xaml.Controls;

namespace Allodia.Mailcal.Dialogs;

public sealed partial class SettingsDialog
{
    /// <summary>Adds one bar per notice about <paramref name="accountId"/>, in the core's order.</summary>
    private void AddAccountNotices(StackPanel panel, string accountId)
    {
        foreach (var notice in _model.Notices.Where(notice => notice.Account == accountId))
        {
            var bar = new InfoBar
            {
                IsOpen = true,
                IsClosable = notice.Dismissible,
                Severity = NoticeBar.SeverityOf(notice.Severity),
                Title = MailboxModel.Words(notice.Title),
                Message = NoticeBar.Beneath(notice) ?? string.Empty,
            };
            AutomationProperties.SetAutomationId(bar, "AccountNotice");
            bar.CloseButtonClick += (_, _) => _model.DismissNotice(notice);
            panel.Children.Add(bar);
        }
    }

    /// <summary>What the page's notices are drawn from, so a change to them redraws it.</summary>
    private IEnumerable<string> AccountNoticesFingerprint() =>
        [ "notices:" + NoticeRules.Fingerprint(_model.Notices.Where(notice => notice.Account is not null)) ];
}
