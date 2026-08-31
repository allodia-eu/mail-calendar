// The setup form's JMAP "Sign in with your provider" gate, as arithmetic. What it pins is what
// cannot be seen by looking at the running app:
//   - the button is offered ONLY for a server the core's pre-flight said offers sign-in, sign-in
//     is discovered, not guaranteed, and a button that dead-ends is worse than no button;
//   - an answer belongs to the address it was asked about, so a slow probe returning after the
//     user typed on cannot light the button for a server nobody asked about;
//   - a failed sign-in leaves the password/API-token field usable. OAuth is an ADDITION to that
//     field, never a replacement, so no failure may leave the user with no way in.
//
// This is the WinUI-free half of the flow (JmapSignInGate), so it runs in the plain net10.0 test
// assembly, no renderer, no cdylib, no network.
using Allodia.Mailcal.Services;
using Xunit;

namespace Allodia.Mailcal.Tests;

public class JmapSignInGateTests
{
    private const string Email = "alice@example.com";

    // Walks a gate to "this server offers sign-in", the way the view does: type, probe, answer.
    private static JmapSignInGate Offered(string email = Email, string server = "")
    {
        var gate = new JmapSignInGate();
        gate.FieldsChanged(email, server);
        var key = gate.BeginProbe();
        Assert.NotNull(key);
        gate.Probed(key!, available: true);
        return gate;
    }

    [Fact]
    public void The_button_is_offered_only_once_the_server_says_it_offers_sign_in()
    {
        var gate = new JmapSignInGate();
        gate.FieldsChanged(Email, string.Empty);

        // Nothing asked yet: no button, and certainly no failure note.
        Assert.False(gate.ShowButton);
        Assert.False(gate.ShowFailure);

        // Asked and told no: still no button, the password/API-token field is the only way in.
        var key = gate.BeginProbe();
        Assert.NotNull(key);
        gate.Probed(key!, available: false);
        Assert.False(gate.ShowButton);
        Assert.True(gate.ManualEnabled);

        // Told yes: the button appears, and can be pressed.
        Assert.True(Offered().ShowButton);
        Assert.True(Offered().ButtonEnabled);
    }

    [Fact]
    public void An_answer_for_a_different_address_never_lights_the_button()
    {
        // The probe is a slow network call. If the user types on while it is out, its answer is
        // about a server they are no longer connecting to, showing a button for it would open a
        // sign-in against the wrong provider.
        var gate = new JmapSignInGate();
        gate.FieldsChanged(Email, string.Empty);
        var key = gate.BeginProbe();
        Assert.NotNull(key);

        gate.FieldsChanged("bob@other.example", string.Empty);
        gate.Probed(key!, available: true);

        Assert.False(gate.ShowButton);
    }

    [Fact]
    public void A_probe_is_skipped_for_a_half_typed_address_and_for_fields_already_answered()
    {
        var gate = new JmapSignInGate();

        // No domain to discover against yet, every one of these would be a wasted round trip.
        gate.FieldsChanged("alice", string.Empty);
        Assert.Null(gate.BeginProbe());
        gate.FieldsChanged("alice@", string.Empty);
        Assert.Null(gate.BeginProbe());

        // A real address probes once; the same fields never probe again.
        gate.FieldsChanged(Email, string.Empty);
        var key = gate.BeginProbe();
        Assert.NotNull(key);
        Assert.Null(gate.BeginProbe()); // already in flight
        gate.Probed(key!, available: true);
        Assert.Null(gate.BeginProbe()); // already answered

        // A different server for the same address is a different question, so it does probe.
        gate.FieldsChanged(Email, "https://jmap.example.com");
        Assert.NotNull(gate.BeginProbe());
    }

    [Fact]
    public void A_failed_sign_in_leaves_the_manual_path_enabled_and_says_so()
    {
        var gate = Offered();

        // While the browser step is out, neither the button nor the secret field may be used.
        gate.SignInStarted();
        Assert.False(gate.ButtonEnabled);
        Assert.False(gate.ManualEnabled);

        gate.SignInFinished(JmapSignInOutcome.Failed);

        // The whole point: a failure is never a dead end. The note explains, the secret field is
        // back, and the button is still there to try again.
        Assert.True(gate.ShowFailure);
        Assert.True(gate.ManualEnabled);
        Assert.True(gate.ShowButton);
        Assert.True(gate.ButtonEnabled);
    }

