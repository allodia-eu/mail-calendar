// Why a refused write says what it says.
//
// ⚠️ The alternative is what shipped until now: the card described the **exception**, and UniFFI
// builds one out of the variant's own fields, so a refusal reached the screen as the literal text
// `reason=NotSwitchable`. An unreachable service is worse, because its exception carries no message
// at all and CoreError then falls back to the type name, so the commonest failure of the four read
// "That didn't work: Unreachable". The purchasing contract asks for the code either way: "you have
// already cancelled" and "that cannot change while a charge is being retried" are different things
// to say, and only the code tells them apart.

using System;
using Allodia.Mailcal.Services;
using uniffi.mailcal_bindings;
using Xunit;

namespace Allodia.Mailcal.Tests;

public class AllodiaWriteFailureTests
{
    private static Exception Refused(AllodiaSubscriptionRefusal reason) =>
        new AllodiaPurchaseException.Refused(reason);

    // One [Fact] over the pairs rather than a [Theory] over them: the generated refusal enum is
    // `internal`, so it cannot cross a public test signature (CS0051), which is the same rule the
    // product code follows for every method naming one.
    [Fact]
    public void EachRefusalTheServiceNamesEarnsItsOwnSentence()
    {
        var expected = new[]
        {
            (AllodiaSubscriptionRefusal.AlreadyCancelled, AllodiaWriteFailure.AlreadyCancelled),
            (AllodiaSubscriptionRefusal.AlreadyActive, AllodiaWriteFailure.AlreadyActive),
            (AllodiaSubscriptionRefusal.AlreadyOnInterval, AllodiaWriteFailure.AlreadyOnInterval),
            (AllodiaSubscriptionRefusal.NotSwitchable, AllodiaWriteFailure.NotSwitchable),
            (AllodiaSubscriptionRefusal.NotFound, AllodiaWriteFailure.NotFound),
        };
        foreach (var (reason, sentence) in expected)
        {
            Assert.Equal(sentence, AllodiaWriteResult.FailureFor(Refused(reason)));
        }
    }

    // Billing that is not configured on this deployment, and a reason a later service invents, both
    // leave a client with nothing specific it could truthfully say.
    [Fact]
    public void ARefusalThisBuildCannotExplainFallsBackRatherThanGuessing()
    {
        Assert.Equal(
            AllodiaWriteFailure.Unexplained,
            AllodiaWriteResult.FailureFor(Refused(AllodiaSubscriptionRefusal.Unavailable)));
    }

    // An outage learned nothing, so there is nothing to explain: it is not evidence about the
    // subscription, which is the distinction the entitlement contract is built on.
    [Fact]
    public void AnythingThatIsNotARefusalIsUnexplained()
    {
        Assert.Equal(
            AllodiaWriteFailure.Unexplained,
            AllodiaWriteResult.FailureFor(new AllodiaPurchaseException.Unreachable()));
        Assert.Equal(
            AllodiaWriteFailure.Unexplained,
            AllodiaWriteResult.FailureFor(new AllodiaPurchaseException.NotSignedIn()));
        Assert.Equal(
            AllodiaWriteFailure.Unexplained,
            AllodiaWriteResult.FailureFor(new InvalidOperationException("something else")));
    }

    // The record carries the code rather than a sentence, so the model half stays free of L10n and
    // the words stay in one place.
    [Fact]
    public void AFailedWriteCarriesTheCodeAndNothingElse()
    {
        var result = AllodiaWriteResult.Failed(AllodiaWriteFailure.NotSwitchable);
        Assert.Equal(AllodiaWriteOutcome.Failed, result.Outcome);
        Assert.Equal(AllodiaWriteFailure.NotSwitchable, result.Failure);
        Assert.Null(result.EndDate);
        Assert.Null(result.Change);
    }
}
