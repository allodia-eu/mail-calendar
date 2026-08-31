// The debug-only harness-account resolution, split out of MailboxModel.Accounts.cs to keep that
// file under the 500-line limit and to gather all MAILCAL_DEV_ACCOUNT logic in one place. Every
// member here is compiled out of a release build (the whole file is under #if DEBUG), so a shipped
// binary carries no canned credentials and no dev-store path.

using uniffi.mailcal_bindings;

namespace Allodia.Mailcal.Services;

#if DEBUG
public sealed partial class MailboxModel
{
    /// <summary>The accounts a dev launch injects, and the store subdirectory they live in.</summary>
    private sealed record DevAccount(string[] Configs, string StoreSubdir);

    /// <summary>
    /// Resolves the accounts to connect at startup and the engine-store subdir they use. For a
    /// harness dev account (<c>stalwart</c> / <c>stalwart-imap</c>) this ALSO switches the credential
    /// store to an isolated dev namespace (<see cref="CredentialStore.UseDevNamespace"/>), so an
    /// account added or removed through the setup form during a harness run persists in a throwaway
    /// store and never touches, or reorders the index of, the developer's real accounts. The canned
    /// harness account is injected fresh each launch (never persisted); anything added through the
    /// form is stored under its real id in the dev namespace, so it persists across dev relaunches
    /// and Remove deletes it, production behaviour, sandboxed. Returns the stored real accounts (and
    /// a <c>null</c> subdir) for a normal launch.
    /// </summary>
    private static (string[] Configs, string? StoreSubdir) ResolveStartupAccounts()
    {
        var dev = ResolveDevAccount();
        if (dev is null)
        {
            return (CredentialStore.Configs(), null);
        }
        CredentialStore.UseDevNamespace(dev.StoreSubdir);
        // Canned harness account first (always injected), then whatever a previous dev session left
        // in the isolated dev namespace: an account added through the form, and the Allodia account
        // a sign-in stores, which is not a mail account at all, and which the core takes back out
        // before anything reads it as one. Reading the store here is what keeps both; Apple does the
        // same, and Android carries only the second because its store is not namespaced per dev
        // account. Re-adding the canned account itself through the form is the one case that would
        // duplicate it, a non-issue for a throwaway harness store, and cleared by wiping the dev
        // namespace.
        string[] configs = [.. dev.Configs, .. CredentialStore.Configs()];
        return (configs, dev.StoreSubdir);
    }

    /// <summary>
    /// The canned IMAP account config injected for <c>MAILCAL_DEV_ACCOUNT=stalwart-imap</c>. IMAP
    /// exercises the full mail-action surface (mark-read/flag, archive, delete, move) plus IDLE
    /// push, unlike the JMAP config. Hand-written rather than built through the shared config
    /// builder (as the JMAP config is): the harness dials the implicit-TLS IMAP by IP
    /// (<c>127.0.0.1:12993</c>) but must present <c>server_name = localhost</c>, the only SAN on
    /// Stalwart's self-signed cert, and the builder always derives <c>server_name</c> from the
    /// dialed host with no override (adding one purely for the harness would be the sort of
    /// test-server knob <c>AGENTS.md</c> forbids). The debug-only TLS policy adds that cert as a
    /// custom root from <c>MAILCAL_EXTRA_CA</c>, which <c>build-and-run.ps1</c> asserts is readable.
    /// </summary>
    /// <summary>
    /// The harness over IMAP, with the SMTP and CalDAV halves the invitation path needs.
    /// </summary>
    /// <remarks>
    /// <para>
    /// This mode used to be IMAP alone, which made it the one dev account that could not exercise
    /// the shape it most resembles: mail in a mailbox beside a calendar on a different server,
    /// which is what every IMAP+CalDAV provider is and what meeting invitations break on. With no
    /// SMTP the account cannot send a reply, and with no CalDAV it has nothing to answer *on*,
    /// so the invitation card correctly said the account could not answer, and no amount of
    /// testing here would ever have reached the code under test.
    /// </para>
    /// <para>
    /// Loopback throwaway credentials, debug builds only, in their own store and credential
    /// namespace, the same guarantees as the IMAP half above.
    /// </para>
    /// </remarks>
    private const string StalwartDevImapToml = """
        [imap]
        addr = "127.0.0.1:12993"
        server_name = "localhost"
        username = "alice@test.local"
        password = "harness-alice-pw"

        [smtp]
        addr = "127.0.0.1:12587"
        server_name = "localhost"
        security = "starttls"

        [caldav]
        base_url = "http://127.0.0.1:28080"
        username = "alice@test.local"
        password = "harness-alice-pw"
        """;

