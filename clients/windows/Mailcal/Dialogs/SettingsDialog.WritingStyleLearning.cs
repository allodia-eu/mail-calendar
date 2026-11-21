// Settings → Writing style ▸ Learn (docs/ai.md rules 11 to 14): the core's writing_style_learning()
// drawn in the flow host. The stage and every control on it are the record's: a control the record
// leaves out is not drawn, and a press only sends its intent, the core moving the flow. Learned
// hands over to the reveal in the same host.

using Allodia.Mailcal.Controls;
using Allodia.Mailcal.Services;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Automation;
using Microsoft.UI.Xaml.Controls;
using uniffi.mailcal_bindings;

namespace Allodia.Mailcal.Dialogs;

public sealed partial class SettingsDialog
{
    // The host the flow is drawn in, and the stage on screen with what it was drawn from, so a
    // record that moves nothing on the stage leaves its controls (an open date picker, a choice)
    // where they are.
    private FlowHost? _learningHost;
    private string? _stageKey;
    private UIElement? _stageShown;

    private UIElement LearningHost(WritingStyleLearning learning)
    {
        _learningHost = new FlowHost("WritingStyleFlow", FlowHeight());
        _stageKey = null;
        _revealParts = null;
        DrawLearning(learning);
        return _learningHost.Root;
    }

    private bool RefreshLearning(WritingStyleLearning learning)
    {
        if (learning.Stage is StyleLearningStage.Learned learned && _revealParts is { InFlow: true } parts && parts.Style == learned.Style)
        {
            return _model.WritingStyleReveal(learned.Style) is { } reveal && RefreshReveal(parts, reveal);
        }
        DrawLearning(learning);
        return true;
    }

    private void DrawLearning(WritingStyleLearning learning)
    {
        if (_learningHost is not { } host)
        {
            return;
        }
        if (learning.Stage is StyleLearningStage.Learned learned)
        {
            if (_model.WritingStyleReveal(learned.Style) is { } reveal)
            {
                RevealHost(reveal, inFlow: true);
            }
            return;
        }
        var key = StageKey(learning.Stage);
        if (key != _stageKey || _stageShown is null)
        {
            _stageKey = key;
            _stageShown = Stage(learning.Stage);
        }
        host.Draw(LearningFrame(learning), _stageShown);
    }

    // The footer from the record: Close, Back where the stage has one, and the stage's one action.
    private FlowFrame LearningFrame(WritingStyleLearning learning)
    {
        var words = AiWords.Catalog();
        var choosing = learning.Stage is StyleLearningStage.ChooseAccount or StyleLearningStage.ChooseRange or StyleLearningStage.Review;
        var close = new FlowAction(choosing ? L10n.ActionCancel() : L10n.ActionClose(), learning.Close, () =>
        {
            _model.MoveLearning(new StyleLearningIntent.Close());
        });
        FlowAction? Action(ControlState? state, string label, Action press) => state is { } given ? new FlowAction(label, given, press) : null;
        var primary = Action(learning.Next, L10n.WizardNext(), () => _model.MoveLearning(new StyleLearningIntent.Next()))
            ?? Action(learning.Learn, L10n.LearnConsentConfirm(), LearnNow)
            ?? Action(learning.Stop, L10n.LearnStop(), () => _model.MoveLearning(new StyleLearningIntent.Stop()))
            ?? (learning.Stage is StyleLearningStage.Failed { Remedy: { } remedy }
                ? Action(remedy.Control, words.Of(remedy.Remedy), () => TakeRemedy(remedy.Remedy, reopen: false))
                : null);
        var back = Action(learning.Back, L10n.WizardBack(), () => _model.MoveLearning(new StyleLearningIntent.Back()));
        // The stages behind this one tell the host which side it comes in from.
        return new FlowFrame(StageTitle(learning.Stage), close, back, primary, Depth: learning.Trail.Length);
    }

    // The consent: the style is stored under the catalog's default name, its description written
    // in the language the app is read in.
    private void LearnNow()
    {
        var language = WritingStyleRules.AppLanguage(
            LanguageStore.Read(), System.Globalization.CultureInfo.InstalledUICulture.TwoLetterISOLanguageName, L10n.Locales);
        _model.MoveLearning(new StyleLearningIntent.Learn(L10n.WritingStyleDefaultName(), language));
    }

    private static string StageTitle(StyleLearningStage stage) => stage switch
    {
        StyleLearningStage.ChooseAccount => L10n.LearnAccountTitle(),
        StyleLearningStage.ChooseRange => L10n.LearnRangeTitle(),
        StyleLearningStage.Review => L10n.LearnConsentTitle(),
        StyleLearningStage.Failed => L10n.LearnFailedTitle(),
        _ => L10n.WritingStyleLearn(),
    };

    // What a stage is drawn from: the same key draws the same stage, so it is kept.
    private static string StageKey(StyleLearningStage stage) => stage switch
    {
        StyleLearningStage.ChooseAccount choose => $"account:{choose.Selected}:{choose.Accounts.Length}",
        StyleLearningStage.ChooseRange range => $"range:{range.Email}:{RangeKey(range.Selected)}:{range.Fetch}",
        StyleLearningStage.Review review => $"review:{review.State.GetType().Name}:{RangeKey(review.Range)}:{review.OlderNotFetched}:{review.SendsTo}:{review.CannotSend}",
        StyleLearningStage.Learning learning => $"learning:{learning.Done}:{learning.Total}",
        StyleLearningStage.Failed failed => $"failed:{failed.Failure}:{failed.Remedy}",
        _ => stage.GetType().Name,
    };

    private static string RangeKey(StyleLearnRange? range) => range switch
    {
        StyleLearnRange.Between between => $"between:{between.Since}:{between.Until}",
        null => "none",
        _ => range.GetType().Name,
    };

    private UIElement Stage(StyleLearningStage stage) => stage switch
    {
        StyleLearningStage.ChooseAccount choose => ChooseAccountStage(choose),
        StyleLearningStage.ChooseRange range => ChooseRangeStage(range),
        StyleLearningStage.Review review => ReviewStage(review),
        StyleLearningStage.Learning learning => Card(Progress(learning.Done, learning.Total, "WritingStyleFlowProgress"), new RevealInk(_brushes)),
        StyleLearningStage.Failed failed => Line(AiWords.Catalog().Of(failed.Failure), "WritingStyleFlowFailure"),
        _ => new StackPanel(),
    };

    private UIElement ChooseAccountStage(StyleLearningStage.ChooseAccount stage)
    {
        var choice = new RadioButtons();
        AutomationProperties.SetName(choice, L10n.LearnAccountTitle());
        AutomationProperties.SetAutomationId(choice, "WritingStyleFlowAccounts");
        foreach (var account in stage.Accounts)
        {
            choice.Items.Add(account.Email);
        }
        choice.SelectedIndex = Array.FindIndex(stage.Accounts, account => account.AccountId == stage.Selected);
        choice.SelectionChanged += (_, _) =>
        {
            if (choice.SelectedIndex >= 0 && stage.Accounts[choice.SelectedIndex].AccountId != stage.Selected)
            {
                _model.MoveLearning(new StyleLearningIntent.SelectAccount(stage.Accounts[choice.SelectedIndex].AccountId));
            }
        };
        return choice;
    }
}
