// Settings → Writing style → Learn my writing style (docs/ai.md, "Learning"): one sheet that holds
// the account, the range, what the device found for them, and the consent. Pressing Learn on it is
// the consent; there is no other prompt. Then the run's progress with a way to stop it, and either
// the new style or why there is none.
//
// The report is read again whenever the account or the range changes, and redrawn in place under
// the choices, so the control the person just used keeps its focus. The state and the rule that a
// superseded read is dropped are LearnSheet, where Mailcal.Tests can reach them.

using System.Globalization;
using Allodia.Mailcal.Services;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Automation;
using Microsoft.UI.Xaml.Controls;
using uniffi.mailcal_bindings;

namespace Allodia.Mailcal.Dialogs;

public sealed partial class SettingsDialog
{
    // Where the report and the consent are drawn, and the button that gives it; both belong to the
    // sheet on screen and are replaced when it is rebuilt.
    private StackPanel? _learnReport;
    private Button? _learnConfirm;

    // Opens the sheet on `account` with everything on the device, and starts reading at once, so
    // the facts consent is given against are on screen as soon as they are known.
    private void OpenLearnSheet(string account)
    {
        var zone = WritingStyleFormat.Zone(_model.ActiveZone);
        var today = DateOnly.FromDateTime(TimeZoneInfo.ConvertTime(DateTimeOffset.UtcNow, zone).DateTime);
        var sheet = new LearnSheet(account, today);
        Apply(() =>
        {
            _learnSheet = sheet;
            _styleScreen = StyleScreen.Learn;
        });
        _ = ReadCorpusAsync(sheet);
    }

    private UIElement BuildLearnSheet(LearnSheet sheet, WritingStyleSnapshot snapshot)
    {
        var panel = new StackPanel { Spacing = 16 };
        panel.Children.Add(Heading(L10n.WritingStyleLearn()));
        if (LearnSheet.AsksForAccount(snapshot.Accounts.Length))
        {
            panel.Children.Add(LearnAccountChoice(sheet, snapshot.Accounts));
        }
        panel.Children.Add(LearnRangeChoice(sheet));

        var confirm = new Button
        {
            Content = L10n.LearnConsentConfirm(),
            Style = (Style)Application.Current.Resources["AccentButtonStyle"],
        };
        // Disabled at once: a second press before the panel redraws would start a second run,
        // which the core refuses as busy over the first one's progress.
        confirm.Click += (_, _) =>
        {
            confirm.IsEnabled = false;
            _ = LearnAsync(sheet);
        };
        _learnConfirm = confirm;
        _learnReport = new StackPanel { Spacing = 6 };
        FillLearnReport(sheet);
        panel.Children.Add(_learnReport);

        var buttons = new StackPanel
        {
            Orientation = Orientation.Horizontal,
            Spacing = 8,
            HorizontalAlignment = HorizontalAlignment.Right,
        };
        var cancel = new Button { Content = L10n.ActionCancel() };
        cancel.Click += (_, _) => Apply(() =>
        {
            _learnSheet = null;
            _styleScreen = StyleScreen.Library;
        });
        buttons.Children.Add(cancel);
        buttons.Children.Add(confirm);
        panel.Children.Add(buttons);
        return panel;
    }

    private UIElement LearnAccountChoice(LearnSheet sheet, IReadOnlyList<AccountWritingStyleRow> accounts)
    {
        var stack = new StackPanel { Spacing = 4 };
        stack.Children.Add(Heading(L10n.LearnAccountTitle()));
        foreach (var account in accounts)
        {
            var id = account.AccountId;
            stack.Children.Add(Radio(account.Email, "learn-account", sheet.Account == id, () =>
            {
                sheet.Account = id;
                _ = ReadCorpusAsync(sheet);
            }));
        }
        return stack;
    }

    private UIElement LearnRangeChoice(LearnSheet sheet)
    {
        var stack = new StackPanel { Spacing = 4 };
        stack.Children.Add(Heading(L10n.LearnRangeTitle()));

        var until = new StackPanel
        {
            Spacing = 4,
            Margin = new Thickness(28, 0, 0, 0),
            Visibility = sheet.UpToDate ? Visibility.Visible : Visibility.Collapsed,
        };
        until.Children.Add(Description(L10n.LearnRangeUntilHint()));
        var picker = new CalendarDatePicker
        {
            Header = L10n.LearnRangeUntilLabel(),
            Date = new DateTimeOffset(sheet.UntilDay.ToDateTime(TimeOnly.MinValue)),
        };
        AutomationProperties.SetName(picker, L10n.LearnRangeUntilLabel());
        picker.DateChanged += (_, args) =>
        {
            // The picker hands back the chosen day at some time of day in the host's offset; the
            // day is what was chosen, and the sheet makes the range end with it.
            if (_rebuilding || args.NewDate is not { } date)
            {
                return;
            }
            sheet.UntilDay = DateOnly.FromDateTime(date.Date);
            _ = ReadCorpusAsync(sheet);
        };
        until.Children.Add(picker);

        stack.Children.Add(Radio(L10n.LearnRangeAll(), "learn-range", !sheet.UpToDate, () =>
        {
            sheet.UpToDate = false;
            until.Visibility = Visibility.Collapsed;
            _ = ReadCorpusAsync(sheet);
        }));
        stack.Children.Add(Radio(L10n.LearnRangeUntil(), "learn-range", sheet.UpToDate, () =>
        {
            sheet.UpToDate = true;
            until.Visibility = Visibility.Visible;
            _ = ReadCorpusAsync(sheet);
        }));
        stack.Children.Add(until);
        return stack;
    }

