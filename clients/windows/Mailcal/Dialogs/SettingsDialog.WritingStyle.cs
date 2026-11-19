// Settings → Writing style (docs/ai.md rules 8 to 10), drawn from the core's writing_style_page():
// the category exists only while the writing assistant is offered. The library's rows open a
// style's reveal or its detail, as each row's `opens` says; each account has a slot picker; Learn
// opens the learning flow in the flow host, or says on the page why it cannot be pressed; a run is
// drawn in Learn's place with its Stop. Everything held, running or refused is the record's.
//
// The flow, the reveal and the detail are drawn in this category's panel in place of the page,
// because WinUI forbids a nested ContentDialog: SettingsDialog.WritingStyleLearning.cs,
// .WritingStyleReveal.cs and .WritingStyleDetail.cs.

using Allodia.Mailcal.Controls;
using Allodia.Mailcal.Services;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Automation;
using Microsoft.UI.Xaml.Controls;
using uniffi.mailcal_bindings;

namespace Allodia.Mailcal.Dialogs;

public sealed partial class SettingsDialog
{
    private const string WritingStyleTag = "writing_style";

    // What the category's panel shows when no flow is open: the page, or a style's reveal or
    // detail opened from its row. A flow the core holds is drawn over all of them.
    private StyleScreen _styleScreen = StyleScreen.Page;

    // The library outcomes already acted on, from when Settings opened, and what the last one came
    // to while it stands on the page.
    private WritingStyleOutcomes _styleOutcomes = new(null);
    private string? _styleSaid;

    // The panel's scroller, whose height a flow host fills, so its footer stays on screen.
    private ScrollViewer? _detailScroller;

    private double FlowHeight() =>
        _detailScroller is { ViewportHeight: > 200 } scroller ? scroller.ViewportHeight - 8 : 480;

    private static WritingStyleWords StyleWords() => new(
        L10n.WritingStyleRefusedBlankName(),
        L10n.WritingStyleRefusedGone(),
        L10n.LearnProgressUnknown(),
        (done, total) => L10n.LearnProgress(Number(done), Number(total)),
        L10n.LearnRangeAll(),
        L10n.LearnRangeUntilLastYear(),
        L10n.LearnRangeBetween(),
        L10n.RevealFrequencyMostly(),
        L10n.RevealFrequencyOften(),
        L10n.RevealFrequencySometimes());

    private static string Number(int value) => value.ToString(System.Globalization.CultureInfo.CurrentCulture);

    private static string StyleLanguage(string code) => WritingStyleRules.LanguageName(code, L10n.LanguageName);

    private static string StyleLanguages(IEnumerable<string> codes) =>
        string.Join(", ", codes.Select(StyleLanguage));

    private string StyleDate(long seconds) => WritingStyleRules.LongDate(
        WritingStyleRules.DayOf(seconds, DisplayZone()), System.Globalization.CultureInfo.CurrentCulture);

    private TimeZoneInfo DisplayZone()
    {
        try
        {
            return TimeZoneInfo.FindSystemTimeZoneById(_model.ActiveZone);
        }
        catch (Exception ex) when (ex is TimeZoneNotFoundException or InvalidTimeZoneException or ArgumentException)
        {
            return TimeZoneInfo.Local;
        }
    }

    /// <summary>The category's panel: the flow while the core holds one, otherwise the screen the
    /// person opened.</summary>
    private UIElement BuildWritingStyle()
    {
        if (_model.WritingStyleLearning() is { } learning)
        {
            return LearningHost(learning);
        }
        _learningHost = null;
        switch (_styleScreen)
        {
            case StyleScreen.Reveal reveal when _model.WritingStyleReveal(reveal.Style) is { } record:
                return RevealHost(record, inFlow: false);
            case StyleScreen.Detail detail when _model.WritingStyleDetail(detail.Style) is { } record:
                return DetailPanel(record);
        }
        _styleScreen = StyleScreen.Page;
        _revealParts = null;
        _detailParts = null;
        return _model.WritingStylePage() is { } page ? StylePage(page) : new StackPanel();
    }

