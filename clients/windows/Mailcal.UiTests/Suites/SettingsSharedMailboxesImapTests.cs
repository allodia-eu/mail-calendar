// The Shared mailboxes section of an IMAP account's page (docs/shared-mailboxes.md rule 1): what the
// server's shared namespace holds for the sign-in is listed, a switch apiece, none of them chosen.
//
// WHY HERE. SettingsSharedMailboxesTests drives the section over JMAP; an IMAP server answers from
// NAMESPACE and ACL instead, and the harness gives alice three shapes there: the support group's
// mailbox, bob's Inbox read-only and dave's Board folder without his Inbox. Nothing is switched on:
// the IMAP store is shared by every suite in its collection.

using Allodia.Mailcal.UiTests.Driver;
using Xunit;

namespace Allodia.Mailcal.UiTests.Suites;

[Collection(ImapHarnessCollection.Name)]
[Trait("Dataset", "harness")]
public sealed class SettingsSharedMailboxesImapTests(ImapHarnessApp harness)
{
    [HarnessFact]
    public void EveryMailboxTheSharedNamespaceHoldsIsASwitchAndNoneIsChosen() => harness.Case(nameof(EveryMailboxTheSharedNamespaceHoldsIsASwitchAndNoneIsChosen), app =>
    {
        var dialog = new SetupFlow(app).OpenAccount(Harness.Alice);
        List<Element> switches = [];
        App.Eventually(() => (switches = app.Agent.FindAll(new Filter { Under = dialog })
            .Where(e => e.Id.StartsWith("SharedMailbox_", StringComparison.Ordinal)).ToList()).Count >= 3, TimeSpan.FromSeconds(30));
        var names = string.Join(" | ", switches.Select(s => s.Id));
        foreach (var owner in new[] { "support", "bob", "dave" })
        {
            Assert.True(switches.Any(s => s.Id.Contains(owner, StringComparison.OrdinalIgnoreCase)),
                $"the shared namespace holds {owner}'s mailbox for alice, and the page has no switch for it. Switches: {names}");
        }
        Assert.All(switches, s => Assert.Equal("Off", app.Agent.Read(s).Toggle));
    }, app => app.CloseDialogs());
}
