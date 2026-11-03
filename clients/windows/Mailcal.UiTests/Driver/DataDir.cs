// Where the app under test keeps its writable state: the same brand value AppPaths.Root reads, so a
// suite looks where this build writes rather than where one brand happens to.

namespace Allodia.Mailcal.UiTests.Driver;

internal static class DataDir
{
    /// <summary>The app's data root: <c>%LOCALAPPDATA%</c> and the brand's <c>MAILCAL_WINDOWS_DATA_DIR</c>.</summary>
    internal static string Root { get; } = Path.Combine(
        Environment.GetFolderPath(Environment.SpecialFolder.LocalApplicationData),
        Brand.WindowsDataDir.Replace('/', Path.DirectorySeparatorChar));
}
