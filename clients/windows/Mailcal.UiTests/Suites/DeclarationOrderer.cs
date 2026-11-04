// Runs a suite's cases in the order they are written, for a suite whose cases build on one another
// (a meeting one case organises and the next answers). xUnit otherwise picks its own order. It
// orders test methods, which is the order xUnit runs them in; a test case orderer orders only the
// cases within one method, a theory's rows, and leaves the methods where xUnit put them.

using Xunit.Sdk;
using Xunit.v3;

namespace Allodia.Mailcal.UiTests.Suites;

public sealed class DeclarationOrderer : ITestMethodOrderer
{
    // A case with no method of its own (null here) has no place in the source, so it goes last.
    public IReadOnlyCollection<TTestMethod?> OrderTestMethods<TTestMethod>(IReadOnlyCollection<TTestMethod?> testMethods)
        where TTestMethod : notnull, ITestMethod =>
        [.. testMethods.OrderBy(testMethod => testMethod is IXunitTestMethod method ? method.Method.MetadataToken : int.MaxValue)];
}
