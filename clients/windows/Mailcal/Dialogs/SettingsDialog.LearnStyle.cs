// Settings → Writing style → Learn my writing style (docs/ai.md, "Learning"): an account when there
// is a choice, the range, what the device found for them with the consent beneath, then the run,
// one page each in the frame the reveal uses (WizardFrame). Pressing Learn on the consent page is
// the consent; there is no other prompt. The run's page has a way to stop it, and ends in either the
// new style or why there is none.
//
// The report is read again whenever the account or the range changes, and redrawn in place, so the
// consent always speaks of the mail Learn would send. The pages, the state and the rule that a
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
    // Where the report and the consent are drawn, and the frame whose last button gives it; both
    // belong to the sheet on screen and are replaced when it is rebuilt.
    private StackPanel? _learnReport;
    private WizardFrame? _learnFrame;

    // Opens the sheet on `account` with everything on the device, and starts reading at once, so
    // the facts consent is given against are on screen as soon as they are known.
    private void OpenLearnSheet(string account, int accounts)
    {
        var zone = WritingStyleFormat.Zone(_model.ActiveZone);
        var today = DateOnly.FromDateTime(TimeZoneInfo.ConvertTime(DateTimeOffset.UtcNow, zone).DateTime);
        var sheet = new LearnSheet(account, today, accounts);
        Apply(() =>
        {
            _learnSheet = sheet;
            _styleScreen = StyleScreen.Learn;
        });
        _ = ReadCorpusAsync(sheet);
    }

    private UIElement BuildLearnSheet(LearnSheet sheet, WritingStyleSnapshot snapshot)
    {
        var frame = new WizardFrame(
            sheet.Pager, PanelHeight, allowsJump: false, (_, index) => LearnPage(sheet, snapshot, sheet.Steps[index]));
        frame.SetCancel(L10n.ActionCancel(), () => Apply(() =>
        {
            _learnSheet = null;
            _styleScreen = StyleScreen.Library;
        }));
        frame.Moved = () => LearnFooter(sheet, frame);
        _learnFrame = frame;
        frame.Show();
        return frame.Root;
    }

    private UIElement LearnPage(LearnSheet sheet, WritingStyleSnapshot snapshot, LearnStep step)
    {
        if (step == LearnStep.Account)
        {
            var account = WizardFrame.Page(L10n.LearnAccountTitle());
            account.Children.Add(LearnAccountChoice(sheet, snapshot.Accounts));
            return account;
        }
        if (step == LearnStep.Range)
        {
            var range = WizardFrame.Page(L10n.LearnRangeTitle());
            range.Children.Add(LearnRangeChoice(sheet));
            return range;
        }
        var consent = WizardFrame.Page(L10n.WritingStyleLearn());
        _learnReport = new StackPanel { Spacing = 6 };
        FillLearnReport(sheet);
        consent.Children.Add(_learnReport);
        return consent;
    }

    // Back and Next take the person through the choices; on the consent page Learn stands where
    // Next did, and only over a report with something to learn from.
    private void LearnFooter(LearnSheet sheet, WizardFrame frame)
    {
        if (sheet.Step != LearnStep.Consent)
        {
            return;
        }
        if (!sheet.CanLearn)
        {
            frame.HidePrimary();
            return;
        }
        frame.SetAction(L10n.LearnConsentConfirm(), enabled: true, accent: true, () =>
        {
            // Disabled at once: a second press before the panel redraws would start a second run,
            // which the core refuses as busy over the first one's progress.
            frame.EnablePrimary(false);
            _ = LearnAsync(sheet);
        });
    }

    private UIElement LearnAccountChoice(LearnSheet sheet, IReadOnlyList<AccountWritingStyleRow> accounts)
    {
        var stack = new StackPanel { Spacing = 4 };
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
        // `sender` rather than `_`: with one named parameter beside it, `_` is the sender itself,
        // and `_ = ReadCorpusAsync(…)` below would assign the task to it rather than discard it.
        picker.DateChanged += (sender, args) =>
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
        if (_learnFrame is { } frame)
        {
            LearnFooter(sheet, frame);
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
        sheet.Pager.Go(sheet.Steps.Count - 1);
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
                _reveal = null;
                _styleScreen = StyleScreen.Style;
            }
            else
            {
                _learnFailure = outcome.Failure ?? new AiFailure(AiProblem.Unavailable);
                _styleScreen = StyleScreen.Failed;
            }
        });
    }

    // The run's progress from the core's snapshot, on the sheet's last page: reading on the device,
    // then one request per part with a determinate bar. Stop stands where Learn did, and takes
    // effect before the next request.
    private UIElement BuildLearning(LearningProgress? progress)
    {
        _learnReport = null;
        _learnFrame = null;
        var frame = new WizardFrame(RunPager(), PanelHeight, allowsJump: false, (_, _) =>
        {
            var page = WizardFrame.Page(L10n.WritingStyleLearn());
            var text = L10n.LearnReading();
            var bar = new ProgressBar { IsIndeterminate = true };
            if (progress is { Stage: LearningStage.Learning, Total: > 0 } learning)
            {
                text = L10n.LearnProgress(learning.Done.ToString(Culture), learning.Total.ToString(Culture));
                bar = new ProgressBar { Maximum = learning.Total, Value = learning.Done };
            }
            AutomationProperties.SetName(bar, text);
            page.Children.Add(Line(text));
            page.Children.Add(bar);
            return page;
        });
        frame.HideCancel();
        frame.Moved = () =>
        {
            frame.SetBack(false);
            frame.SetAction(L10n.LearnStop(), enabled: true, accent: false, _model.CancelWritingStyleLearning);
        };
        frame.Show();
        return frame.Root;
    }

    private UIElement BuildLearnFailed(AiFailure failure)
    {
        var frame = new WizardFrame(RunPager(), PanelHeight, allowsJump: false, (_, _) =>
        {
            var page = WizardFrame.Page(L10n.LearnFailedTitle());
            page.Children.Add(Line(WritingStyleText.Of(failure)));
            return page;
        });
        frame.HideCancel();
        frame.Moved = () =>
        {
            frame.SetBack(false);
            frame.SetAction(L10n.ActionClose(), enabled: true, accent: true, () => Apply(() =>
            {
                _learnFailure = null;
                _styleScreen = StyleScreen.Library;
            }));
        };
        frame.Show();
        return frame.Root;
    }

    // The run is the sheet's last page. One started before this dialog opened has no sheet here,
    // and is drawn on the last page of the sheet it would have had.
    private WizardPager RunPager()
    {
        var pager = _learnSheet?.Pager
            ?? new WizardPager(LearnSheet.StepsFor(_model.WritingStyles.Accounts.Length).Length);
        pager.Go(pager.Count - 1);
        return pager;
    }

    // The catalog locale the app is shown in: the language the style's description is written in,
    // because the person reads it back.
    private static string CatalogLocale() =>
        WritingStyleFormat.CatalogLocale(
            ShowcaseMode.LanguageOverride ?? LanguageStore.Read(),
            CultureInfo.CurrentUICulture.TwoLetterISOLanguageName);
}
