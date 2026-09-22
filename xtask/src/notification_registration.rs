//! Guards the order of two calls in the Windows client's `Main`, which decides whether a click on
//! a new-mail notification opens the app or kills it.
//!
//! A launch that *is* a click has to build an `AppNotificationActivatedEventArgs` inside
//! `AppInstance.GetCurrent().GetActivatedEventArgs()`, and the runtime cannot do that unless
//! `AppNotificationManager.Register()` has already run in this process. Unregistered, it does not
//! throw: it **fails fast**, `0xc0000409` inside `Microsoft.WindowsAppRuntime.dll`, and it does so
//! before `Log.Init` has given anything somewhere to write.
//!
//! So the failure is silent in every channel a developer would look in. The app vanishes on the one
//! launch the feature exists for, the log holds nothing at all, and every other launch is perfectly
//! healthy, which is what makes it read as a broken notification rather than as a crash. Only
//! Windows Error Reporting knows, and only if someone thinks to look.
//!
//! No gate can click a notification, and the launch itself reaches only a Windows host
//! (`clients/windows/uitests/NotificationLaunch.Tests.ps1`). This is the half that runs everywhere:
//! the ordering is decidable from the source, and the source is one file.

use std::path::Path;

/// The file whose statement order decides this.
const PROGRAM: &str = "clients/windows/Mailcal/Program.cs";

/// Registering the notification platform. Must come first.
const REGISTER: &str = "NewMailNotifier.Arm()";

/// Reading the activation, which needs the platform already up.
const ACTIVATION: &str = "GetActivatedEventArgs(";

/// What the source says about the two calls.
#[derive(Debug, PartialEq, Eq)]
enum Ordering {
    /// `Arm()` comes first, which is the only arrangement that survives a click.
    Correct,
    /// Both are present and the wrong way round.
    Inverted,
    /// The registration is not in this file at all.
    MissingRegistration,
    /// The activation read is not in this file at all.
    MissingActivation,
}

/// Runs the check. `Ok(true)` means the registration precedes the activation read.
///
/// # Errors
///
/// Propagates an unreadable `Program.cs`: a check that reports OK because it could not look is
/// worse than no check.
pub(crate) fn run(root: &Path) -> Result<bool, String> {
    let path = root.join(PROGRAM);
    let source = std::fs::read_to_string(&path)
        .map_err(|err| format!("cannot read {}: {err}", path.display()))?;

    match ordering(&source) {
        Ordering::Correct => {
            println!("OK: {PROGRAM} registers for notifications before it reads the activation.");
            Ok(true)
        }
        Ordering::Inverted => {
            println!(
                "ERROR: {PROGRAM} reads the activation before it registers for notifications."
            );
            explain();
            Ok(false)
        }
        Ordering::MissingRegistration => {
            println!("ERROR: {PROGRAM} does not call {REGISTER}.");
            explain();
            Ok(false)
        }
        Ordering::MissingActivation => {
            println!("ERROR: {PROGRAM} does not call {ACTIVATION}).");
            println!(
                "    The check cannot see the ordering it exists for. If the activation moved, \
                 move this check with it."
            );
            Ok(false)
        }
    }
}

/// What to do about it, in the terms the failure will actually present itself in.
fn explain() {
    eprintln!(
        "
Register first, in Main, above the activation read:

    NewMailNotifier.Arm();
    var activation = AppInstance.GetCurrent().GetActivatedEventArgs();

What a click DOES is installed later, from the window (NewMailNotifier.Route). Out of order, a
launch that is itself a notification click FAILS FAST inside the Windows App SDK, before the log
has a path: an empty log, a WER entry, and every other launch healthy (docs/client-traps.md)."
    );
}

/// Where the two calls stand relative to each other, ignoring comments.
///
/// Comments are dropped because both names are discussed in prose in this very file, and a rule
/// that a comment can break is a rule people delete.
fn ordering(source: &str) -> Ordering {
    let position = |needle: &str| {
        source
            .lines()
            .map(str::trim_start)
            .filter(|line| !line.starts_with("//"))
            .position(|line| line.contains(needle))
    };
    match (position(REGISTER), position(ACTIVATION)) {
        (Some(register), Some(activation)) if register < activation => Ordering::Correct,
        (Some(_), Some(_)) => Ordering::Inverted,
        (None, Some(_)) => Ordering::MissingRegistration,
        _ => Ordering::MissingActivation,
    }
}

#[cfg(test)]
mod tests {
    use super::{Ordering, ordering};

    #[test]
    fn registering_first_is_correct() {
        let source = "\
            NewMailNotifier.Arm();\n\
            var a = AppInstance.GetCurrent().GetActivatedEventArgs();\n";
        assert_eq!(ordering(source), Ordering::Correct);
    }

    #[test]
    fn registering_afterwards_is_the_bug_this_exists_for() {
        let source = "\
            var a = AppInstance.GetCurrent().GetActivatedEventArgs();\n\
            NewMailNotifier.Arm();\n";
        assert_eq!(ordering(source), Ordering::Inverted);
    }

    #[test]
    fn a_registration_only_mentioned_in_a_comment_does_not_count() {
        let source = "\
            // NewMailNotifier.Arm() used to live here.\n\
            var a = AppInstance.GetCurrent().GetActivatedEventArgs();\n\
            NewMailNotifier.Arm();\n";
        assert_eq!(ordering(source), Ordering::Inverted);
    }

    #[test]
    fn an_activation_read_only_in_a_comment_does_not_count() {
        let source = "\
            NewMailNotifier.Arm();\n\
            // GetActivatedEventArgs() is read below.\n";
        assert_eq!(ordering(source), Ordering::MissingActivation);
    }

    #[test]
    fn a_missing_registration_is_reported_rather_than_passed() {
        let source = "var a = AppInstance.GetCurrent().GetActivatedEventArgs();\n";
        assert_eq!(ordering(source), Ordering::MissingRegistration);
    }
}
