//! Supplies the analytics relay endpoint `src/analytics.rs` reads with `option_env!`.
//!
//! The mechanism is [`mailcal_buildenv`], shared with `mailcal-oauth`; the policy is here.
//!
//! Without this script the two names below were readable **only** from the real environment, so a
//! line in the repo's `.env` was a silent no-op and `option_env!`'s invisibility to cargo's
//! fingerprint left a cached crate holding the old empty value. Both failures look identical from
//! outside: an app that says `analytics: no relay endpoint in this build` and sends nothing, which
//! reads as a broken feature rather than a build that never got the value.

use mailcal_buildenv::{Injected, REQUIRE_VAR};

/// The relay's base URL. Absent (the default, and every from-source build) means no sink is
/// constructed and the build sends nothing at all.
const RELAY_URL_VAR: &str = "ALLODIA_TELEMETRY_URL";

/// The app key identifying this product to the relay. **Not a secret** (it ships in the binary),
/// and not required: `src/analytics.rs` falls back to a default, so a build that has the endpoint
/// but not this still reports.
const RELAY_APP_KEY_VAR: &str = "ALLODIA_TELEMETRY_APP_KEY";

fn main() {
    let injected = Injected::load();
    injected.export(&[RELAY_URL_VAR, RELAY_APP_KEY_VAR]);

    if injected.is_required() && !injected.missing(&[RELAY_URL_VAR]).is_empty() {
        refuse_a_shipped_build_with_nowhere_to_report();
    }
}

/// Fails the build when one we ship has no relay endpoint.
///
/// The app key is deliberately not required; it has a default. The endpoint has none, and its
/// absence is invisible in every other respect: consent is still asked for and recorded, the
/// payload preview still renders the truth, and the app behaves identically. It simply has nowhere
/// to send, which is exactly the shape of a defect no test can see.
fn refuse_a_shipped_build_with_nowhere_to_report() -> ! {
    panic!(
        "{REQUIRE_VAR} is set, so this build must know where to report consented analytics, and \
         {RELAY_URL_VAR} is missing.\n\n\
         A build without it is legitimate and simply sends nothing (docs/analytics.md), which is \
         why this is opt-in. But a shipped build that lost it still asks the user for consent, \
         still records their yes, and then reports nowhere: so the one thing that consent bought \
         is the one thing missing, and nothing about the running app says so.\n\n\
         Set {RELAY_URL_VAR} in the environment or in the repo's .env (BUILDING.md), or unset \
         {REQUIRE_VAR} to build without."
    );
}
