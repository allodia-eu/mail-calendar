use std::{
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
    time::{Duration, Instant},
};

use mailcal_account::{Appearance, TimeFormat, WeekStart, load_preferences, save_preferences};

use super::{PreferencesFile, ScratchPreferences};

fn counted() -> (Arc<AtomicUsize>, impl Fn() + Send + Sync + 'static) {
    let told = Arc::new(AtomicUsize::new(0));
    let counter = Arc::clone(&told);
    (told, move || {
        counter.fetch_add(1, Ordering::SeqCst);
    })
}

/// A change is read back at once, before the writer has saved it, and is in the file after.
#[test]
fn a_change_is_read_at_once_and_saved_behind() {
    let scratch = ScratchPreferences::empty();
    let path = scratch.path();
    let file = PreferencesFile::open(path.clone(), || {});
    file.edit(|prefs| prefs.appearance = Appearance::Dark);
    assert_eq!(file.load().appearance, Appearance::Dark);
    file.settle();
    assert_eq!(load_preferences(&path).appearance, Appearance::Dark);
    assert!(!file.failed());
}

/// ⚠️ Changes made together from several threads are all kept: each is applied to what the file
/// holds as it is written, so none writes back a copy read before another landed.
#[test]
fn changes_made_together_are_all_kept() {
    let scratch = ScratchPreferences::empty();
    let path = scratch.path();
    let file = PreferencesFile::open(path.clone(), || {});
    let threads: Vec<_> = (0..8)
        .map(|n| {
            let file = file.clone();
            std::thread::spawn(move || {
                let account = format!("account-{n}");
                file.edit(move |prefs| {
                    prefs
                        .notify_marks
                        .insert(account.clone(), "2026-01-01T00:00:00Z".to_owned());
                });
            })
        })
        .collect();
    file.edit(|prefs| prefs.week_start = WeekStart::Sunday);
    for thread in threads {
        thread.join().unwrap();
    }
    file.settle();
    let on_disk = load_preferences(&path);
    assert_eq!(on_disk.notify_marks.len(), 8);
    assert_eq!(on_disk.week_start, WeekStart::Sunday);
}

/// A field another process wrote while this one was running survives this one's next save.
#[test]
fn a_field_written_elsewhere_survives() {
    let scratch = ScratchPreferences::empty();
    let path = scratch.path();
    let file = PreferencesFile::open(path.clone(), || {});
    file.edit(|prefs| prefs.appearance = Appearance::Light);
    file.settle();

    let mut theirs = load_preferences(&path);
    theirs.time_format = TimeFormat::TwelveHour;
    save_preferences(&path, &theirs).unwrap();

    file.edit(|prefs| prefs.week_start = WeekStart::Sunday);
    file.settle();
    let on_disk = load_preferences(&path);
    assert_eq!(on_disk.time_format, TimeFormat::TwelveHour);
    assert_eq!(on_disk.appearance, Appearance::Light);
    assert_eq!(on_disk.week_start, WeekStart::Sunday);
}

/// ⚠️ A save that fails is said once and keeps its change: every read still has it, and a retry
/// once the cause has gone writes it and says so.
#[test]
fn a_failed_save_keeps_its_change_until_a_retry_writes_it() {
    let dir = tempfile::tempdir().unwrap();
    // A file where the directory should be: the save cannot create it.
    let blocked = dir.path().join("data");
    std::fs::write(&blocked, b"not a directory").unwrap();
    let path = blocked.join("preferences.toml");
    let (told, on_failure) = counted();
    let file = PreferencesFile::open(path.clone(), on_failure);

    file.edit(|prefs| prefs.appearance = Appearance::Dark);
    file.settle();
    assert!(file.failed());
    assert_eq!(told.load(Ordering::SeqCst), 1);
    assert_eq!(file.load().appearance, Appearance::Dark);

    // Failing again is not news.
    file.edit(|prefs| prefs.week_start = WeekStart::Sunday);
    file.settle();
    assert_eq!(told.load(Ordering::SeqCst), 1);

    // Nothing is tried again on its own: the writer does not spin on a disk that refuses.
    std::fs::remove_file(&blocked).unwrap();
    file.settle();
    assert!(file.failed());

    file.retry();
    assert!(!file.failed());
    assert_eq!(told.load(Ordering::SeqCst), 2);
    let on_disk = load_preferences(&path);
    assert_eq!(on_disk.appearance, Appearance::Dark);
    assert_eq!(on_disk.week_start, WeekStart::Sunday);
}

/// The last handle going writes what is still waiting before the writer stops.
#[test]
fn what_waits_is_written_when_the_last_handle_goes() {
    let scratch = ScratchPreferences::empty();
    let path = scratch.path();
    let file = PreferencesFile::open(path.clone(), || {});
    file.edit(|prefs| prefs.appearance = Appearance::Light);
    drop(file);
    for _ in 0..200 {
        if load_preferences(&path).appearance == Appearance::Light {
            return;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    panic!("the waiting change was never written");
}

/// A process about to end waits for what is waiting to be written, and no longer than it said.
#[test]
fn a_bounded_wait_returns_once_written_or_at_its_limit() {
    let scratch = ScratchPreferences::empty();
    let path = scratch.path();
    let file = PreferencesFile::open(path.clone(), || {});
    file.edit(|prefs| prefs.appearance = Appearance::Light);
    assert!(file.settle_within(Duration::from_secs(5)));
    assert_eq!(load_preferences(&path).appearance, Appearance::Light);

    // A disk that does not answer: the writer cannot take the file while this holds it.
    let held = file.hold_disk();
    file.edit(|prefs| prefs.appearance = Appearance::Dark);
    let started = Instant::now();
    assert!(!file.settle_within(Duration::from_millis(100)));
    assert!(started.elapsed() < Duration::from_secs(2));
    drop(held);
    assert!(file.settle_within(Duration::from_secs(5)));
    assert_eq!(load_preferences(&path).appearance, Appearance::Dark);
}

/// Without a file nothing is kept, and every read is the defaults.
#[test]
fn without_a_file_nothing_is_kept() {
    let file = PreferencesFile::default();
    assert!(!file.is_stored());
    file.edit(|prefs| prefs.appearance = Appearance::Dark);
    file.settle();
    file.retry();
    assert_eq!(file.load().appearance, Appearance::System);
    assert!(!file.failed());
}
