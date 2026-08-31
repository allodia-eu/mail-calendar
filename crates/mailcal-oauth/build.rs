//! Supplies the OAuth client registrations `src/credentials.rs` reads with `option_env!`.
//!
//! The mechanism; environment first then the repo's gitignored `.env`, blank counting as unset,
//! and the cargo directives that stop a stale value surviving a rebuild, is
//! [`mailcal_buildenv`], shared with `mailcal-bindings`. What stays here is the policy: which
//! registrations *this* target compiles in, and what to say when a build we ship lost one.
//!
//! Reading the file at all is what covers every front door at once. Each client builds the core
//! through a different one (Gradle, `xcodebuild`, MSBuild, the GNOME SDK, plain `cargo`) so
//! asking each of them to export five variables would leave whichever one you forgot producing an
//! app that silently drops Google and Microsoft sign-in.

use std::fmt::Write as _;

use mailcal_buildenv::{Injected, REQUIRE_VAR};

/// Every variable `src/credentials.rs` can read. `option_env!` names them a second time, and the
/// two lists must agree: a name here and nowhere else only costs a rebuild, a name there and not
/// here costs a build that ignores the value it was given.
const CREDENTIAL_VARS: &[&str] = &[
    "MAILCAL_GOOGLE_DESKTOP_CLIENT_ID",
    "MAILCAL_GOOGLE_DESKTOP_CLIENT_SECRET",
    "MAILCAL_GOOGLE_IOS_CLIENT_ID",
    "MAILCAL_GOOGLE_ANDROID_CLIENT_ID",
    "MAILCAL_MS_CLIENT_ID",
    "MAILCAL_ALLODIA_CLIENT_ID",
    "MAILCAL_ALLODIA_HOST",
];

fn main() {
    let injected = Injected::load();
    injected.export(CREDENTIAL_VARS);

    if injected.is_required() {
        require_the_registrations_this_target_uses(&injected);
    }
}

/// Fails the build naming every registration this target needs and did not get.
///
/// Only the variables this target actually compiles in are required, so the message can never be
/// spurious: a macOS release does not need Android's client id and is not asked for it.
fn require_the_registrations_this_target_uses(injected: &Injected) {
    let target_os = std::env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    // Mirrors the `cfg` selection in `src/credentials.rs`; the two must agree.
    let mut required = match target_os.as_str() {
        "ios" => vec!["MAILCAL_GOOGLE_IOS_CLIENT_ID"],
        "android" => vec!["MAILCAL_GOOGLE_ANDROID_CLIENT_ID"],
        _ => vec![
            "MAILCAL_GOOGLE_DESKTOP_CLIENT_ID",
            "MAILCAL_GOOGLE_DESKTOP_CLIENT_SECRET",
        ],
    };
    // Neither of these varies by target: `credentials.rs` reads both with a bare `option_env!`.
    required.push("MAILCAL_MS_CLIENT_ID");
    required.push("MAILCAL_ALLODIA_CLIENT_ID");

    let missing = injected.missing(&required);
    if missing.is_empty() {
        return;
    }

    let mut message = format!(
        "{REQUIRE_VAR} is set, so this build must carry every OAuth client registration it \
         uses, and {} missing for target_os \"{target_os}\":\n",
        if missing.len() == 1 {
            "one is"
        } else {
            "these are"
        },
    );
    for var in missing {
        let _ = writeln!(message, "    {var}");
    }
    message.push_str(
        "\nA build without them is legitimate and simply does not offer the sign-in routes they \
         carry, which is why this is opt-in, but a shipped build that lost them looks correct \
         everywhere except in front of a user. Set them in the environment or in the repo's .env \
         (BUILDING.md), or unset ",
    );
    message.push_str(REQUIRE_VAR);
    message.push_str(" to build without.");
    panic!("{message}");
}
