//! Ending a step's processes when the gate stops it.
//!
//! [`Child::kill`] ends only the process the gate started, and the work runs below it: `rustc`
//! under `cargo`, and on Windows the Gradle client under the `cmd.exe` that runs `gradlew.bat`.
//! Left running, they finish a build nobody waits for, beside whatever runs next.

use std::process::{Child, Command, ExitStatus, Stdio};

/// Ends `child` and every process started under it, then reaps `child`.
pub(crate) fn kill_tree(child: &mut Child) {
    let root = child.id();
    #[cfg(windows)]
    let _ = quiet(Command::new("taskkill").args(["/T", "/F", "/PID", &root.to_string()]));
    #[cfg(not(windows))]
    {
        let mut pids = vec![root];
        if let Ok(table) = Command::new("ps")
            .args(["-A", "-o", "pid=", "-o", "ppid="])
            .stdin(Stdio::null())
            .stderr(Stdio::null())
            .output()
        {
            pids.extend(descendants(&String::from_utf8_lossy(&table.stdout), root));
        }
        // The step's own process first, so it starts nothing new while the rest are ended.
        let _ = quiet(
            Command::new("kill")
                .arg("-KILL")
                .args(pids.iter().map(u32::to_string)),
        );
    }
    let _ = child.kill();
    let _ = child.wait();
}

fn quiet(command: &mut Command) -> std::io::Result<ExitStatus> {
    command
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
}

/// Every process below `root`, read from a table of `pid ppid` lines as `ps` prints them.
#[cfg(any(not(windows), test))]
fn descendants(table: &str, root: u32) -> Vec<u32> {
    let pairs: Vec<(u32, u32)> = table
        .lines()
        .filter_map(|line| {
            let mut fields = line.split_whitespace();
            Some((fields.next()?.parse().ok()?, fields.next()?.parse().ok()?))
        })
        .collect();
    let mut found = Vec::new();
    let mut parents = vec![root];
    while let Some(parent) = parents.pop() {
        for &(pid, ppid) in &pairs {
            if ppid == parent && pid != root && !found.contains(&pid) {
                found.push(pid);
                parents.push(pid);
            }
        }
    }
    found
}

#[cfg(test)]
mod tests {
    use super::descendants;

    #[test]
    fn every_process_below_the_step_is_found_and_nothing_beside_it() {
        let table = "    1     0\n  100     1\n  101   100\n  102   101\n  103   101\n  200     1\n  \
                     201   200\nnot a row\n";
        let mut below = descendants(table, 100);
        below.sort_unstable();
        assert_eq!(below, [101, 102, 103]);
        assert_eq!(descendants(table, 999), Vec::<u32>::new());
    }
}
