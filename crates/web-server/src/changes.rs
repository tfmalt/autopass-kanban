//! Source-change coalescing and monotonic change identity.
//!
//! Two separate defects motivate this module.
//!
//! **Fan-out.** The filesystem watcher used to call `events.send(())` on every
//! raw `notify` event. A single `git pull` or a multi-file `kanban` write became
//! dozens of `change` events, and each one made every connected client refetch
//! the full repository snapshot.
//!
//! **Lossiness.** The emitted events carried no id, so a client that
//! reconnected after a dropped connection, or that fell behind the broadcast
//! buffer, silently missed every change in the gap with no way to detect it.
//! That is invisible permanent staleness in a tool whose entire job is showing
//! current state.
//!
//! [`ChangeBroadcaster`] fixes both: raw signals are coalesced behind a debounce
//! window with a ceiling (so a sustained burst still produces periodic updates
//! rather than starving), and each coalesced burst increments a monotonic
//! generation that is published as the SSE event id.

use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use tokio::sync::{broadcast, mpsc};
use tokio::time::{Instant, sleep, sleep_until};

/// Quiet period after the last raw signal before a burst is published.
pub(crate) const DEFAULT_DEBOUNCE: Duration = Duration::from_millis(150);
/// Upper bound on how long a sustained burst (a long `git pull`) may delay a
/// published change. Without it, continuous filesystem activity would starve
/// clients of updates indefinitely.
pub(crate) const DEFAULT_CEILING: Duration = Duration::from_secs(1);

const BROADCAST_CAPACITY: usize = 128;

/// Publishes at most one change notification per coalesced burst of source
/// changes, each carrying a strictly increasing generation number.
#[derive(Debug, Clone)]
pub(crate) struct ChangeBroadcaster {
    generation: Arc<AtomicU64>,
    /// Capacity-1 channel: a burst collapses into at most one queued signal, so
    /// the coalescer never has to drain an unbounded backlog.
    signals: mpsc::Sender<()>,
    events: broadcast::Sender<u64>,
}

impl ChangeBroadcaster {
    pub(crate) fn new() -> Self {
        Self::with_timing(DEFAULT_DEBOUNCE, DEFAULT_CEILING)
    }

    /// Construct with explicit timing. Tests use this together with
    /// `tokio::time::pause()` so coalescing behavior is asserted against a
    /// controlled clock rather than real sleeps.
    pub(crate) fn with_timing(debounce: Duration, ceiling: Duration) -> Self {
        let (signals, signal_rx) = mpsc::channel(1);
        let (events, _) = broadcast::channel(BROADCAST_CAPACITY);
        let generation = Arc::new(AtomicU64::new(0));

        tokio::spawn(coalesce(
            signal_rx,
            generation.clone(),
            events.clone(),
            debounce,
            ceiling,
        ));

        Self {
            generation,
            signals,
            events,
        }
    }

    /// Signal that the markdown source may have changed.
    ///
    /// Never blocks and never fails: if a signal is already queued this one is
    /// folded into it, which is precisely the coalescing behavior wanted. Safe
    /// to call from the synchronous `notify` watcher callback.
    pub(crate) fn notify(&self) {
        let _ = self.signals.try_send(());
    }

    pub(crate) fn subscribe(&self) -> broadcast::Receiver<u64> {
        self.events.subscribe()
    }

    /// The most recently published generation. `0` means nothing has been
    /// published since startup.
    pub(crate) fn current_generation(&self) -> u64 {
        self.generation.load(Ordering::SeqCst)
    }
}

impl Default for ChangeBroadcaster {
    fn default() -> Self {
        Self::new()
    }
}

async fn coalesce(
    mut signals: mpsc::Receiver<()>,
    generation: Arc<AtomicU64>,
    events: broadcast::Sender<u64>,
    debounce: Duration,
    ceiling: Duration,
) {
    while signals.recv().await.is_some() {
        let burst_started = Instant::now();
        loop {
            let deadline = burst_started + ceiling;
            tokio::select! {
                biased;
                // Ceiling first: a sustained burst must still publish on time.
                _ = sleep_until(deadline) => break,
                signal = signals.recv() => {
                    if signal.is_none() {
                        return;
                    }
                }
                _ = sleep(debounce) => break,
            }
        }
        // Publish only after the burst is complete, so a subscriber that reacts
        // to generation N observes every change that produced it.
        let published = generation.fetch_add(1, Ordering::SeqCst) + 1;
        let _ = events.send(published);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fast() -> ChangeBroadcaster {
        ChangeBroadcaster::with_timing(Duration::from_millis(150), Duration::from_secs(1))
    }

    /// B7 + B8: one burst of raw filesystem events produces exactly one change
    /// notification.
    #[tokio::test(start_paused = true)]
    async fn a_burst_of_raw_events_publishes_exactly_one_generation() {
        let broadcaster = fast();
        let mut rx = broadcaster.subscribe();

        for _ in 0..50 {
            broadcaster.notify();
            // Let the coalescer drain the capacity-1 channel between signals so
            // this exercises repeated wake-ups, not just a dropped backlog.
            tokio::task::yield_now().await;
            tokio::time::advance(Duration::from_millis(1)).await;
        }

        tokio::time::advance(Duration::from_millis(200)).await;
        tokio::task::yield_now().await;

        assert_eq!(rx.recv().await.unwrap(), 1);
        assert!(
            rx.try_recv().is_err(),
            "a single burst must not publish a second generation"
        );
        assert_eq!(broadcaster.current_generation(), 1);
    }

    /// A burst longer than the ceiling must still publish, at the ceiling
    /// interval, rather than starving clients of updates.
    #[tokio::test(start_paused = true)]
    async fn a_sustained_burst_publishes_at_the_ceiling() {
        let broadcaster = fast();
        let mut rx = broadcaster.subscribe();

        // Keep signalling for well over the 1 s ceiling, never quiet for the
        // 150 ms debounce window.
        for _ in 0..40 {
            broadcaster.notify();
            tokio::task::yield_now().await;
            tokio::time::advance(Duration::from_millis(60)).await;
        }
        tokio::time::advance(Duration::from_millis(300)).await;
        tokio::task::yield_now().await;

        let first = rx.recv().await.unwrap();
        assert_eq!(first, 1, "the ceiling must publish before the burst ends");
        assert!(
            broadcaster.current_generation() >= 2,
            "a 2.4 s burst must publish more than once at a 1 s ceiling, got {}",
            broadcaster.current_generation()
        );
    }

    /// Generations are strictly increasing and never repeat, which is what makes
    /// them usable as resumable SSE event ids.
    #[tokio::test(start_paused = true)]
    async fn generations_are_strictly_monotonic() {
        let broadcaster = fast();
        let mut rx = broadcaster.subscribe();

        let mut seen = Vec::new();
        for _ in 0..5 {
            broadcaster.notify();
            tokio::time::advance(Duration::from_millis(300)).await;
            tokio::task::yield_now().await;
            seen.push(rx.recv().await.unwrap());
        }

        assert_eq!(seen, vec![1, 2, 3, 4, 5]);
    }

    /// Quiet periods publish nothing: a client that receives no event can trust
    /// that nothing changed.
    #[tokio::test(start_paused = true)]
    async fn no_signal_publishes_no_event() {
        let broadcaster = fast();
        let mut rx = broadcaster.subscribe();

        tokio::time::advance(Duration::from_secs(5)).await;
        tokio::task::yield_now().await;

        assert!(rx.try_recv().is_err());
        assert_eq!(broadcaster.current_generation(), 0);
    }
}
