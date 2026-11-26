// The new-mail notifications choice a launch carries from notifications.txt into the core: off is
// sent once, anything else is the core's own default and sends nothing, and the file goes either
// way so the choice is never carried twice.

using System.Collections.Generic;
using System.IO;
using Allodia.Mailcal.Services;
using uniffi.mailcal_bindings;
using Xunit;

namespace Allodia.Mailcal.Tests;

public sealed class NotificationsImportTests : IDisposable
{
    private readonly string _dir =
        Path.Combine(Path.GetTempPath(), $"mailcal-notifications-{Guid.NewGuid():N}");
    private readonly List<PreferenceIntent> _sent = [];

    public NotificationsImportTests() => Directory.CreateDirectory(_dir);

    public void Dispose() => Directory.Delete(_dir, recursive: true);

    private string FilePath => NotificationsImport.FileIn(_dir);

    private bool Run() => NotificationsImport.Run(FilePath, _sent.Add);

    [Fact]
    public void TheFileSitsBesideTheOtherPreferences() =>
        Assert.Equal(Path.Combine(_dir, "notifications.txt"), FilePath);

    [Theory]
    [InlineData("off")]
    [InlineData("off\n")]
    [InlineData("  off\r\n")]
    public void OffIsSentOnce(string stored)
    {
        File.WriteAllText(FilePath, stored);

        Assert.True(Run());

        var sent = Assert.Single(_sent);
        var set = Assert.IsType<PreferenceIntent.SetNewMailNotifications>(sent);
        Assert.False(set.On);
        Assert.False(File.Exists(FilePath));

        Assert.False(Run());
        Assert.Single(_sent);
    }

    [Theory]
    [InlineData("on")]
    [InlineData("")]
    [InlineData("OFF")]
    [InlineData("disabled")]
    public void AnythingElseSendsNothingAndStillGoes(string stored)
    {
        File.WriteAllText(FilePath, stored);

        Assert.False(Run());

        Assert.Empty(_sent);
        Assert.False(File.Exists(FilePath));
    }

    [Fact]
    public void NoFileSendsNothing()
    {
        Assert.False(Run());
        Assert.Empty(_sent);
    }

    [Fact]
    public void NoDirectorySendsNothing()
    {
        var path = NotificationsImport.FileIn(Path.Combine(_dir, "absent"));
        Assert.False(NotificationsImport.Run(path, _sent.Add));
        Assert.Empty(_sent);
    }
}
