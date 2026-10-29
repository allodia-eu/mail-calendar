// The app's message catalog (messages/<locale>.json), for assertions on copy.

using System.Text.Json.Nodes;

namespace Allodia.Mailcal.UiTests.Driver;

internal static class Catalog
{
    private static readonly Lazy<Dictionary<string, JsonObject>> Locales = new(() =>
        Directory.EnumerateFiles(Path.Combine(App.Repo, "messages"), "*.json")
            .ToDictionary(file => Path.GetFileNameWithoutExtension(file)!, file => JsonNode.Parse(File.ReadAllText(file))!.AsObject()));

    /// <summary>The name this build was given, which the l10n codegen writes into the generated
    /// resources: it is injected (docs/branding.md), so an unbranded build reads otherwise than the
    /// catalog's.</summary>
    internal static string AppTitle()
    {
        var resw = Path.Combine(App.Repo, "clients", "windows", "Mailcal", "Strings", "en", "Resources.resw");
        if (!File.Exists(resw))
        {
            throw new InvalidOperationException($"no generated {resw}; build the client first (build-and-run.ps1 -NoRun)");
        }
        return System.Xml.Linq.XDocument.Load(resw).Descendants("data")
            .FirstOrDefault(d => (string?)d.Attribute("name") == "app_title")?.Element("value")?.Value
            ?? throw new InvalidOperationException($"the generated {resw} carries no app_title");
    }

    /// <summary>The English value of <paramref name="key"/>: the harness store comes up in English.</summary>
    internal static string English(string key) =>
        (string?)Locales.Value["en"][key] ?? throw new InvalidOperationException($"the catalog defines no '{key}'");

    /// <summary>
    /// Every locale's value of <paramref name="key"/>. For a label on screen in whatever language
    /// the app came up in: matching the set asks whether this is the element bound to that key,
    /// without re-deriving the app's choice of language.
    /// </summary>
    internal static IReadOnlySet<string> Everywhere(string key)
    {
        var values = Locales.Value.Values.Select(locale => (string?)locale[key]).OfType<string>().ToHashSet();
        return values.Count > 0 ? values : throw new InvalidOperationException($"no catalog locale defines '{key}', the key was renamed or removed");
    }
}
