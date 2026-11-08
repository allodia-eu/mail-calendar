// The core's three questions on Windows (docs/notices.md rule 8): each a notice in the window's one
// bar with two answers, the first the accent one, never closable, and taken away by its answer.
//
// MAILCAL_SHOWCASE_QUESTIONS (debug builds, read in the core) raises all three, so everything from
// the question to the button is real: a sent message whose copy is not in Sent and an invitation
// reply not passed on, both on the first account, and a changed time zone. The twin is
// clients/apple/UITests/QuestionNoticeTests.swift.

using Allodia.Mailcal.UiTests.Driver;
using Xunit;

namespace Allodia.Mailcal.UiTests.Suites;

/// <summary>The showcase with the core's three questions raised.</summary>
public sealed class QuestionsShowcaseApp : ShowcaseApp
{
    protected override IReadOnlyDictionary<string, string> Env { get; } =
        new Dictionary<string, string> { ["MAILCAL_SHOWCASE_QUESTIONS"] = "1" };
}

[Trait("Dataset", "showcase")]
public sealed class QuestionNoticeTests(QuestionsShowcaseApp showcase) : IClassFixture<QuestionsShowcaseApp>
{
    /// <summary>Each question's two answers, first and second, as the catalog words them.</summary>
    private static readonly (string First, string Second)[] Questions =
    [
        (Catalog.English("notice_action_file_sent_copy"), Catalog.English("notice_action_leave_sent_copy")),
        (Catalog.English("notice_action_send_reply"), Catalog.English("notice_action_dont_send_reply")),
        (Catalog.English("notice_action_use_time_zone"), Catalog.English("notice_action_keep_time_zone")),
    ];

    [OfflineFact]
    public void EachQuestionIsANoticeAnsweredByItsOwnButtonsAndItsSecondAnswerTakesItAway() =>
        showcase.Case(nameof(EachQuestionIsANoticeAnsweredByItsOwnButtonsAndItsSecondAnswerTakesItAway), app =>
        {
            app.RowsList();
            var answered = new List<string>();
            // The bar draws the highest-ranked question first; answering it brings up the next,
            // so the time zone's, ranked last, is reached on the mail screen too.
            foreach (var _ in Questions)
            {
                Element? bar = null;
                Element? first = null;
                Element? second = null;
                if (!App.Eventually(() => (bar = app.NoticeBar()) is not null
                        && (first = app.NoticeAnswer(bar)) is not null
                        && (second = app.NoticeSecondAnswer(bar)) is not null, TimeSpan.FromSeconds(30)))
                {
                    throw new InvalidOperationException(
                        $"the bar draws no question with two answers after answering [{string.Join(", ", answered)}]. Is this a DEBUG build? MAILCAL_SHOWCASE_QUESTIONS is compiled out of Release");
                }
                var pair = (first!.Name, second!.Name);
                Assert.True(Questions.Contains(pair), $"the bar's answers are one question's pair; it draws [{pair.Item1}] and [{pair.Item2}]");
                Assert.DoesNotContain(pair.Item2, answered);
                Assert.True(app.NoticeClose(bar!) is null, "a question is answered, never closed");
                Assert.True(app.NoticeAction(bar!) is null, "a question draws its two answers, not the single action button");

                app.Agent.Invoke(second);
                answered.Add(pair.Item2);
                Assert.True(App.Eventually(() => app.NoticeBar() is not { } now || app.NoticeSecondAnswer(now)?.Name != pair.Item2, TimeSpan.FromSeconds(10)),
                    $"{pair.Item2} leaves its question standing");
            }
            Assert.Equal(Questions.Length, answered.Count);
            Assert.True(App.Eventually(() => app.NoticeBar() is null, TimeSpan.FromSeconds(10)),
                "with every question answered, no notice is left on the mail screen");
        });
}