    [Fact]
    public void A_cancelled_sign_in_re_enables_the_manual_path_without_claiming_a_failure()
    {
        var gate = Offered();
        gate.SignInStarted();
        gate.SignInFinished(JmapSignInOutcome.Cancelled);

        // Backing out of the browser is not an error to report at the user.
        Assert.False(gate.ShowFailure);
        Assert.True(gate.ManualEnabled);
        Assert.True(gate.ButtonEnabled);
    }

    [Fact]
    public void Editing_the_address_after_a_failure_clears_the_note_and_the_offer()
    {
        var gate = Offered();
        gate.SignInStarted();
        gate.SignInFinished(JmapSignInOutcome.Failed);

        gate.FieldsChanged("bob@other.example", string.Empty);

        // Both belonged to the old address: a different provider has neither been asked nor failed.
        Assert.False(gate.ShowFailure);
        Assert.False(gate.ShowButton);
    }

    [Fact]
    public void A_detected_card_offers_the_sign_in_instead_of_the_secret_field_not_beside_it()
    {
        var gate = new JmapSignInGate();
        gate.CardChanged(detected: true);
        gate.FieldsChanged(Email, string.Empty);

        // Before the server has answered, the manual path is all there is, it must be showing.
        Assert.True(gate.ShowManualSecret);

        var key = gate.BeginProbe();
        gate.Probed(key!, available: true);

        // Now there is a one-click route. Showing the secret box too would ask the user to choose
        // between two ways to do one job, and put "just add your password" next to "there's no API
        // token to create".
        Assert.True(gate.ShowButton);
        Assert.False(gate.ShowManualSecret);
    }

    [Fact]
    public void A_failed_sign_in_brings_the_secret_field_back_on_a_detected_card()
    {
        var gate = new JmapSignInGate();
        gate.CardChanged(detected: true);
        gate.FieldsChanged(Email, string.Empty);
        gate.Probed(gate.BeginProbe()!, available: true);
        Assert.False(gate.ShowManualSecret);

        gate.SignInStarted();
        gate.SignInFinished(JmapSignInOutcome.Failed);

        // The load-bearing half of hiding it: a server that declines this flow may never leave the
        // user with no way in.
        Assert.True(gate.ShowManualSecret);
        Assert.True(gate.ManualEnabled);
    }

    [Fact]
    public void A_server_that_offers_no_sign_in_keeps_the_secret_field_on_a_detected_card()
    {
        var gate = new JmapSignInGate();
        gate.CardChanged(detected: true);
        gate.FieldsChanged(Email, string.Empty);
        gate.Probed(gate.BeginProbe()!, available: false);

        Assert.False(gate.ShowButton);
        Assert.True(gate.ShowManualSecret);
    }

    [Fact]
    public void The_manual_form_keeps_the_secret_field_even_when_sign_in_is_offered()
    {
        // Someone who chose "Set up manually" asked for the fields; the offer sits above them
        // rather than replacing them (this is where Android's two screens differ, deliberately).
        var gate = Offered();
        Assert.True(gate.ShowButton);
        Assert.True(gate.ShowManualSecret);

        // And arriving here FROM a detected card, the "Set up manually" link, reveals them again.
        var detected = new JmapSignInGate();
        detected.CardChanged(detected: true);
        detected.FieldsChanged(Email, string.Empty);
        detected.Probed(detected.BeginProbe()!, available: true);
        Assert.False(detected.ShowManualSecret);
        detected.CardChanged(detected: false);
        Assert.True(detected.ShowManualSecret);
    }

    [Fact]
    public void Reset_returns_the_form_to_its_first_run_state()
    {
        var gate = Offered();
        gate.SignInStarted();
        gate.SignInFinished(JmapSignInOutcome.Failed);

        gate.Reset();

        // Adding a second account must not inherit the first one's answer or its failure note.
        Assert.False(gate.ShowButton);
        Assert.False(gate.ShowFailure);
        Assert.True(gate.ManualEnabled);
    }
}
