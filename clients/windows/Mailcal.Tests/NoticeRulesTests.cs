// Which of the core's notices the Windows client draws where (docs/notices.md rules 2, 6 and 12).
// The core has ranked them; every rule here keeps that order.

using Allodia.Mailcal.Services;
using Allodia.Mailcal.ViewModels;
using uniffi.mailcal_bindings;
using Xunit;

namespace Allodia.Mailcal.Tests;

public class NoticeRulesTests
{
    private static Notice Make(
        string id,
        string screens,
        string severity = nameof(NoticeSeverity.Degraded),
        string? account = null,
        bool dismissible = false,
        bool remedyFailed = false,
        bool action = true) =>
        new(
            Id: id,
            Kind: new NoticeKind.Unreachable(),
            Title: new NoticeText("notice_" + id, new Dictionary<string, string> { ["address"] = account ?? "" }),
            Detail: null,
            Severity: Enum.Parse<NoticeSeverity>(severity),
            Screens: Enum.Parse<NoticeScreens>(screens),
            Account: account,
            Action: action ? NoticeAction.Retry : null,
            ActionLabel: action ? new NoticeText("notice_action_retry", []) : null,
            Dismissible: dismissible,
            RemedyFailed: remedyFailed);

    private static string Words(NoticeText text) =>
        text.Key + (text.Args.TryGetValue("address", out var address) && address.Length > 0 ? ":" + address : "");

    [Theory]
    [InlineData(nameof(AppDestination.Mail), nameof(NoticeScreens.Mail))]
    [InlineData(nameof(AppDestination.Calendar), nameof(NoticeScreens.Calendar))]
    [InlineData(nameof(AppDestination.Contacts), nameof(NoticeScreens.Contacts))]
    public void Each_destination_is_the_screen_of_its_name(string destination, string screen) =>
        Assert.Equal(Enum.Parse<NoticeScreens>(screen), NoticeRules.ScreenOf(Enum.Parse<AppDestination>(destination)));

    [Fact]
    public void The_bar_draws_the_first_notice_that_belongs_to_the_screen_in_the_cores_order()
    {
        Notice[] notices =
        [
            Make("calendar", nameof(NoticeScreens.Calendar), nameof(NoticeSeverity.Blocked)),
            Make("mail", nameof(NoticeScreens.Mail)),
            Make("everywhere", nameof(NoticeScreens.Everywhere), nameof(NoticeSeverity.Informational)),
        ];

        Assert.Equal("calendar", NoticeRules.Prominent(notices, NoticeScreens.Calendar)?.Id);
        // Not the Blocked one: it belongs to another screen. Never re-ranked by severity here.
        Assert.Equal("mail", NoticeRules.Prominent(notices, NoticeScreens.Mail)?.Id);
        // A notice for every screen is drawn where nothing of the screen's own comes first.
        Assert.Equal("everywhere", NoticeRules.Prominent(notices, NoticeScreens.Contacts)?.Id);
    }

    [Fact]
    public void A_screen_with_nothing_of_its_own_has_no_bar_and_no_more_entry()
    {
        Notice[] notices = [Make("mail", nameof(NoticeScreens.Mail), account: "a")];

        Assert.Null(NoticeRules.Prominent(notices, NoticeScreens.Calendar));
        Assert.Equal(0, NoticeRules.More(notices, NoticeScreens.Calendar));
        Assert.Null(NoticeRules.Prominent([], NoticeScreens.Mail));
        Assert.Equal(0, NoticeRules.More([], NoticeScreens.Mail));
    }

    [Fact]
    public void More_counts_every_other_notice_in_force_whatever_screen_it_belongs_to()
    {
        Notice[] notices =
        [
            Make("one", nameof(NoticeScreens.Mail)),
            Make("two", nameof(NoticeScreens.Calendar)),
            Make("three", nameof(NoticeScreens.Everywhere)),
        ];

        Assert.Equal(2, NoticeRules.More(notices, NoticeScreens.Mail));
        Assert.Equal(0, NoticeRules.More([notices[0]], NoticeScreens.Mail));
    }