    // The page moved, or the flow, a reveal or a detail. A flow, reveal or detail in hand draws its
    // controls in place, so what is typed stays; anything else draws the panel again.
    private void OnWritingStyleChanged()
    {
        if (_rebuilding)
        {
            return;
        }
        SyncWritingStyleCategory();
        if (CurrentCategory() != WritingStyleTag)
        {
            return;
        }
        if (RefreshStyleScreen())
        {
            return;
        }
        ShowCategory(WritingStyleTag);
    }

    // Draws the screen in hand again without rebuilding it, when it is still the one to draw.
    private bool RefreshStyleScreen()
    {
        var learning = _model.WritingStyleLearning();
        if (learning is not null)
        {
            return _learningHost is not null && RefreshLearning(learning);
        }
        if (_learningHost is not null)
        {
            return false;
        }
        return _styleScreen switch
        {
            StyleScreen.Reveal reveal when _revealParts is { } parts && _model.WritingStyleReveal(reveal.Style) is { } record =>
                RefreshReveal(parts, record),
            StyleScreen.Detail detail when _detailParts is { } parts && _model.WritingStyleDetail(detail.Style) is { } record =>
                RefreshDetail(parts, record),
            _ => false,
        };
    }

    private UIElement StylePage(WritingStylePage page)
    {
        if (_styleOutcomes.Fresh(page.Outcome) is { Result: WritingStyleResult.Refused refused, Kind: not WritingStyleChangeKind.Rename })
        {
            _styleSaid = WritingStyleRules.Say(refused.Reason, StyleWords());
        }
        var ink = new RevealInk(_brushes);
        var panel = new StackPanel { Spacing = 20 };
        panel.Children.Add(Description(L10n.WritingStyleIntro()));
        if (_styleSaid is { } said)
        {
            var bar = new InfoBar { Severity = InfoBarSeverity.Error, IsClosable = true, Message = said, IsOpen = true };
            AutomationProperties.SetAutomationId(bar, "WritingStyleOutcome");
            bar.Closed += (_, _) => _styleSaid = null;
            panel.Children.Add(bar);
        }
        panel.Children.Add(StyleLibrary(page, ink));
        panel.Children.Add(LearnArea(page, ink));
        if (page.Accounts.Length > 0)
        {
            var slots = new StackPanel { Spacing = 8 };
            slots.Children.Add(Heading(L10n.WritingStyleAccountsHeading()));
            slots.Children.Add(StyleSlots(page));
            panel.Children.Add(slots);
        }
        return panel;
    }

    private UIElement StyleLibrary(WritingStylePage page, RevealInk ink)
    {
        if (page.Styles.Length == 0)
        {
            var empty = Muted(L10n.WritingStyleEmpty(), ink);
            AutomationProperties.SetAutomationId(empty, "WritingStyleEmpty");
            return Card(empty, ink);
        }
        var rows = new StackPanel { Spacing = 4 };
        foreach (var style in page.Styles)
        {
            var subtitle = style.Facts.LearnedAt is { } at
                ? L10n.WritingStyleLearnedFrom((int)style.Facts.Messages, StyleDate(at))
                : string.Empty;
            if (style.Facts.Languages.Length > 0)
            {
                subtitle = string.Join(" · ", new[] { subtitle, StyleLanguages(style.Facts.Languages) }.Where(part => part.Length > 0));
            }
            var id = style.Id;
            var opens = style.Opens;
            rows.Children.Add(ControlComponents.NavigationRow(style.Name, subtitle, style.Open.Enabled, $"WritingStyleRow_{id}", () =>
            {
                SayStyleNothing();
                _styleScreen = opens == WritingStyleOpens.Reveal ? new StyleScreen.Reveal(id) : new StyleScreen.Detail(id);
                ShowCategory(WritingStyleTag);
            }));
        }
        return rows;
    }

