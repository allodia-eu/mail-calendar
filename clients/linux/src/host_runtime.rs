//! The one Tokio runtime every host service reaching the desktop portal runs on.
//!
//! ⚠️ **`ashpd` keeps a single session-bus connection for the whole process** (a `OnceLock` in its
//! `Proxy`), and zbus drives that connection from whichever runtime opened it. Two host services
//! here reach the portal through `ashpd`: the secure store, because inside a sandbox `oo7` asks
//! `org.freedesktop.portal.Secret` for the keyring key, and new-mail notifications. A runtime
//! built per call, or owned by one of those services, takes the connection's reader with it when
//! it is dropped, while the connection itself stays cached. Every later portal call then awaits a
//! reply that can never arrive: **no error, no timeout, a thread parked for the life of the
//! process**, and the state machine that thread was serving parked with it.
//!
//! So there is exactly one runtime, it is never dropped, and no other module may build its own.

use std::sync::OnceLock;

use tokio::runtime::Runtime;

/// The shared runtime, or `None` when the process could not build one at all.
///
/// One worker is enough and is deliberate: everything scheduled here is asynchronous D-Bus and
/// keyring traffic that yields rather than blocks, and callers reach it through `block_on` from
/// their own threads.
pub(crate) fn shared() -> Option<&'static Runtime> {
    static RUNTIME: OnceLock<Option<Runtime>> = OnceLock::new();
    RUNTIME
        .get_or_init(|| {
            tokio::runtime::Builder::new_multi_thread()
                .worker_threads(1)
                .enable_all()
                .build()
                .ok()
        })
        .as_ref()
}

#[cfg(test)]
mod tests {
    #[test]
    fn every_caller_shares_one_runtime() {
        // The regression this pins: a runtime per caller is dropped when its caller finishes,
        // taking the process-global portal connection's reader with it, and the next portal call
        // hangs forever.
        let first = super::shared().expect("a host runtime");
        let second = super::shared().expect("a host runtime");

        assert!(
            std::ptr::eq(first, second),
            "every portal caller must run on the same runtime, or the shared connection dies \
             with the first one to finish"
        );
    }
}
