use std::{
    process::Command,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
    time::{Duration, Instant},
};

use super::{Plan, run, slug};
use crate::{
    fixture::Repo,
    gate::{Outcome, Palette},
    gate_exec::{Cmd, commands},
};

const PLAIN: Palette = Palette {
    bold: "",
    red: "",
    green: "",
    yellow: "",
    reset: "",
};

fn outcome<'a>(steps: &'a [(String, Outcome)], label: &str) -> &'a Outcome {
    &steps.iter().find(|(l, _)| l == label).expect(label).1
}

#[test]
fn after_a_failure_the_rest_is_not_run_unless_asked_to_keep_going() {
    // A program that does not exist fails to start on every host. The second step waits behind the
    // first's lock, so it has not started when the first fails.
    let repo = Repo::new(&[]);
    let missing = || commands(vec![Cmd::new("mailcal-no-such-program")]);
    let mut stopping = Plan::default();
    stopping.add("first", &[], Some("dir"), missing());
    stopping.add("second", &[], Some("dir"), missing());
    let (steps, timings) = run(repo.root(), stopping, false, &PLAIN);
    assert!(matches!(outcome(&steps, "first"), Outcome::Fail));
    assert!(matches!(outcome(&steps, "second"), Outcome::Skip(_)));
    assert!(timings.contains_key("first"));

    let mut going = Plan::default();
    going.add("first", &[], Some("dir"), missing());
    going.add("second", &[], Some("dir"), missing());
    let (steps, _) = run(repo.root(), going, true, &PLAIN);
    assert!(matches!(outcome(&steps, "first"), Outcome::Fail));
    assert!(matches!(outcome(&steps, "second"), Outcome::Fail));
}

/// The helper below, by the name libtest filters on.
const HELPER: &str = "gate_sched::tests::helper_process";

/// Not a test: the process the next test runs as a step. Told to, it starts another of itself,
/// which writes a count to a file every 20 ms, so the test can tell whether a process under the
/// step's own is still alive.
#[test]
#[ignore = "a process the stopped-step test starts"]
fn helper_process() {
    let Some(beat) = std::env::var_os("MAILCAL_GATE_TEST_BEAT") else {
        return;
    };
    if std::env::var_os("MAILCAL_GATE_TEST_PARENT").is_some() {
        let _ = Command::new(std::env::current_exe().unwrap())
            .args(["--exact", HELPER, "--ignored"])
            .env_remove("MAILCAL_GATE_TEST_PARENT")
            .spawn();
        std::thread::sleep(Duration::from_secs(60));
        return;
    }
    for n in 0..3000 {
        let _ = std::fs::write(&beat, n.to_string());
        std::thread::sleep(Duration::from_millis(20));
    }
}

#[test]
fn a_step_the_gate_stops_says_why_and_takes_what_it_started_with_it() {
    let repo = Repo::new(&[]);
    let beat = repo.root().join("beat");
    let mut plan = Plan::default();
    let to_beat = beat.clone();
    plan.add("long", &[], None, move |ctx| {
        let mut helper = Command::new(std::env::current_exe().unwrap());
        helper
            .args(["--exact", HELPER, "--ignored"])
            .env("MAILCAL_GATE_TEST_BEAT", &to_beat)
            .env("MAILCAL_GATE_TEST_PARENT", "1");
        if ctx.run(&mut helper) {
            Outcome::Pass
        } else {
            Outcome::Fail
        }
    });
    let beating = beat.clone();
    plan.add("fails", &[], None, move |_| {
        let deadline = Instant::now() + Duration::from_secs(30);
        while !beating.exists() && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(20));
        }
        Outcome::Fail
    });
    let (steps, _) = run(repo.root(), plan, false, &PLAIN);
    assert!(matches!(outcome(&steps, "fails"), Outcome::Fail));
    match outcome(&steps, "long") {
        Outcome::Skip(why) => assert_eq!(why, "stopped: \"fails\" failed"),
        other => panic!("{other:?}"),
    }
    assert!(beat.exists(), "the process under the step never started");
    let count = || std::fs::read_to_string(&beat).unwrap_or_default();
    let before = count();
    std::thread::sleep(Duration::from_millis(300));
    assert_eq!(count(), before, "the process under the step still runs");
}

