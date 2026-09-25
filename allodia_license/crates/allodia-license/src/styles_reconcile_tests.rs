// SPDX-FileCopyrightText: 2026 Allodia
// SPDX-License-Identifier: LicenseRef-Allodia-1.0

//! The account list's rules, held for writing styles: the four cases that look alike from outside
//! (the server moved, this device moved, both did, neither did), and the one place styles differ,
//! which is what counts as the same style when this device has never synced it.

use super::*;
use crate::fingerprint;

fn style(name: &str, notes: &str) -> SyncedStyle {
    let mut guide = StyleGuide::new();
    guide.notes = notes.to_owned();
    SyncedStyle {
        name: name.to_owned(),
        guide,
    }
}

fn record(id: &str, version: u64, style: SyncedStyle) -> StyleRecord {
    StyleRecord {
        id: id.to_owned(),
        version,
        style,
        updated_at: "2026-09-23T10:00:00.000Z".to_owned(),
    }
}

fn list(styles: Vec<StyleRecord>, deleted: Vec<Tombstone>) -> StyleList {
    StyleList {
        styles,
        deleted,
        synced_at: "2026-09-23T10:05:00.000Z".to_owned(),
    }
}

fn gone(id: &str, version: u64) -> Tombstone {
    Tombstone {
        id: id.to_owned(),
        version,
        deleted_at: "2026-09-22T08:00:00.000Z".to_owned(),
    }
}

/// A local style in step with the service as it was at `version`.
fn synced(style_id: &str, base: &SyncedStyle, id: &str, version: u64) -> LocalStyle {
    LocalStyle {
        style_id: style_id.to_owned(),
        style: base.clone(),
        sync: Some(SyncState {
            id: id.to_owned(),
            version,
            fingerprint: fingerprint(base),
            detached: false,
        }),
    }
}

fn new_here(style_id: &str, style: SyncedStyle) -> LocalStyle {
    LocalStyle {
        style_id: style_id.to_owned(),
        style,
        sync: None,
    }
}

#[test]
fn nothing_changed_on_either_side_is_nothing_to_do() {
    let base = style("Work", "Short.");
    let local = vec![synced("s1", &base, "rec-1", 4)];
    assert!(reconcile_styles(&local, &list(vec![record("rec-1", 4, base)], vec![])).is_empty());
}

#[test]
fn the_server_moved_and_this_device_did_not_is_an_update() {
    let base = style("Work", "Short.");
    let local = vec![synced("s1", &base, "rec-1", 4)];
    let remote = list(vec![record("rec-1", 5, style("Office", "Short."))], vec![]);
    match &reconcile_styles(&local, &remote)[..] {
        [Verdict::UpdateAvailable { local_id, current }] => {
            assert_eq!(local_id, "s1");
            assert_eq!(current.style.name, "Office");
        }
        other => panic!("expected an update, got {other:?}"),
    }
}

#[test]
fn this_device_moved_and_the_server_did_not_is_a_push_at_the_version_it_read() {
    let base = style("Work", "Short.");
    let mut local = vec![synced("s1", &base, "rec-1", 4)];
    local[0].style.guide.notes = "Short, and never before nine.".to_owned();
    match &reconcile_styles(&local, &list(vec![record("rec-1", 4, base)], vec![]))[..] {
        [
            Verdict::Push {
                local_id,
                id,
                version,
            },
        ] => assert_eq!(
            (local_id.as_str(), id.as_str(), *version),
            ("s1", "rec-1", 4)
        ),
        other => panic!("expected a push, got {other:?}"),
    }
}

/// The case the three-way base exists for: applying the service's version would throw away the
/// edit made here and call it an update.
#[test]
fn both_sides_moved_is_a_conflict_and_never_an_update() {
    let base = style("Work", "Short.");
    let mut local = vec![synced("s1", &base, "rec-1", 4)];
    local[0].style.name = "Work (laptop)".to_owned();
    let remote = list(
        vec![record("rec-1", 5, style("Work (phone)", "Short."))],
        vec![],
    );
    match &reconcile_styles(&local, &remote)[..] {
        [Verdict::Conflict { local_id, current }] => {
            assert_eq!(local_id, "s1");
            assert_eq!(current.version, 5);
        }
        other => panic!("expected a conflict, got {other:?}"),
    }
}

#[test]
fn a_style_the_service_has_never_seen_is_uploaded() {
    let local = vec![new_here("s1", style("Work", "Short."))];
    assert_eq!(
        reconcile_styles(&local, &list(vec![], vec![])),
        vec![Verdict::Upload {
            local_id: "s1".to_owned()
        }]
    );
}

/// Only an identical style is adopted, which is what a sign-in again after a sign-out finds: the
/// bookkeeping was dropped, the styles on both sides were not.
#[test]
fn an_identical_style_the_service_already_holds_is_adopted_rather_than_uploaded_twice() {
    let same = style("Work", "Short.");
    let local = vec![new_here("s1", same.clone())];
    match &reconcile_styles(&local, &list(vec![record("rec-1", 3, same)], vec![]))[..] {
        [Verdict::Adopt { local_id, current }] => {
            assert_eq!(local_id, "s1");
            assert_eq!(current.id, "rec-1");
        }
        other => panic!("expected an adoption, got {other:?}"),
    }
}

/// Where styles part from accounts. Two devices that each learned a "Work" style hold two styles;
/// adopting one as the other would overwrite it with the next push.
#[test]
fn a_different_style_under_the_same_name_is_uploaded_and_the_other_offered() {
    let local = vec![new_here("s1", style("Work", "Learned on the laptop."))];
    let remote = list(
        vec![record("rec-1", 3, style("Work", "Learned on the phone."))],
        vec![],
    );
    let verdicts = reconcile_styles(&local, &remote);
    assert_eq!(verdicts.len(), 2, "{verdicts:?}");
    assert!(verdicts.contains(&Verdict::Upload {
        local_id: "s1".to_owned()
    }));
    assert!(
        verdicts
            .iter()
            .any(|verdict| matches!(verdict, Verdict::Offer { current } if current.id == "rec-1"))
    );
}

#[test]
fn a_style_from_another_device_is_offered() {
    let remote = list(vec![record("rec-9", 1, style("Personal", ""))], vec![]);
    match &reconcile_styles(&[], &remote)[..] {
        [Verdict::Offer { current }] => assert_eq!(current.id, "rec-9"),
        other => panic!("expected an arrival, got {other:?}"),
    }
}

#[test]
fn a_style_forgotten_elsewhere_is_removed_here() {
    let base = style("Work", "Short.");
    let local = vec![synced("s1", &base, "rec-1", 4)];
    assert_eq!(
        reconcile_styles(&local, &list(vec![], vec![gone("rec-1", 5)])),
        vec![Verdict::RemovedElsewhere {
            local_id: "s1".to_owned()
        }]
    );
}

#[test]
fn a_style_forgotten_elsewhere_is_not_offered_back_in_the_same_pass() {
    let remote = list(
        vec![record("rec-9", 1, style("Personal", ""))],
        vec![gone("rec-9", 2)],
    );
    assert!(reconcile_styles(&[], &remote).is_empty());
}

/// A `since` delta carries only what changed, so an unchanged style is simply absent.
#[test]
fn a_style_missing_from_a_delta_is_not_treated_as_forgotten() {
    let base = style("Work", "Short.");
    let local = vec![synced("s1", &base, "rec-1", 4)];
    assert!(reconcile_styles(&local, &list(vec![], vec![])).is_empty());
}
