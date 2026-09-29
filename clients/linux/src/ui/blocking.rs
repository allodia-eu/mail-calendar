//! One of the core's blocking calls, run off the GTK main thread, with its answer handed back to
//! it.
//!
//! A thread of its own per call, as the account connect and the provider sign-ins already take.
//! Not the host runtime: that one drives the desktop portal (`crate::host_runtime`), and a learning
//! request waits up to five minutes for its answer.

/// Runs `work` on a thread of its own and `answer` on the main loop with what it returned.
///
/// `answer` holds widgets, so it never leaves the main thread; only the value crosses.
pub(super) fn off_main_thread<T: Send + 'static>(
    work: impl FnOnce() -> T + Send + 'static,
    answer: impl FnOnce(T) + 'static,
) {
    let (sender, receiver) = relm4::channel::<T>();
    std::thread::spawn(move || sender.emit(work()));
    gtk::glib::MainContext::default().spawn_local(async move {
        if let Some(value) = receiver.recv().await {
            answer(value);
        }
    });
}
