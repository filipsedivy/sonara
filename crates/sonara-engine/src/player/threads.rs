//! The player's threads (not real-time code): the render thread's pacing loop around the
//! real-time [`RenderLoop::step`], the prefetch thread and the maintenance thread.
//!
//! The render thread rests between steps as the prototype's does: 20 ms when the transport is
//! stopped and has nothing to render, half a block at 48 kHz (5.33 ms) while it waits for the
//! device. The rest happens here, outside the real-time step: the step itself never sleeps,
//! locks or allocates (RT-03), as a device callback does not, although its thread idles between
//! callbacks.
//!
//! Before its loop, the render thread runs its optional [`RenderThreadStart`] hook once: setup
//! that belongs on the thread itself, such as joining the output device's audio workgroup or
//! requesting its scheduling class. The hook runs before the first [`RenderLoop::step`],
//! so the step's rules do not bind it. What the hook returns, if anything, the thread holds
//! through its loop and drops after the last step, before it exits: teardown on the same
//! thread, such as leaving the workgroup, so the step's rules (RT-02, RT-03) do not bind that
//! either. Nothing of the hook runs inside the loop.

use std::any::Any;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, PoisonError};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use super::ProjectSource;
use super::control::{Core, service};
use super::prefetch::Prefetch;
use super::render::{RenderLoop, Step};

/// The render thread's rest when the transport has nothing to render (`IDLE_SLEEP`).
const IDLE_REST: Duration = Duration::from_millis(20);
/// The render thread's rest while it waits for the device: half a block at 48 kHz.
const WAIT_REST: Duration = Duration::from_micros(512 * 1_000_000 / 48_000 / 2);
/// How often the prefetch thread tops its feed up.
const PREFETCH_REST: Duration = Duration::from_millis(2);
/// How often the maintenance thread runs.
const SERVICE_REST: Duration = Duration::from_millis(5);

/// Setup that the `sonara-render` thread runs once, on itself, before its loop: for example
/// joining the output device's audio workgroup (`sonara_io::thread::join_audio_workgroup`) or
/// requesting its scheduling class (`sonara_io::thread::raise_render_qos`). It runs outside
/// the real-time loop, so it may make system calls, allocate and log; it must return, because
/// the loop starts only afterwards. It cannot fail the player: a hook reports its own errors.
///
/// The hook may return a guard that must live as long as the thread's loop and drop on the
/// thread itself, such as a workgroup membership, which leaves the workgroup on drop. The thread
/// holds it through the loop and drops it after the last step, before it exits. It need not be
/// `Send`: it never leaves the thread. `None` holds nothing.
pub type RenderThreadStart = Box<dyn FnOnce() -> Option<Box<dyn Any>> + Send>;

/// The prefetch thread's work, for a test that drives it by hand ([`Player::manual`]).
///
/// [`Player::manual`]: super::Player::manual
#[derive(Debug, Clone)]
pub struct PrefetchWorker {
    prefetch: Arc<Mutex<Prefetch>>,
}

impl PrefetchWorker {
    pub(crate) fn new(prefetch: Arc<Mutex<Prefetch>>) -> Self {
        Self { prefetch }
    }

    /// Takes back the blocks the render thread used and reads the clips ahead until the feed
    /// is full; returns the blocks read. The lock is released between blocks. Not real-time:
    /// reads media.
    pub fn fill(&self) -> usize {
        let mut read = 0;
        while self
            .prefetch
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .fill_one()
        {
            read += 1;
        }
        read
    }
}

/// The running threads of a [`Player`](super::Player).
pub(crate) struct Threads {
    quit: Arc<AtomicBool>,
    render: Option<JoinHandle<RenderLoop>>,
    prefetch: Option<JoinHandle<()>>,
    service: Option<JoinHandle<()>>,
}

impl Threads {
    /// Starts the render, prefetch and maintenance threads. The render thread runs
    /// `render_start`, if any, once before its loop, and drops the guard it returns after the
    /// loop.
    ///
    /// # Errors
    ///
    /// The system's error when a thread cannot be started; the threads already started are
    /// stopped again.
    pub(crate) fn spawn(
        mut render: RenderLoop,
        core: Arc<Mutex<Core>>,
        prefetch: Arc<Mutex<Prefetch>>,
        source: Arc<dyn ProjectSource>,
        render_start: Option<RenderThreadStart>,
    ) -> std::io::Result<Self> {
        let quit = Arc::new(AtomicBool::new(false));
        let mut threads = Self {
            quit: Arc::clone(&quit),
            render: None,
            prefetch: None,
            service: None,
        };
        let stop = Arc::clone(&quit);
        threads.render = Some(spawn("sonara-render", move || {
            // Setup on this thread, before the real-time loop.
            let guard = render_start.and_then(|start| start());
            // Acquire ↔ the Release store in `Drop`: nothing else is published with it.
            while !stop.load(Ordering::Acquire) {
                match render.step() {
                    Step::Wrote => {}
                    Step::Waiting => thread::sleep(WAIT_REST),
                    Step::Idle => thread::sleep(IDLE_REST),
                }
            }
            // Teardown on this thread, after the last step: a workgroup is left by the thread
            // that joined it, before that thread exits.
            drop(guard);
            // Returned to the joining thread, which drops it there (RT-09).
            render
        })?);
        let stop = Arc::clone(&quit);
        let worker = PrefetchWorker::new(prefetch);
        threads.prefetch = Some(spawn("sonara-prefetch", move || {
            while !stop.load(Ordering::Acquire) {
                worker.fill();
                thread::sleep(PREFETCH_REST);
            }
        })?);
        let stop = quit;
        threads.service = Some(spawn("sonara-player", move || {
            while !stop.load(Ordering::Acquire) {
                service(&core, &*source);
                thread::sleep(SERVICE_REST);
            }
        })?);
        Ok(threads)
    }
}