    // Reads what learning from the sheet's current choice would take. The Sent folder is read on
    // the device and nothing leaves it; an answer for a choice the person has since changed is
    // dropped rather than drawn under the new one.
    private async Task ReadCorpusAsync(LearnSheet sheet)
    {
        var read = sheet.BeginRead();
        FillLearnReport(sheet);
        var until = sheet.Until(WritingStyleFormat.Zone(_model.ActiveZone));
        var outcome = await _model.SentCorpusReportAsync(sheet.Account, until);
        if (sheet.Finish(read, outcome.Value, outcome.Failure))
        {
            FillLearnReport(sheet);
        }
    }

    // The report, then the consent: what was found, and exactly what is sent where. No consent is
    // offered over a report with nothing to learn from.
    private void FillLearnReport(LearnSheet sheet)
    {
        if (_learnReport is not { } report || _learnSheet != sheet)
        {
            return;
        }
        report.Children.Clear();
        if (_learnConfirm is { } confirm)
        {
            confirm.Visibility = sheet.CanLearn ? Visibility.Visible : Visibility.Collapsed;
        }
        if (sheet.Reading)
        {
            report.Children.Add(Busy(L10n.LearnReading()));
            return;
        }
        if (sheet.Failure is { } failure)
        {
            report.Children.Add(Warning(WritingStyleText.Of(failure)));
            return;
        }
        if (sheet.Report is not { } found)
        {
            return;
        }
        report.Children.Add(Line(L10n.LearnReportFound((int)found.Found)));
        report.Children.Add(Line(L10n.LearnReportUsable((int)found.Usable)));
        if (found.Languages.Length > 0)
        {
            report.Children.Add(Line(L10n.LearnReportLanguages(
                WritingStyleText.Languages(found.Languages.Select(language => language.Language)))));
        }
        if (found.Undetected > 0)
        {
            report.Children.Add(Line(L10n.LearnReportUndetected((int)found.Undetected)));
        }
        if (found.Horizon is { } horizon)
        {
            report.Children.Add(Description(
                L10n.LearnReportHorizon(WritingStyleText.Date(horizon, _model.ActiveZone))));
        }
        if (found.Usable == 0)
        {
            report.Children.Add(Warning(L10n.LearnReportNothing()));
            return;
        }
        var consent = Heading(L10n.LearnConsentTitle());
        consent.Margin = new Thickness(0, 10, 0, 0);
        report.Children.Add(consent);
        report.Children.Add(Line(ConsentText()));
    }

    // Where the words go, named as the person set it up: Allodia's relay, or the host of their own
    // endpoint.
    private string ConsentText() =>
        _model.WritingStyles.Route == AiRoute.Relay
            ? L10n.LearnConsentRelay()
            : L10n.LearnConsentOwn(OwnEndpointForm.Host(_model.OwnEndpoint?.BaseUrl ?? string.Empty));

    // The consent was given: learn, then open the new style, or say why there is none.
    private async Task LearnAsync(LearnSheet sheet)
    {
        if (!sheet.CanLearn)
        {
            return;
        }
        var until = sheet.Until(WritingStyleFormat.Zone(_model.ActiveZone));
        Apply(() => _styleScreen = StyleScreen.Learning);
        var outcome = await _model.LearnWritingStyleAsync(
            sheet.Account, until, L10n.WritingStyleDefaultName(), CatalogLocale());
        // Leaving the category put it back on the library; the new style is in the list there.
        if (_styleScreen != StyleScreen.Learning)
        {
            return;
        }
        Apply(() =>
        {
            _learnSheet = null;
            if (outcome.Value is { } learned)
            {
                _openStyle = learned.StyleId;
                _styleScreen = StyleScreen.Style;
            }
            else
            {
                _learnFailure = outcome.Failure ?? new AiFailure(AiProblem.Unavailable);
                _styleScreen = StyleScreen.Failed;
            }
        });
    }

    // The run's progress from the core's snapshot: reading on the device, then one request per
    // part with a determinate bar. Stop takes effect before the next request.
    private UIElement BuildLearning(LearningProgress? progress)
    {
        var panel = new StackPanel { Spacing = 8 };
        panel.Children.Add(Heading(L10n.WritingStyleLearn()));
        var text = L10n.LearnReading();
        var bar = new ProgressBar { IsIndeterminate = true };
        if (progress is { Stage: LearningStage.Learning, Total: > 0 } learning)
        {
            text = L10n.LearnProgress(learning.Done.ToString(Culture), learning.Total.ToString(Culture));
            bar = new ProgressBar { Maximum = learning.Total, Value = learning.Done };
        }
        AutomationProperties.SetName(bar, text);
        panel.Children.Add(Line(text));
        panel.Children.Add(bar);
        var stop = new Button { Content = L10n.LearnStop() };
        stop.Click += (_, _) => _model.CancelWritingStyleLearning();
        panel.Children.Add(stop);
        return panel;
    }

    private UIElement BuildLearnFailed(AiFailure failure)
    {
        var panel = new StackPanel { Spacing = 8 };
        panel.Children.Add(Heading(L10n.LearnFailedTitle()));
        panel.Children.Add(Line(WritingStyleText.Of(failure)));
        var close = new Button { Content = L10n.ActionClose() };
        close.Click += (_, _) => Apply(() =>
        {
            _learnFailure = null;
            _styleScreen = StyleScreen.Library;
        });
        panel.Children.Add(close);
        return panel;
    }

    // The catalog locale the app is shown in: the language the style's description is written in,
    // because the person reads it back.
    private static string CatalogLocale() =>
        WritingStyleFormat.CatalogLocale(
            ShowcaseMode.LanguageOverride ?? LanguageStore.Read(),
            CultureInfo.CurrentUICulture.TwoLetterISOLanguageName);
}
