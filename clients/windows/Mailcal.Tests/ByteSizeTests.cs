// How a file's size is worded on its attachment row and its composer tile (Services/ByteSize.cs).
using System.Globalization;
using Allodia.Mailcal.Services;
using Xunit;

namespace Allodia.Mailcal.Tests;

public sealed class ByteSizeTests
{
    private static string In(string culture, ulong bytes)
    {
        var before = CultureInfo.CurrentCulture;
        CultureInfo.CurrentCulture = CultureInfo.GetCultureInfo(culture);
        try
        {
            return ByteSize.Format(bytes);
        }
        finally
        {
            CultureInfo.CurrentCulture = before;
        }
    }

    [Fact]
    public void ASizeIsWordedInBinaryUnitsWithOneDecimal()
    {
        Assert.Equal("0 B", In("en-GB", 0));
        Assert.Equal("1023 B", In("en-GB", 1023));
        Assert.Equal("1 KB", In("en-GB", 1024));
        Assert.Equal("1.5 KB", In("en-GB", 1536));
        Assert.Equal("3.8 MB", In("en-GB", 4_000_000));
        Assert.Equal("2 GB", In("en-GB", 2UL * 1024 * 1024 * 1024));
    }

    [Fact]
    public void TheDecimalSeparatorIsTheAppsLanguagesOwn()
    {
        Assert.Equal("1,5 KB", In("nl-NL", 1536));
        Assert.Equal("1,5 KB", In("de-DE", 1536));
    }
}