#[test]
fn a_job_runs_only_after_what_it_waits_for_has_passed() {
    let repo = Repo::new(&[]);
    let mut plan = Plan::default();
    let passes = plan.add("passes", &[], None, |_| Outcome::Pass);
    let fails = plan.add("fails", &[], None, |_| Outcome::Fail);
    plan.add("after the pass", &[passes], None, |_| Outcome::Pass);
    plan.add("after the failure", &[passes, fails], None, |_| {
        Outcome::Pass
    });
    let (steps, _) = run(repo.root(), plan, true, &PLAIN);
    assert!(matches!(outcome(&steps, "after the pass"), Outcome::Pass));
    match outcome(&steps, "after the failure") {
        Outcome::Skip(why) => assert!(why.contains("\"fails\""), "{why}"),
        other => panic!("{other:?}"),
    }
}

#[test]
fn two_jobs_holding_one_build_directory_never_run_together() {
    let repo = Repo::new(&[]);
    let inside = Arc::new(AtomicUsize::new(0));
    let most = Arc::new(AtomicUsize::new(0));
    let mut plan = Plan::default();
    for n in 0..4 {
        let (inside, most) = (Arc::clone(&inside), Arc::clone(&most));
        plan.add(format!("job {n}"), &[], Some("target/debug"), move |_| {
            let now = inside.fetch_add(1, Ordering::SeqCst) + 1;
            most.fetch_max(now, Ordering::SeqCst);
            std::thread::sleep(Duration::from_millis(30));
            inside.fetch_sub(1, Ordering::SeqCst);
            Outcome::Pass
        });
    }
    let (steps, _) = run(repo.root(), plan, false, &PLAIN);
    assert!(steps.iter().all(|(_, o)| matches!(o, Outcome::Pass)));
    assert_eq!(most.load(Ordering::SeqCst), 1);
}

#[test]
fn jobs_without_a_shared_directory_overlap() {
    let repo = Repo::new(&[]);
    let inside = Arc::new(AtomicUsize::new(0));
    let most = Arc::new(AtomicUsize::new(0));
    let mut plan = Plan::default();
    for n in 0..3 {
        let (inside, most) = (Arc::clone(&inside), Arc::clone(&most));
        plan.add(format!("job {n}"), &[], None, move |_| {
            let now = inside.fetch_add(1, Ordering::SeqCst) + 1;
            most.fetch_max(now, Ordering::SeqCst);
            std::thread::sleep(Duration::from_millis(200));
            inside.fetch_sub(1, Ordering::SeqCst);
            Outcome::Pass
        });
    }
    run(repo.root(), plan, false, &PLAIN);
    assert!(most.load(Ordering::SeqCst) > 1);
}

#[test]
fn what_a_command_prints_lands_in_its_jobs_log() {
    let repo = Repo::new(&[]);
    let mut plan = Plan::default();
    plan.add(
        "says hello",
        &[],
        None,
        commands(vec![Cmd::new("cargo").args(["--version"])]),
    );
    let (steps, _) = run(repo.root(), plan, false, &PLAIN);
    assert!(matches!(outcome(&steps, "says hello"), Outcome::Pass));
    let log = std::fs::read_to_string(repo.root().join("target/gate/00-says-hello.log")).unwrap();
    assert!(log.contains("$ cargo --version"), "{log}");
    assert!(log.contains("cargo "), "{log}");
}

#[test]
fn a_label_becomes_a_file_name() {
    assert_eq!(slug("tests (run)"), "tests-run");
    assert_eq!(
        slug("windows (regenerate C# bindings + l10n)"),
        "windows-regenerate-c-bindings-l10n"
    );
}
