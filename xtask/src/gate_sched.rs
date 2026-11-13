//! Runs the gate's jobs side by side, each as soon as what it waits for has passed.
//!
//! A job names the jobs it needs and, when it runs cargo, the build directory it holds. Two jobs
//! holding one directory never run together: cargo would make the second wait on the directory's
//! lock anyway, and a job waiting there is a job that looks busy and is not. Everything else runs
//! at once, so the formatting check, the Python suites, clippy, the test build and Gradle overlap
//! instead of queueing behind each other.
//!
//! Each job writes everything it prints to its own log under `target/gate/`. The console gets one
//! line when a job starts and one when it ends, and the whole log of a job that failed; a log that
//! interleaved eight jobs would be one nobody could read.

use std::{
    collections::HashMap,
    fs::File,
    io::Write as _,
    path::{Path, PathBuf},
    process::{Child, Command, ExitStatus, Stdio},
    sync::{
        Arc, Mutex, OnceLock,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    time::{Duration, Instant},
};

use crate::{
    gate::{Outcome, Palette, Step},
    gate_exec::{Timings, as_typed},
    gate_kill::kill_tree,
    gate_testrun::Build,
};

/// A job's place in the plan; a job may only wait for jobs added before it.
pub(crate) type JobId = usize;

/// What a job does, given its context.
pub(crate) type Work = Box<dyn FnOnce(&Ctx) -> Outcome + Send>;

/// One step of the gate.
pub(crate) struct Job {
    label: String,
    after: Vec<JobId>,
    lock: Option<&'static str>,
    work: Work,
}

impl std::fmt::Debug for Job {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Job")
            .field("label", &self.label)
            .field("after", &self.after)
            .field("lock", &self.lock)
            .finish_non_exhaustive()
    }
}

/// The gate's jobs, in the order the summary lists them.
#[derive(Debug, Default)]
pub(crate) struct Plan {
    jobs: Vec<Job>,
}

impl Plan {
    /// Adds a job that starts once every job in `after` has passed and `lock` is free.
    pub(crate) fn add(
        &mut self,
        label: impl Into<String>,
        after: &[JobId],
        lock: Option<&'static str>,
        work: impl FnOnce(&Ctx) -> Outcome + Send + 'static,
    ) -> JobId {
        let id = self.jobs.len();
        assert!(
            after.iter().all(|&a| a < id),
            "a job waits only for earlier jobs"
        );
        self.jobs.push(Job {
            label: label.into(),
            after: after.to_vec(),
            lock,
            work: Box::new(work),
        });
        id
    }

    /// Adds a step whose verdict is known without running anything: a skip or a missing tool.
    pub(crate) fn verdict(&mut self, label: impl Into<String>, outcome: Outcome) -> JobId {
        self.add(label, &[], None, move |_| outcome)
    }

    /// Each job's label, what it waits for and the directory it holds, for `--list`.
    pub(crate) fn describe(&self) -> Vec<(String, Vec<String>, Option<&'static str>)> {
        self.jobs
            .iter()
            .map(|job| {
                let after = job
                    .after
                    .iter()
                    .map(|&a| self.jobs[a].label.clone())
                    .collect();
                (job.label.clone(), after, job.lock)
            })
            .collect()
    }
}

/// What a running job can reach: the checkout, its log, the builds earlier jobs left, and whether
/// the gate has given up.
#[derive(Debug)]
pub(crate) struct Ctx {
    /// The repository root.
    pub(crate) root: PathBuf,
    /// This job's log.
    log: PathBuf,
    /// The step whose failure stopped the gate, once one has.
    halted_by: Arc<OnceLock<String>>,
    /// Whether the gate ended one of this job's processes, so its failure is not its own.
    stopped: AtomicBool,
    colour: bool,
    builds: Arc<Mutex<HashMap<&'static str, Build>>>,
}

impl Ctx {
    /// Appends a line to this job's log.
    pub(crate) fn note(&self, text: &str) {
        if let Ok(mut file) = File::options().append(true).create(true).open(&self.log) {
            let _ = writeln!(file, "{text}");
        }
    }

    /// Where this job's log is.
    pub(crate) fn log_path(&self) -> &Path {
        &self.log
    }

    /// Whether to colour what a tool prints, which is when the gate's own output is a terminal.
    pub(crate) fn colour(&self) -> bool {
        self.colour
    }

    /// Runs `command` to the end, everything it prints going to this job's log.
    pub(crate) fn run(&self, command: &mut Command) -> bool {
        let Ok(log) = File::options().append(true).create(true).open(&self.log) else {
            return false;
        };
        let Ok(err) = log.try_clone() else {
            return false;
        };
        self.note(&format!("$ {}", shown(command)));
        let spawned = as_typed(command)
            .stdin(Stdio::null())
            .stdout(log)
            .stderr(err)
            .spawn();
        self.finish(spawned)
    }

