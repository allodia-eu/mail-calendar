// The learning flow's two stages with the most on them (docs/ai.md rule 11): which sent mail to
// learn from, from the core's presets and the days it bounds, and the review, which says what was
// read on the device and what Learn would send where before anything leaves it.

using Allodia.Mailcal.Services;
using Microsoft.UI.Text;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Automation;
using Microsoft.UI.Xaml.Controls;
using uniffi.mailcal_bindings;

namespace Allodia.Mailcal.Dialogs;

public sealed partial class SettingsDialog
{
    private UIElement ChooseRangeStage(StyleLearningStage.ChooseRange stage)
    {
        var words = StyleWords();
        var panel = new StackPanel { Spacing = 12 };
        panel.Children.Add(Line(stage.Email, "WritingStyleFlowAccount"));
        panel.Children.Add(Description(Reach(stage.Reach)));

        // The core's presets, then a range of the person's own, which is always offered within
        // the days the record bounds.
        var options = stage.Presets.ToList();
        options.Add(new StyleLearnRange.Between(stage.Earliest ?? stage.Latest, stage.Latest));
        var choice = new RadioButtons();
        AutomationProperties.SetName(choice, L10n.LearnRangeTitle());
        AutomationProperties.SetAutomationId(choice, "WritingStyleFlowRanges");
        foreach (var option in options)
        {
            choice.Items.Add(WritingStyleRules.Range(option, words));
        }
        choice.SelectedIndex = options.FindIndex(option => stage.Selected?.GetType() == option.GetType());
        panel.Children.Add(choice);

        var latest = WritingStyleRules.Day(stage.Latest) ?? DateOnly.FromDateTime(DateTime.Today);
        var earliest = stage.Earliest is { } first ? WritingStyleRules.Day(first) : null;
        switch (stage.Selected)
        {
            case StyleLearnRange.UntilEndOfLastYear:
                panel.Children.Add(Description(L10n.LearnRangeUntilHint()));
                break;
            case StyleLearnRange.Between between:
                panel.Children.Add(BetweenDays(between, earliest, latest));
                break;
        }
        choice.SelectionChanged += (_, _) =>
        {
            if (choice.SelectedIndex < 0 || options[choice.SelectedIndex].GetType() == stage.Selected?.GetType())
            {
                return;
            }
            // A range of the person's own opens on the days the core allows, which they then
            // narrow; with no first day bounded, on the last day alone.
            _model.MoveLearning(new StyleLearningIntent.SelectRange(options[choice.SelectedIndex]));
        };
        return panel;
    }

    // The two days of a range of the person's own, each held to the days the core allows.
    private UIElement BetweenDays(StyleLearnRange.Between between, DateOnly? earliest, DateOnly latest)
    {
        var since = WritingStyleRules.Day(between.Since) ?? latest;
        var until = WritingStyleRules.Day(between.Until) ?? latest;
        var row = new StackPanel { Orientation = Orientation.Horizontal, Spacing = 12 };
        CalendarDatePicker Picker(string header, DateOnly day, string id)
        {
            var picker = new CalendarDatePicker
            {
                Header = header,
                Date = new DateTimeOffset(day.ToDateTime(TimeOnly.MinValue)),
                MaxDate = new DateTimeOffset(latest.ToDateTime(TimeOnly.MinValue)),
            };
            if (earliest is { } first)
            {
                picker.MinDate = new DateTimeOffset(first.ToDateTime(TimeOnly.MinValue));
            }
            AutomationProperties.SetName(picker, header);
            AutomationProperties.SetAutomationId(picker, id);
            return picker;
        }
        var from = Picker(L10n.LearnRangeFrom(), since, "WritingStyleFlowSince");
        var to = Picker(L10n.LearnRangeUntilLabel(), until, "WritingStyleFlowUntil");
        void Send()
        {
            if (from.Date is { } a && to.Date is { } b)
            {
                _model.MoveLearning(new StyleLearningIntent.SelectRange(new StyleLearnRange.Between(
                    WritingStyleRules.Day(DateOnly.FromDateTime(a.Date)), WritingStyleRules.Day(DateOnly.FromDateTime(b.Date)))));
            }
        }
        from.DateChanged += (_, _) => Send();
        to.DateChanged += (_, _) => Send();
        row.Children.Add(from);
        row.Children.Add(to);
        return row;
    }