/// Spawns a named thread.
fn spawn<T: Send + 'static>(
    name: &str,
    body: impl FnOnce() -> T + Send + 'static,
) -> std::io::Result<JoinHandle<T>> {
    thread::Builder::new().name(name.to_owned()).spawn(body)
}

impl Drop for Threads {
    fn drop(&mut self) {
        // Release ↔ the Acquire loads of the loops.
        self.quit.store(true, Ordering::Release);
        if let Some(handle) = self.render.take() {
            // The render loop's state, graphs and rings are dropped on this thread (RT-09).
            drop(handle.join());
        }
        for handle in [self.prefetch.take(), self.service.take()]
            .into_iter()
            .flatten()
        {
            drop(handle.join());
        }
    }
}

#[cfg(test)]
mod tests {
    use std::any::Any;
    use std::sync::Arc;
    use std::sync::mpsc::{self, RecvTimeoutError, Sender, TryRecvError};
    use std::thread;
    use std::time::Duration;

    use sonara_core::plugins::builtin_registry;

    use crate::player::{
        AudioDevice, DeviceError, DeviceStream, Player, PlayerSettings, PlayerSetup,
        ProjectSnapshot, StreamRequest,
    };

    /// A device that never opens: the hook needs no stream.
    struct NoDevice;

    impl AudioDevice for NoDevice {
        fn start(&mut self, _request: &StreamRequest) -> Result<DeviceStream, DeviceError> {
            Err(DeviceError("no device".to_owned()))
        }

        fn stop(&mut self) {}
    }

    fn setup() -> PlayerSetup {
        PlayerSetup {
            registry: Arc::new(builtin_registry()),
            device: Box::new(NoDevice),
            source: Arc::new(|| -> Option<ProjectSnapshot> { None }),
            settings: PlayerSettings::default(),
        }
    }

    /// The hook runs on the render thread itself, once (a `QoS` class is set per thread).
    #[test]
    fn the_render_thread_runs_its_start_hook_once_on_itself() {
        let (sender, ran_on) = mpsc::channel();
        let player = Player::with_render_start(
            setup(),
            Some(Box::new(move || -> Option<Box<dyn Any>> {
                let name = thread::current().name().map(str::to_owned);
                sender.send(name).unwrap();
                None
            })),
        )
        .unwrap();
        let name = ran_on.recv_timeout(Duration::from_secs(10)).unwrap();
        assert_eq!(name.as_deref(), Some("sonara-render"));
        // The hook was consumed by its one call, and its sender with it.
        assert_eq!(
            ran_on.recv_timeout(Duration::from_secs(10)),
            Err(RecvTimeoutError::Disconnected)
        );
        drop(player);
    }

    /// A guard that reports the thread it is dropped on.
    struct Guard(Sender<Option<String>>);

    impl Drop for Guard {
        fn drop(&mut self) {
            let name = thread::current().name().map(str::to_owned);
            self.0.send(name).unwrap();
        }
    }

    /// The guard the hook returns lives through the loop and drops on the render thread after
    /// it, when the player stops its threads (a workgroup is left by the thread that
    /// joined it).
    #[test]
    fn the_render_thread_drops_its_start_guard_on_itself_after_the_loop() {
        let (started, hook_ran) = mpsc::channel();
        let (dropped_on, guard_dropped) = mpsc::channel();
        let player = Player::with_render_start(
            setup(),
            Some(Box::new(move || -> Option<Box<dyn Any>> {
                started.send(()).unwrap();
                Some(Box::new(Guard(dropped_on)))
            })),
        )
        .unwrap();
        hook_ran.recv_timeout(Duration::from_secs(10)).unwrap();
        // The loop runs: the guard is held.
        assert_eq!(guard_dropped.try_recv(), Err(TryRecvError::Empty));
        drop(player);
        // Dropping the player joined the render thread, which dropped the guard first.
        assert_eq!(
            guard_dropped.try_recv().unwrap().as_deref(),
            Some("sonara-render")
        );
    }
}