    // One picker per account used for mail: None, then each style, in the library's order.
    private UIElement StyleSlots(WritingStylePage page)
    {
        var stack = new StackPanel { Spacing = 12 };
        var labels = new List<string> { L10n.WritingStyleNone() };
        labels.AddRange(page.Styles.Select(style => style.Name));
        foreach (var slot in page.Accounts)
        {
            var at = slot.Selected is { } chosen ? Array.FindIndex(page.Styles, style => style.Id == chosen) + 1 : 0;
            var account = slot.AccountId;
            stack.Children.Add(ControlComponents.ComboBox(slot.Email, labels, at, slot.Picker, $"WritingStyleSlot_{account}", picked =>
            {
                SayStyleNothing();
                _model.ChangeWritingStyle(new WritingStyleIntent.Assign(account, picked == 0 ? null : page.Styles[picked - 1].Id));
            }));
        }
        return stack;
    }

    // Learn, or the run in its place, or why Learn cannot be pressed with the one thing that helps.
    private UIElement LearnArea(WritingStylePage page, RevealInk ink)
    {
        var area = new StackPanel { Spacing = 12 };
        var words = AiWords.Catalog();
        if (page.Run is WritingStyleRun.Learning running)
        {
            var card = new StackPanel { Spacing = 12 };
            card.Children.Add(Progress(running.Done, running.Total, "WritingStyleRun"));
            card.Children.Add(ControlComponents.Button(L10n.LearnStop(), running.Stop, "WritingStyleStop", () =>
                _model.MoveLearning(new StyleLearningIntent.Stop())));
            area.Children.Add(Card(card, ink));
            return area;
        }
        if (page.Run is WritingStyleRun.Failed failed)
        {
            area.Children.Add(Bar(InfoBarSeverity.Error, L10n.WritingStyleRunFailed(), words.Of(failed.Failure), "WritingStyleRunFailed",
                RemedyButton(failed.Remedy, words, "WritingStyleRunRemedy", reopen: true)));
        }
        switch (page.LearnBlocked)
        {
            case WritingStyleLearnBlock.NoMailAccount:
                area.Children.Add(Bar(InfoBarSeverity.Informational, null, L10n.WritingStyleLearnNoAccount(), "WritingStyleLearnBlocked", null));
                break;
            case WritingStyleLearnBlock.CannotSend blocked:
                area.Children.Add(Bar(InfoBarSeverity.Informational, null, words.Of(blocked.Failure), "WritingStyleLearnBlocked",
                    RemedyButton(blocked.Remedy, words, "WritingStyleLearnRemedy", reopen: false)));
                break;
        }
        var learn = ControlComponents.Button(L10n.WritingStyleLearn(), page.Learn, "WritingStyleLearn", () =>
        {
            SayStyleNothing();
            _model.MoveLearning(new StyleLearningIntent.Open());
        }, (Style)Application.Current.Resources["AccentButtonStyle"]);
        area.Children.Add(learn);
        return area;
    }

    // A standing line in the platform's own bar: what cannot be done, or what went wrong, with the
    // one control that helps beneath it.
    private static InfoBar Bar(InfoBarSeverity severity, string? title, string message, string automationId, UIElement? remedy)
    {
        var bar = new InfoBar { Severity = severity, IsOpen = true, IsClosable = false, Message = message };
        if (remedy is not null)
        {
            bar.Content = new Border { Padding = new Thickness(0, 0, 0, 12), Child = remedy };
        }
        if (title is not null)
        {
            bar.Title = title;
        }
        AutomationProperties.SetAutomationId(bar, automationId);
        return bar;
    }

    // A failure's one remedy, drawn from the control its record gives. One that names another part of
    // Settings opens it; one that moves the flow is the flow's, which a run that failed with its host
    // closed reopens on the failure first.
    private UIElement? RemedyButton(AiRemedyControl? remedy, AiWords words, string automationId, bool reopen)
    {
        if (remedy is not { } given)
        {
            return null;
        }
        return ControlComponents.Button(words.Of(given.Remedy), given.Control, automationId, () =>
        {
            if (RemedyOpens(given.Remedy) is { } opens)
            {
                opens();
                return;
            }
            if (reopen)
            {
                _model.MoveLearning(new StyleLearningIntent.Open());
            }
            _model.MoveLearning(new StyleLearningIntent.Remedy());
        });
    }