    private string Reach(StyleSentReach reach) => reach is StyleSentReach.Since since && WritingStyleRules.Day(since.Day) is { } day
        ? L10n.LearnReportHorizon(WritingStyleRules.LongDate(day, System.Globalization.CultureInfo.CurrentCulture))
        : L10n.LearnReportHorizonAll();

    private UIElement ReviewStage(StyleLearningStage.Review stage)
    {
        var panel = new StackPanel { Spacing = 12 };
        panel.Children.Add(Line(stage.Email, "WritingStyleFlowAccount"));
        panel.Children.Add(Description(Reach(stage.Reach)));
        switch (stage.ReviewValue)
        {
            case StyleLearningReview.Reading:
                var reading = new StackPanel { Orientation = Orientation.Horizontal, Spacing = 10 };
                reading.Children.Add(new ProgressRing { IsActive = true, Width = 20, Height = 20 });
                reading.Children.Add(Line(L10n.LearnReading(), "WritingStyleFlowReading"));
                panel.Children.Add(reading);
                return panel;
            case StyleLearningReview.Ready ready:
                panel.Children.Add(Card(Report(ready.Report), new RevealInk(_brushes)));
                break;
            case StyleLearningReview.NothingUsable nothing:
                panel.Children.Add(Card(Report(nothing.Report), new RevealInk(_brushes)));
                panel.Children.Add(Bar(InfoBarSeverity.Warning, null, L10n.LearnReportNothing(), "WritingStyleFlowNothing", null));
                return panel;
        }

        // What Learn sends where, under the stage's own title.
        var sends = new StackPanel { Spacing = 4 };
        var consent = stage.SendsTo switch
        {
            StyleLearningDestination.Relay => L10n.LearnConsentRelay(),
            StyleLearningDestination.OwnEndpoint own => L10n.LearnConsentOwn(own.Host),
            _ => stage.CannotSend is { } failure ? AiWords.Catalog().Of(failure) : string.Empty,
        };
        sends.Children.Add(Line(consent, "WritingStyleFlowConsent"));
        panel.Children.Add(sends);
        if (!stage.KeywordKept)
        {
            panel.Children.Add(Description(L10n.LearnKeywordNotKept()));
        }
        return panel;
    }

    // What the review read: how many messages, how many say enough, in which languages, and over
    // which days.
    private UIElement Report(StyleLearningReport report)
    {
        var lines = new StackPanel { Spacing = 2 };
        AutomationProperties.SetAutomationId(lines, "WritingStyleFlowReport");
        lines.Children.Add(Line(L10n.LearnReportFound((int)report.Found), "WritingStyleFlowFound"));
        lines.Children.Add(Line(L10n.LearnReportUsable((int)report.Usable), "WritingStyleFlowUsable"));
        if (report.Languages.Length > 0)
        {
            lines.Children.Add(Line(L10n.LearnReportLanguages(string.Join(", ", report.Languages.Select(language =>
                $"{StyleLanguage(language.Language)} ({Number((int)language.Usable)})"))), "WritingStyleFlowLanguages"));
        }
        if (report.OtherLanguages > 0)
        {
            lines.Children.Add(Line(L10n.LearnReportUndetected((int)report.OtherLanguages), "WritingStyleFlowUndetected"));
        }
        if (report.Oldest is { } oldest && report.Newest is { } newest)
        {
            lines.Children.Add(Description(L10n.LearnReportDates(StyleDate(oldest), StyleDate(newest))));
        }
        return lines;
    }
}
