// What the two suites about "The organiser wasn't told" share: the account's remembered answer,
// which would make the core answer for the user and ask nothing, and the sentence the question is
// put in (docs/invitations.md, "A server can promise to send the reply, and then not send it").

using Allodia.Mailcal.UiTests.Driver;

namespace Allodia.Mailcal.UiTests.Suites;

internal static class ReplyPromptFlow
{
    /// <summary>Where the core keeps the preferences of MAILCAL_DEV_ACCOUNT=stalwart-imap
    /// (AppPaths.PrefsDir), a remembered answer among them.</summary>
    internal static readonly string PrefsFile = Path.Combine(
        Environment.GetFolderPath(Environment.SpecialFolder.LocalApplicationData), "Allodia", "MailCalendar", "dev-imap", "preferences.toml");

    /// <summary>Whether the account holds a standing answer.</summary>
    internal static bool Remembered() =>
        File.Exists(PrefsFile) && File.ReadLines(PrefsFile).Any(line => line.Contains("invitation_reply_fallback", StringComparison.Ordinal));

    /// <summary>
    /// Forgets the account's standing answer. Called before a case asks, so a run heals what the
    /// last one left, including one that died between ticking the box and getting here. Safe under
    /// the running app: the core reads the file on each RSVP and writes it whole from that read.
    /// </summary>
    internal static void ForgetRemembered()
    {
        if (!File.Exists(PrefsFile))
        {
            return;
        }
        var kept = new List<string>();
        var inTable = false;
        foreach (var line in File.ReadAllLines(PrefsFile))
        {
            if (line.TrimStart().StartsWith('['))
            {
                inTable = line.Trim() == "[invitation_reply_fallback]";
            }
            if (!inTable)
            {
                kept.Add(line);
            }
        }
        File.WriteAllLines(PrefsFile, kept);
    }

    /// <summary>
    /// Whether <paramref name="texts"/> hold the catalog's undelivered-body sentence with the meeting
    /// and the organiser's address in its slots, in any locale. Built from the template, so the whole
    /// sentence is pinned, the order of its clauses included: the answer was saved, said first.
    /// </summary>
    internal static bool ShowsBody(IEnumerable<string> texts, string summary, string organiser)
    {
        var expected = Catalog.Everywhere("invitation_reply_undelivered_body")
            .Select(template => template.Replace("{summary}", summary).Replace("{organizer}", organiser))
            .ToHashSet();
        return texts.Any(expected.Contains);
    }

    /// <summary>Every text drawn under <paramref name="root"/>.</summary>
    internal static List<string> Texts(App app, Element root) =>
        app.Agent.FindAll(new Filter { Type = "Text", Under = root }).Select(t => t.Name).Where(n => n.Length > 0).ToList();

    /// <summary>The button under <paramref name="root"/> labelled with <paramref name="key"/> in any
    /// locale, or null.</summary>
    internal static Element? Button(App app, Element root, string key)
    {
        var labels = Catalog.Everywhere(key);
        return app.Agent.FindAll(new Filter { Type = "Button", Under = root, OnScreen = true }).FirstOrDefault(b => labels.Contains(b.Name));
    }
}
