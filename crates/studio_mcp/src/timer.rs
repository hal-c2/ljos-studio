//! The time a server reads and the wake-ups it asks for.
//!
//! The reload budget (`preview.rs`) counts reloads in the last second and
//! catches up the one a burst left out once that second is over. Natively a
//! thread sleeps until then; in a browser there are no threads to sleep on
//! and `std::time::Instant` and `SystemTime` panic, so the page's own clock and
//! `setTimeout` do it. The server is given a [`Timer`] and never asks the
//! platform itself.

use std::time::Duration;

/// A monotonic clock and one-shot wake-ups.
pub trait Timer: Send + Sync {
    /// Time since the timer's own origin; never goes back.
    fn now(&self) -> Duration;

    /// Time since the Unix epoch, for what a person reads back as a date
    /// (a review note's time); it may go back when the clock is set.
    fn wall(&self) -> Duration;

    /// Calls `then` once, `delay` from now. It may run on another thread
    /// (natively) or from the page's event loop (web), never inside this
    /// call.
    fn after(&self, delay: Duration, then: Box<dyn FnOnce() + Send>);

    /// Blocks the caller for `delay` and answers true, for a reader that
    /// must see a write the budget held back. A timer whose caller cannot
    /// block (a page's one thread) answers false at once.
    fn wait(&self, delay: Duration) -> bool {
        let _ = delay;
        false
    }
}

/// A thread per wake-up: there is at most one per burst of writes, and it
/// ends, so this is a timer and not a poll.
#[cfg(feature = "native")]
#[derive(Debug)]
pub struct ThreadTimer {
    origin: std::time::Instant,
}

#[cfg(feature = "native")]
impl Default for ThreadTimer {
    fn default() -> Self {
        Self {
            origin: std::time::Instant::now(),
        }
    }
}

#[cfg(feature = "native")]
impl Timer for ThreadTimer {
    fn now(&self) -> Duration {
        self.origin.elapsed()
    }

    fn wall(&self) -> Duration {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
    }

    fn after(&self, delay: Duration, then: Box<dyn FnOnce() + Send>) {
        std::thread::spawn(move || {
            std::thread::sleep(delay);
            then();
        });
    }

    fn wait(&self, delay: Duration) -> bool {
        std::thread::sleep(delay);
        true
    }
}
