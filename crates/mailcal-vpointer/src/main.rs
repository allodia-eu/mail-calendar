//! Pointer input for the Linux client's headless test compositor. The tool, and why it exists
//! rather than `wlrctl`, is in `wayland.rs`.
//!
//! Linux only, like the compositor it drives. It is still a workspace member, so every
//! `--workspace` command compiles it on every host, and Wayland's client libraries do not build on
//! Windows. So its dependencies are Linux-only, and on any other host the binary says so and exits.

#[cfg(target_os = "linux")]
mod wayland;

#[cfg(target_os = "linux")]
fn main() -> std::process::ExitCode {
    wayland::main()
}

#[cfg(not(target_os = "linux"))]
fn main() -> std::process::ExitCode {
    eprintln!("mailcal-vpointer drives a Wayland compositor, which only Linux runs");
    std::process::ExitCode::FAILURE
}