    /// Runs `command` with what it prints on stdout kept apart in `out`, and everything else in
    /// this job's log: cargo's `--message-format json` lines, read once it has finished.
    pub(crate) fn run_split(&self, command: &mut Command, out: &Path) -> bool {
        let (Ok(log), Ok(stdout)) = (
            File::options().append(true).create(true).open(&self.log),
            File::create(out),
        ) else {
            return false;
        };
        self.note(&format!("$ {}", shown(command)));
        let spawned = as_typed(command)
            .stdin(Stdio::null())
            .stdout(stdout)
            .stderr(log)
            .spawn();
        self.finish(spawned)
    }

    /// Whether the gate has given up, so a job starts nothing more.
    pub(crate) fn stopping(&self) -> bool {
        self.halted_by.get().is_some()
    }

    /// Whether `child` has exited, ending it and everything under it first when the gate has
    /// given up.
    pub(crate) fn poll(&self, child: &mut Child) -> Option<Result<ExitStatus, ()>> {
        if self.stopping() {
            kill_tree(child);
            self.stopped.store(true, Ordering::Relaxed);
            return Some(Err(()));
        }
        match child.try_wait() {
            Ok(Some(status)) => Some(Ok(status)),
            Ok(None) => None,
            Err(_) => Some(Err(())),
        }
    }

    fn finish(&self, spawned: std::io::Result<Child>) -> bool {
        let mut child = match spawned {
            Ok(child) => child,
            Err(e) => {
                self.note(&format!("could not start: {e}"));
                return false;
            }
        };
        loop {
            match self.poll(&mut child) {
                Some(Ok(status)) => {
                    if !status.success() {
                        self.note(&format!("exited with {status}"));
                    }
                    return status.success();
                }
                Some(Err(())) => {
                    match self.halted_by.get() {
                        Some(by) => self.note(&format!("stopped: \"{by}\" failed")),
                        None => self.note("could not wait for it to finish"),
                    }
                    return false;
                }
                None => std::thread::sleep(Duration::from_millis(50)),
            }
        }
    }

    /// Keeps what a test build produced, for the jobs that run it.
    pub(crate) fn keep_build(&self, key: &'static str, build: Build) {
        if let Ok(mut builds) = self.builds.lock() {
            builds.insert(key, build);
        }
    }

    /// What the test build `key` produced, once it has run.
    pub(crate) fn build(&self, key: &str) -> Option<Build> {
        self.builds.lock().ok()?.get(key).cloned()
    }
}

/// Starts `command` with its output in `log`, for a job that runs several at once. The caller
/// prepares the environment, since what it sets would not survive [`as_typed`] after it.
pub(crate) fn spawn_into(command: &mut Command, log: &Path) -> std::io::Result<Child> {
    let file = File::create(log)?;
    let err = file.try_clone()?;
    command
        .stdin(Stdio::null())
        .stdout(file)
        .stderr(err)
        .spawn()
}

/// A command as a person would type it, for the log.
fn shown(command: &Command) -> String {
    let mut text = command.get_program().to_string_lossy().into_owned();
    for arg in command.get_args() {
        text.push(' ');
        text.push_str(&arg.to_string_lossy());
    }
    text
}

enum State {
    Waiting,
    Running(Instant),
    Done(Outcome),
}

/// How often the console hears which jobs are still going.
const HEARTBEAT: Duration = Duration::from_secs(30);

