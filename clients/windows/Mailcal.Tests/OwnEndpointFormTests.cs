// The own AI endpoint form (docs/ai.md, "Where requests go"). The key field is never filled back
// in, so an empty field must keep the stored key: read as "no key", every edit of the address
// would quietly remove it. And a refusal is worded from its code, never from the exception's text.

using System;
using Allodia.Mailcal.Services;
using uniffi.mailcal_bindings;
using Xunit;

namespace Allodia.Mailcal.Tests;

public class OwnEndpointFormTests
{
    [Fact]
    public void AnEmptyKeyFieldKeepsTheStoredKey()
    {
        Assert.Null(OwnEndpointForm.KeyArgument(string.Empty));
        Assert.Null(OwnEndpointForm.KeyArgument("   "));
        Assert.Equal("sk-local", OwnEndpointForm.KeyArgument("sk-local"));
    }

    // One [Fact] over the pairs: the generated exceptions are `internal` (CS0051 on a [Theory]).
    [Fact]
    public void EachRefusalNamesTheFieldToFix()
    {
        Assert.Equal(OwnEndpointProblem.Address, OwnEndpointForm.ProblemFor(new OwnEndpointException.InvalidUrl()));
        Assert.Equal(OwnEndpointProblem.Https, OwnEndpointForm.ProblemFor(new OwnEndpointException.NotHttps()));
        Assert.Equal(OwnEndpointProblem.Model, OwnEndpointForm.ProblemFor(new OwnEndpointException.NoModel()));
        Assert.Equal(
            OwnEndpointProblem.Keystore,
            OwnEndpointForm.ProblemFor(new OwnEndpointException.Keystore("the store's own words")));
        Assert.Equal(
            OwnEndpointProblem.Keystore,
            OwnEndpointForm.ProblemFor(new InvalidOperationException("internal detail")));
    }

    // The consent sheet names where the words go by host, including a server on this computer.
    [Fact]
    public void TheConsentNamesTheHost()
    {
        Assert.Equal("api.example.eu", OwnEndpointForm.Host("https://api.example.eu/v1"));
        Assert.Equal("127.0.0.1", OwnEndpointForm.Host("http://127.0.0.1:28434/v1"));
        Assert.Equal("not an address", OwnEndpointForm.Host(" not an address "));
    }
}
