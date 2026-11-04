// The order the suites run in, the same on every machine. xUnit's own order is stable on one
// machine and differs between two, because it hashes the assembly's path into the ids it sorts by,
// so a run that passed on a developer's machine says nothing about the order CI runs. Suites
// sharing a launch leave the app as their last case left it, so their order is part of what a run
// tests. Applied to the whole assembly (UiApp.cs).

using Xunit.Sdk;
using Xunit.v3;

namespace Allodia.Mailcal.UiTests.Suites;

/// <summary>Every suite's cases in the order they are written, which is what lets a suite's cases
/// build on one another (a meeting one case organises and the next answers). It orders test
/// methods, the order xUnit runs them in; a test case orderer orders only the cases within one
/// method, a theory's rows.</summary>
public sealed class DeclarationOrderer : ITestMethodOrderer
{
    // A case with no method of its own (null here) has no place in the source, so it goes last.
    public IReadOnlyCollection<TTestMethod?> OrderTestMethods<TTestMethod>(IReadOnlyCollection<TTestMethod?> testMethods)
        where TTestMethod : notnull, ITestMethod =>
        [.. testMethods.OrderBy(testMethod => testMethod is IXunitTestMethod method ? method.Method.MetadataToken : int.MaxValue)];
}

/// <summary>Collections, and the suites within one, by name.</summary>
public sealed class NameOrderer : ITestCollectionOrderer, ITestClassOrderer
{
    public IReadOnlyCollection<TTestCollection> OrderTestCollections<TTestCollection>(IReadOnlyCollection<TTestCollection> testCollections)
        where TTestCollection : ITestCollection =>
        [.. testCollections.OrderBy(collection => collection.TestCollectionDisplayName, StringComparer.Ordinal)];

    public IReadOnlyCollection<TTestClass?> OrderTestClasses<TTestClass>(IReadOnlyCollection<TTestClass?> testClasses)
        where TTestClass : notnull, ITestClass =>
        [.. testClasses.OrderBy(testClass => testClass?.TestClassName, StringComparer.Ordinal)];
}
