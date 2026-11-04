//! Cancellation ownership for bounded embedded shell operations.
use std::future::Future;
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
};

#[derive(Default)]
struct State {
    cancelled: AtomicBool,
    processes: Mutex<Vec<i32>>,
    tasks: Mutex<Vec<tokio::task::AbortHandle>>,
}

tokio::task_local! { static CURRENT: Arc<State>; }

/// Owns processes and asynchronous subshells created by an embedded operation.
/// Dropping the owner terminates the operation's process groups and tasks.
#[derive(Default)]
pub struct ProcessScope {
    state: Arc<State>,
}
impl ProcessScope {
    /// Creates a cancellation owner.
    pub fn new() -> Self {
        Self::default()
    }
    /// Executes with process ownership propagated into shell-created tasks.
    pub async fn run<F: Future>(&self, future: F) -> F::Output {
        CURRENT.scope(Arc::clone(&self.state), future).await
    }
}
impl Drop for ProcessScope {
    fn drop(&mut self) {
        self.state.cancelled.store(true, Ordering::SeqCst);
        for handle in self.state.tasks.lock().unwrap().iter() {
            handle.abort();
        }
        #[cfg(unix)]
        for pid in self.state.processes.lock().unwrap().iter() {
            let _ = nix::sys::signal::killpg(
                nix::unistd::Pid::from_raw(*pid),
                nix::sys::signal::Signal::SIGKILL,
            );
        }
    }
}

pub(crate) fn active() -> bool {
    CURRENT.try_with(|_| ()).is_ok()
}
pub(crate) struct ProcessRegistration {
    state: Arc<State>,
    pid: i32,
}
impl Drop for ProcessRegistration {
    fn drop(&mut self) {
        #[cfg(unix)]
        let _ = nix::sys::signal::killpg(
            nix::unistd::Pid::from_raw(self.pid),
            nix::sys::signal::Signal::SIGKILL,
        );
        self.state
            .processes
            .lock()
            .unwrap()
            .retain(|pid| *pid != self.pid);
    }
}

pub(crate) fn register(pid: u32) -> Option<ProcessRegistration> {
    CURRENT
        .try_with(|state| {
            #[expect(clippy::cast_possible_wrap)]
            let pid = pid as i32;
            state.processes.lock().unwrap().push(pid);
            #[cfg(unix)]
            if state.cancelled.load(Ordering::SeqCst) {
                let _ = nix::sys::signal::killpg(
                    nix::unistd::Pid::from_raw(pid),
                    nix::sys::signal::Signal::SIGKILL,
                );
            }
            ProcessRegistration {
                state: Arc::clone(state),
                pid,
            }
        })
        .ok()
}

pub(crate) fn spawn<F>(future: F) -> tokio::task::JoinHandle<F::Output>
where
    F: Future + Send + 'static,
    F::Output: Send + 'static,
{
    let state = CURRENT.try_with(Arc::clone).ok();
    if let Some(state) = state {
        let child_state = Arc::clone(&state);
        let handle = tokio::spawn(CURRENT.scope(child_state, future));
        state.tasks.lock().unwrap().push(handle.abort_handle());
        if state.cancelled.load(Ordering::SeqCst) {
            handle.abort();
        }
        handle
    } else {
        tokio::spawn(future)
    }
}