    [Fact]
    public void Each_account_with_a_notice_is_marked_by_its_first_and_lists_them_all()
    {
        Notice[] notices =
        [
            Make("expired", nameof(NoticeScreens.Everywhere), nameof(NoticeSeverity.Blocked), account: "a"),
            Make("offline", nameof(NoticeScreens.Everywhere), nameof(NoticeSeverity.Informational)),
            Make("calendar", nameof(NoticeScreens.Calendar), account: "b"),
            Make("outage", nameof(NoticeScreens.Calendar), account: "a"),
        ];

        var marks = NoticeRules.Marks(notices, Words);

        Assert.Equal(["a", "b"], marks.Keys.Order());
        Assert.Equal(SidebarMarkTone.Blocked, marks["a"].Tone);
        Assert.Equal("notice_expired:a", marks["a"].Label);
        Assert.Equal("notice_expired:a\nnotice_outage:a", marks["a"].Detail);
        Assert.Equal(SidebarMarkTone.Degraded, marks["b"].Tone);
        Assert.Equal("notice_calendar:b", marks["b"].Label);
    }

    [Theory]
    [InlineData(nameof(NoticeSeverity.Blocked), nameof(SidebarMarkTone.Blocked), "\uE783")]
    [InlineData(nameof(NoticeSeverity.Degraded), nameof(SidebarMarkTone.Degraded), "\uE7BA")]
    [InlineData(nameof(NoticeSeverity.Informational), nameof(SidebarMarkTone.Informational), "\uE946")]
    public void A_mark_takes_its_glyph_from_its_severity(string severity, string tone, string glyph)
    {
        Assert.Equal(Enum.Parse<SidebarMarkTone>(tone), NoticeRules.ToneOf(Enum.Parse<NoticeSeverity>(severity)));
        Assert.Equal(glyph, SidebarMark.GlyphFor(Enum.Parse<SidebarMarkTone>(tone)));
    }

    [Fact]
    public void A_notice_is_announced_once_when_its_id_first_appears()
    {
        var announcements = new NoticeAnnouncements();
        var first = Make("first", nameof(NoticeScreens.Mail));
        var second = Make("second", nameof(NoticeScreens.Mail));

        Assert.Equal(["first"], announcements.Arrived([first]).Select(notice => notice.Id));
        // A redraw of the same notice, even one whose remedy has since failed, says nothing.
        Assert.Empty(announcements.Arrived([first with { RemedyFailed = true }]));
        Assert.Equal(["second"], announcements.Arrived([first, second]).Select(notice => notice.Id));
        // One that went and came back is raised again.
        Assert.Empty(announcements.Arrived([second]));
        Assert.Equal(["first"], announcements.Arrived([first, second]).Select(notice => notice.Id));
    }

    [Fact]
    public void Only_a_notice_with_an_action_offers_a_button()
    {
        Assert.True(NoticeRules.Offers(Make("retry", nameof(NoticeScreens.Mail))));
        Assert.False(NoticeRules.Offers(Make("none", nameof(NoticeScreens.Mail), action: false)));
    }

    [Fact]
    public void The_fingerprint_moves_with_what_a_notice_says_and_not_otherwise()
    {
        var notice = Make("a", nameof(NoticeScreens.Mail), account: "a");
        var drawn = NoticeRules.Fingerprint([notice]);

        Assert.Equal(drawn, NoticeRules.Fingerprint([Make("a", nameof(NoticeScreens.Mail), account: "a")]));
        Assert.NotEqual(drawn, NoticeRules.Fingerprint([notice with { RemedyFailed = true }]));
        Assert.NotEqual(drawn, NoticeRules.Fingerprint([notice with { Action = NoticeAction.OpenAccountSettings }]));
        Assert.NotEqual(drawn, NoticeRules.Fingerprint([notice with { Title = new NoticeText("notice_other", []) }]));
        Assert.NotEqual(drawn, NoticeRules.Fingerprint([]));
    }
}