/// Runs `plan` and returns every job's verdict in plan order, with how long each took.
pub(crate) fn run(
    root: &Path,
    plan: Plan,
    keep_going: bool,
    palette: &Palette,
) -> (Vec<Step>, Timings) {
    let logs = root.join("target/gate");
    let _ = std::fs::remove_dir_all(&logs);
    let _ = std::fs::create_dir_all(&logs);
    let halted_by: Arc<OnceLock<String>> = Arc::default();
    let builds = Arc::new(Mutex::new(HashMap::new()));
    let colour = !palette.bold.is_empty();
    let labels: Vec<String> = plan.jobs.iter().map(|j| j.label.clone()).collect();
    let mut pending: Vec<Option<Job>> = plan.jobs.into_iter().map(Some).collect();
    let mut state: Vec<State> = labels.iter().map(|_| State::Waiting).collect();
    let mut held: HashMap<&'static str, JobId> = HashMap::new();
    let mut timings = Timings::new();
    let (tx, rx) = mpsc::channel::<(JobId, Outcome, bool)>();
    let started = Instant::now();
    // The job whose failure stopped the rest, unless the gate keeps going.
    let mut halted: Option<JobId> = None;
    let mut last_beat = Instant::now();

    loop {
        for id in 0..pending.len() {
            let Some(job) = &pending[id] else { continue };
            let blocked_by = job.after.iter().find(|&&a| match &state[a] {
                State::Done(outcome) => !matches!(outcome, Outcome::Pass),
                _ => false,
            });
            if let Some(by) = halted {
                pending[id] = None;
                let why = format!("not run: \"{}\" failed", labels[by]);
                state[id] = State::Done(Outcome::Skip(why));
                continue;
            }
            if let Some(&dep) = blocked_by {
                pending[id] = None;
                let why = format!("not run: \"{}\" did not pass", labels[dep]);
                state[id] = State::Done(Outcome::Skip(why));
                continue;
            }
            let ready = job
                .after
                .iter()
                .all(|&a| matches!(state[a], State::Done(_)));
            let free = job.lock.is_none_or(|lock| !held.contains_key(lock));
            if !(ready && free) {
                continue;
            }
            let job = pending[id].take().expect("checked above");
            if let Some(lock) = job.lock {
                held.insert(lock, id);
            }
            state[id] = State::Running(Instant::now());
            println!("{}==> {}{}", palette.bold, job.label, palette.reset);
            let ctx = Ctx {
                root: root.to_path_buf(),
                log: logs.join(format!("{id:02}-{}.log", slug(&job.label))),
                halted_by: Arc::clone(&halted_by),
                stopped: AtomicBool::new(false),
                colour,
                builds: Arc::clone(&builds),
            };
            let tx = tx.clone();
            let work = job.work;
            std::thread::spawn(move || {
                let outcome = work(&ctx);
                let _ = tx.send((id, outcome, ctx.stopped.load(Ordering::Relaxed)));
            });
        }

        let running: Vec<JobId> = (0..state.len())
            .filter(|&id| matches!(state[id], State::Running(_)))
            .collect();
        if running.is_empty() {
            break;
        }
        match rx.recv_timeout(Duration::from_millis(500)) {
            Ok((id, outcome, stopped)) => {
                held.retain(|_, holder| *holder != id);
                let outcome = match (outcome, halted) {
                    (Outcome::Fail, Some(by)) if stopped => {
                        Outcome::Skip(format!("stopped: \"{}\" failed", labels[by]))
                    }
                    (outcome, _) => outcome,
                };
                if let State::Running(since) = state[id] {
                    timings.insert(labels[id].clone(), since.elapsed());
                }
                let failed = matches!(outcome, Outcome::Fail | Outcome::Need(_));
                report(&labels[id], &outcome, &timings, &logs, id, palette);
                state[id] = State::Done(outcome);
                if failed && !keep_going && halted.is_none() {
                    halted = Some(id);
                    let _ = halted_by.set(labels[id].clone());
                }
            }
            Err(_) if last_beat.elapsed() >= HEARTBEAT => {
                last_beat = Instant::now();
                let names: Vec<&str> = running.iter().map(|&id| labels[id].as_str()).collect();
                println!(
                    "    ... {}s in, still running: {}",
                    started.elapsed().as_secs(),
                    names.join(", ")
                );
            }
            Err(_) => {}
        }
    }

    let steps = labels
        .into_iter()
        .zip(state)
        .map(|(label, state)| match state {
            State::Done(outcome) => (label, outcome),
            _ => (label, Outcome::Skip("not run".to_owned())),
        })
        .collect();
    (steps, timings)
}

/// Prints a job's verdict as it lands, and the whole log of one that failed.
fn report(
    label: &str,
    outcome: &Outcome,
    timings: &Timings,
    logs: &Path,
    id: JobId,
    palette: &Palette,
) {
    let took = timings.get(label).map_or(0.0, Duration::as_secs_f64);
    match outcome {
        Outcome::Pass => println!(
            "{}ok{}   {took:6.1}s  {label}",
            palette.green, palette.reset
        ),
        Outcome::Skip(why) => println!(
            "{}skip{}          {label}: {why}",
            palette.yellow, palette.reset
        ),
        Outcome::Fail | Outcome::Need(_) => {
            let log = logs.join(format!("{id:02}-{}.log", slug(label)));
            if let Ok(text) = std::fs::read_to_string(&log) {
                for line in text.lines().filter(|l| !is_progress(l)) {
                    println!("{line}");
                }
            }
            let why = match outcome {
                Outcome::Need(why) => format!(" needs {why}"),
                _ => " failed".to_owned(),
            };
            eprintln!(
                "{}!! {label}{why} ({took:.1}s; log: {}){}",
                palette.red,
                log.display(),
                palette.reset
            );
        }
    }
}

/// Cargo's progress lines, which a failure's log is clearer without.
fn is_progress(line: &str) -> bool {
    let line = line.trim_start();
    [
        "Compiling ",
        "Checking ",
        "Documenting ",
        "Fresh ",
        "Blocking ",
        "Downloaded ",
        "Locking ",
    ]
    .iter()
    .any(|p| line.starts_with(p))
}

/// A label as a file name: lowercase letters and digits, the rest folded into single dashes.
fn slug(label: &str) -> String {
    let mut out = String::new();
    for c in label.chars() {
        if c.is_ascii_alphanumeric() {
            out.push(c.to_ascii_lowercase());
        } else if !out.ends_with('-') {
            out.push('-');
        }
    }
    out.trim_matches('-').chars().take(48).collect()
}

#[cfg(test)]
#[path = "gate_sched_tests.rs"]
mod tests;