    /// <summary>
    /// One harness JMAP account config, built through the shared config builder, the same FFI the
    /// real setup form uses (<c>AccountConfigToml</c>'s JMAP counterpart), so this canned fixture
    /// can't silently drift from the <c>[jmap]</c> schema. Loopback-only throwaway credentials,
    /// never a real account; the explicit <c>http://</c> is preserved for this local fixture. The
    /// inputs are constant and valid, so the builder never throws.
    /// </summary>
    private static string StalwartDevJmapToml(
        string email = "alice@test.local",
        string password = "harness-alice-pw") =>
        MailcalBindingsMethods.JmapAccountConfigToml(new JmapSetup(
            Email: email,
            ServerUrl: "http://127.0.0.1:28080",
            Password: password));

    /// <summary>
    /// Dev/verification only: when <c>MAILCAL_DEV_ACCOUNT</c> names a harness mode, boot against
    /// the local seeded Stalwart harness (<c>docker/stalwart</c>) by injecting a canned config,
    /// bypassing the setup form, so a developer, or an automated debug run, targets the throwaway
    /// loopback mailbox instead of personal accounts. <c>stalwart</c> connects over JMAP (plaintext
    /// <c>127.0.0.1:28080</c>, no push); <c>stalwart-multi</c> the same over TWO accounts;
    /// <c>stalwart-imap</c> over implicit-TLS IMAP, which adds mail actions and IDLE push. Each gets
    /// its own engine store AND its own credential namespace, so none mixes with real accounts nor
    /// with the others. Returns <c>null</c> for <c>personal</c>/unset, so the stored accounts are
    /// used. Every connection is made by the Rust core, so no platform HTTP policy applies.
    /// Compiled only into debug builds.
    /// </summary>
    /// <summary>Whether this run is connected to the local Stalwart harness rather than to stored
    /// accounts, the condition under which a send outside <c>test.local</c> is refused
    /// (<see cref="HarnessRecipientGate"/>). Always false in a release build, where the dev-account
    /// switch is compiled out entirely.</summary>
    internal static bool IsHarnessDevAccount
    {
#if DEBUG
        get => (Environment.GetEnvironmentVariable("MAILCAL_DEV_ACCOUNT")?.Trim().ToLowerInvariant()
            ?? string.Empty).StartsWith("stalwart", StringComparison.Ordinal);
#else
        get => false;
#endif
    }

    private static DevAccount? ResolveDevAccount()
    {
        var raw = Environment.GetEnvironmentVariable("MAILCAL_DEV_ACCOUNT")?.Trim().ToLowerInvariant();
        switch (raw)
        {
            case null or "" or "personal":
                return null;
            case "stalwart":
                Log.Info("MAILCAL_DEV_ACCOUNT=stalwart, connecting the local harness over JMAP");
                return new DevAccount([StalwartDevJmapToml()], AppPaths.DevStoreSubdir(raw)!);
            case "stalwart-multi":
                // Two harness accounts at once. It exists for CONTACTS: the engine merges people
                // across accounts on a shared address, and a single-account boot cannot show that,
                // the seeded `shared-*` card is filed in alice's book AND bob's precisely so this
                // mode renders it as one row marked "In 2 accounts".
                Log.Info("MAILCAL_DEV_ACCOUNT=stalwart-multi, connecting the local harness over JMAP as two accounts (alice + bob)");
                return new DevAccount(
                    [
                        StalwartDevJmapToml(),
                        StalwartDevJmapToml("bob@test.local", "harness-bob-pw"),
                    ],
                    AppPaths.DevStoreSubdir(raw)!);
            case "stalwart-imap":
                Log.Info("MAILCAL_DEV_ACCOUNT=stalwart-imap, connecting the local harness over IMAP (mail actions + IDLE)");
                return new DevAccount([StalwartDevImapToml], AppPaths.DevStoreSubdir(raw)!);
            case "first-run":
                // Injects nothing. The namespace starts empty, so the app opens on the screens a
                // person sees once; anything added through the form persists there and is wiped
                // with the directory.
                Log.Info("MAILCAL_DEV_ACCOUNT=first-run, an empty namespace: the first-account screen");
                return new DevAccount([], AppPaths.DevStoreSubdir(raw)!);
            default:
                // A recognised-elsewhere dev mode this client doesn't support (e.g. demo). Fall back
                // to the stored accounts, but say so loudly rather than silently connecting the
                // developer's real accounts as if the switch were ignored.
                Log.Warn($"MAILCAL_DEV_ACCOUNT='{raw}' is not supported on Windows; using stored accounts. Use 'stalwart' (JMAP), 'stalwart-multi' (two JMAP accounts), 'stalwart-imap' (IMAP) or 'first-run' (an empty namespace) here.");
                return null;
        }
    }
}
#endif
