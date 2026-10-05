//! The one logger this test binary installs, shared by every test that asserts on log lines.
//!
//! `log` allows exactly one logger per process and the cases run in parallel, so the buffer is
//! process-wide and each reader filters it by the modules it is about: a parallel test logging
//! from elsewhere in the crate can neither satisfy nor break another's claim.

use std::sync::{Arc, Mutex, OnceLock};

/// Every record logged since the capture was installed, with the module that logged it.
type Lines = Arc<Mutex<Vec<(String, String)>>>;

struct Capture(Lines);

impl log::Log for Capture {
    fn enabled(&self, _metadata: &log::Metadata<'_>) -> bool {
        true
    }

    fn log(&self, record: &log::Record<'_>) {
        self.0
            .lock()
            .unwrap()
            .push((record.target().to_owned(), record.args().to_string()));
    }

    fn flush(&self) {}
}

fn captured() -> &'static Lines {
    static LINES: OnceLock<Lines> = OnceLock::new();
    LINES.get_or_init(|| {
        let lines = Arc::new(Mutex::new(Vec::new()));
        log::set_boxed_logger(Box::new(Capture(Arc::clone(&lines))))
            .expect("this test binary installs no other logger");
        log::set_max_level(log::LevelFilter::Debug);
        lines
    })
}

/// Installs the capture, so a test can do so before the lines it wants are written.
pub(crate) fn install() {
    let _ = captured();
}

/// Every captured line logged from one of `targets`, in order.
pub(crate) fn lines_from(targets: &[&str]) -> Vec<String> {
    captured()
        .lock()
        .unwrap()
        .iter()
        .filter(|(target, _)| targets.contains(&target.as_str()))
        .map(|(_, line)| line.clone())
        .collect()
}
