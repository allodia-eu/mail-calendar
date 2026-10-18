// Runs a suite's cases in the order they are written, for a suite whose cases build on one another
// (a meeting one case organises and the next answers). xUnit otherwise picks its own order.

using Xunit.Abstractions;
using Xunit.Sdk;

namespace Allodia.Mailcal.UiTests.Suites;

public sealed class DeclarationOrderer : ITestCaseOrderer
{
    public const string Name = "Allodia.Mailcal.UiTests.Suites.DeclarationOrderer";
    public const string Assembly = "Mailcal.UiTests";

    public IEnumerable<TTestCase> OrderTestCases<TTestCase>(IEnumerable<TTestCase> testCases)
        where TTestCase : ITestCase =>
        testCases.OrderBy(testCase => testCase.TestMethod.Method.ToRuntimeMethod().MetadataToken);
}
