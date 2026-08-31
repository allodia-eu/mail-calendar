//! The app's light/dark appearance on the GTK host.
//!
//! The choice is a core setting (`docs/settings.md` → General), persisted beside every other
//! display preference so the clients cannot each invent their own default. This module is only the
//! half libadwaita owns: turning it into a `ColorScheme`, driving the Settings picker's order, and
//! reading the debug-only `MAILCAL_APPEARANCE` override a screenshot or UI run pins a launch with.

use mailcal_bindings::Appearance;

use crate::l10n;

/// The order the Settings picker offers the three choices in, and therefore what each index means
/// in **both** directions.
///
/// One list rather than a `match` per direction: two independently written mappings can disagree,
/// and the disagreement is silent in the way that costs most; the page opens on one choice and
/// stores its neighbour.
pub(crate) const PICKER_ORDER: [Appearance; 3] =
    [Appearance::System, Appearance::Light, Appearance::Dark];

/// Paints the app in `appearance`.
pub(crate) fn apply(appearance: Appearance) {
    adw::StyleManager::default().set_color_scheme(color_scheme(appearance));
}

/// The libadwaita scheme `appearance` paints in.
///
/// `System` maps to `Default` rather than to a `Prefer`/`Force` scheme: it hands libadwaita back to
/// the desktop's own setting, which it then keeps following *while the app runs*. Resolving the
/// desktop's current scheme here instead would pin the app to whatever it was at launch.
pub(crate) const fn color_scheme(appearance: Appearance) -> adw::ColorScheme {
    match appearance {
        Appearance::Light => adw::ColorScheme::ForceLight,
        Appearance::Dark => adw::ColorScheme::ForceDark,
        Appearance::System => adw::ColorScheme::Default,
    }
}

/// The catalog label for `appearance`, so the picker's labels are built from the same order its
/// indices are read back from.
pub(crate) fn label(appearance: Appearance) -> &'static str {
    match appearance {
        Appearance::System => l10n::settings_appearance_system(),
        Appearance::Light => l10n::settings_appearance_light(),
        Appearance::Dark => l10n::settings_appearance_dark(),
    }
}

/// Which entry the picker opens on for the `stored` choice.
pub(crate) fn selection(stored: Appearance) -> u32 {
    PICKER_ORDER
        .iter()
        .position(|candidate| *candidate == stored)
        .and_then(|index| u32::try_from(index).ok())
        .unwrap_or(0)
}

/// What picking entry `index` chooses. An index the list does not hold reads as `System`: the
/// default, and the one choice that cannot be wrong for a user who has never picked.
pub(crate) fn chosen(index: u32) -> Appearance {
    usize::try_from(index)
        .ok()
        .and_then(|index| PICKER_ORDER.get(index).copied())
        .unwrap_or(Appearance::System)
}

/// The appearance this launch comes up in: the `MAILCAL_APPEARANCE` override when it names one,
/// else `stored`. A later pick in Settings wins for the rest of the session: the override decides
/// how a run *starts*, not what the app is allowed to do.
pub(crate) fn at_launch(stored: Appearance) -> Appearance {
    launch_override().unwrap_or(stored)
}

/// The `MAILCAL_APPEARANCE` override, or `None` when it is unset or unrecognised.
#[cfg(debug_assertions)]
fn launch_override() -> Option<Appearance> {
    parse(std::env::var("MAILCAL_APPEARANCE").ok().as_deref())
}

/// A shipped binary must not have its theme flipped by a stray environment variable; the same
/// property the dev-account and showcase switches hold.
#[cfg(not(debug_assertions))]
const fn launch_override() -> Option<Appearance> {
    None
}

/// Reads an override value, or `None` when it is unset or names nothing we know: a typo'd override
/// must not silently read as "follow the system", which looks exactly like a working one.
#[cfg(debug_assertions)]
fn parse(raw: Option<&str>) -> Option<Appearance> {
    match raw
        .map(|value| value.trim().to_ascii_lowercase())
        .as_deref()
    {
        Some("system") => Some(Appearance::System),
        Some("light") => Some(Appearance::Light),
        Some("dark") => Some(Appearance::Dark),
        _ => None,
    }
}

/// The picker's rules, which ship in every build; unlike the launch override below them, whose
/// tests are gated the same way `parse` itself is.
#[cfg(test)]
mod picker_tests {
    use mailcal_bindings::Appearance;

    use super::{PICKER_ORDER, chosen, color_scheme, label, selection};

    #[test]
    fn the_picker_stores_the_choice_it_is_showing() {
        // The failure this rules out is silent and total: with the two directions mapped
        // separately, the page opens on one entry and writes its neighbour, so a user picking
        // Dark gets Light and the setting looks broken rather than mis-wired.
        for stored in PICKER_ORDER {
            assert_eq!(chosen(selection(stored)), stored);
        }
    }

    #[test]
    fn the_order_is_the_one_every_client_offers() {
        // Pinned literally rather than left to the round trip above, which a reversed list would
        // satisfy just as well. `docs/settings.md` names this order, and a screenshot set is
        // captured against it.
        assert_eq!(selection(Appearance::System), 0);
        assert_eq!(selection(Appearance::Light), 1);
        assert_eq!(selection(Appearance::Dark), 2);
        assert_eq!(
            label(Appearance::System),
            crate::l10n::settings_appearance_system()
        );
        assert_eq!(
            label(Appearance::Dark),
            crate::l10n::settings_appearance_dark()
        );
    }

    #[test]
    fn an_index_the_list_does_not_hold_follows_the_host() {
        // A `DropDown` cannot report an index it was not built with, so this is about what the app
        // does if one ever arrives: follow the host, which is the default and the one choice a user
        // who never picked cannot object to.
        assert_eq!(chosen(3), Appearance::System);
        assert_eq!(chosen(u32::MAX), Appearance::System);
    }

    #[test]
    fn following_the_host_stays_the_hosts_decision() {
        assert_eq!(
            color_scheme(Appearance::Light),
            adw::ColorScheme::ForceLight
        );
        assert_eq!(color_scheme(Appearance::Dark), adw::ColorScheme::ForceDark);
        // `Default`, not a `Prefer`/`Force` scheme resolved from the desktop at launch: System has
        // to keep following the desktop while the app runs, and every Force/Prefer value pins it.
        assert_eq!(color_scheme(Appearance::System), adw::ColorScheme::Default);
    }
}

#[cfg(all(test, debug_assertions))]
mod tests {
    use mailcal_bindings::Appearance;

    use super::parse;

    #[test]
    fn the_contract_spellings_are_matched_literally() {
        assert_eq!(parse(Some("dark")), Some(Appearance::Dark));
        assert_eq!(parse(Some(" LIGHT ")), Some(Appearance::Light));
        // An override in its own right: it pins a run to the desktop's setting even for a developer
        // whose stored choice is Light or Dark.
        assert_eq!(parse(Some("system")), Some(Appearance::System));
    }

    #[test]
    fn anything_else_leaves_the_stored_choice_standing() {
        assert_eq!(parse(None), None);
        assert_eq!(parse(Some("")), None);
        assert_eq!(parse(Some("night")), None);
        assert_eq!(parse(Some("1")), None);
    }
}