    // A remedy that opens another part of Settings, which the client does; the ones that move the
    // flow are the flow's.
    private Action? RemedyOpens(AiRemedy remedy) => remedy switch
    {
        AiRemedy.OpenAiEndpoint => () => SelectCategory("advanced"),
        AiRemedy.SignInToAllodia when Categories().Any(category => category.Tag == "allodia") => () => SelectCategory("allodia"),
        _ => null,
    };

    private static UIElement Progress(uint done, uint total, string automationId)
    {
        var stack = new StackPanel { Spacing = 6 };
        var line = new TextBlock { Text = WritingStyleRules.Progress(done, total, StyleWords()), TextWrapping = TextWrapping.Wrap };
        AutomationProperties.SetAutomationId(line, automationId);
        stack.Children.Add(line);
        stack.Children.Add(new ProgressBar
        {
            IsIndeterminate = total == 0,
            Maximum = Math.Max(total, 1),
            Value = done,
        });
        return stack;
    }

    private static TextBlock Line(string text, string automationId)
    {
        var line = new TextBlock { Text = text, TextWrapping = TextWrapping.Wrap };
        AutomationProperties.SetAutomationId(line, automationId);
        return line;
    }

    private void SayStyleNothing() => _styleSaid = null;

    // Moves the source list to another category, as a person's click does.
    private void SelectCategory(string tag)
    {
        var item = _categories.Items.OfType<ListViewItem>().FirstOrDefault(row => (string)row.Tag == tag);
        if (item is not null)
        {
            _categories.SelectedItem = item;
        }
    }

    // Leaving the category, or Settings, closes what it had open: a flow (a run goes on, drawn on
    // the page), or a reveal, which then counts as shown.
    private void LeaveWritingStyle()
    {
        if (_learningHost is not null && _model.WritingStyleLearning() is { } learning)
        {
            if (learning.Stage is StyleLearningStage.Learned learned)
            {
                _model.ChangeWritingStyle(new WritingStyleIntent.CloseReveal(learned.Style));
            }
            else
            {
                _model.MoveLearning(new StyleLearningIntent.Close());
            }
        }
        else if (_styleScreen is StyleScreen.Reveal reveal)
        {
            _model.ChangeWritingStyle(new WritingStyleIntent.CloseReveal(reveal.Style));
        }
        _learningHost = null;
        _revealParts = null;
        _detailParts = null;
        _styleScreen = StyleScreen.Page;
    }

    // Adds the category to the source list, or takes it away, as the offer comes and goes.
    private void SyncWritingStyleCategory()
    {
        var listed = _categories.Items.OfType<ListViewItem>().Any(row => (string)row.Tag == WritingStyleTag);
        var offered = Categories().Any(category => category.Tag == WritingStyleTag);
        if (listed == offered)
        {
            return;
        }
        var selected = CurrentCategory();
        _rebuilding = true;
        _categories.Items.Clear();
        foreach (var (tag, label) in Categories())
        {
            _categories.Items.Add(CategoryItem(tag, label));
        }
        var index = Array.FindIndex(Categories(), category => category.Tag == selected);
        _categories.SelectedIndex = index < 0 ? 0 : index;
        _rebuilding = false;
        if (index < 0)
        {
            ShowCategory(Categories()[0].Tag);
        }
    }

    /// <summary>What the category's panel shows when no flow is open.</summary>
    private abstract record StyleScreen
    {
        internal static readonly StyleScreen Page = new PageScreen();

        internal sealed record PageScreen : StyleScreen;

        /// <summary>A style's reveal, opened from its row.</summary>
        internal sealed record Reveal(string Style) : StyleScreen;

        internal sealed record Detail(string Style) : StyleScreen;
    }
}
