//! Refreshes the snapshot on a background thread.
//!
//! A full refresh is four round trips through the My.JDownloader relay, well
//! into the hundreds of milliseconds; keeping it off the input thread is what
//! keeps the interface responsive.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, Sender, channel};
use std::thread;
use std::time::Duration;

use crate::api::{SharedApi, Snapshot, describe_error};

pub enum Update {
    Snapshot(Snapshot),
    Error(String),
}

pub struct Poller {
    rx: Receiver<Update>,
    wake: Sender<()>,
    stop: Arc<AtomicBool>,
}

impl Poller {
    pub fn start(api: SharedApi, period: Duration) -> Self {
        let (tx, rx) = channel::<Update>();
        let (wake_tx, wake_rx) = channel::<()>();
        let stop = Arc::new(AtomicBool::new(false));
        let stop_flag = stop.clone();

        thread::spawn(move || {
            while !stop_flag.load(Ordering::Relaxed) {
                let result = api.lock().map(|mut a| a.snapshot());
                let update = match result {
                    Ok(Ok(s)) => Update::Snapshot(s),
                    Ok(Err(e)) => Update::Error(describe_error(&e)),
                    Err(_) => Update::Error("api lock poisoned".into()),
                };
                if tx.send(update).is_err() {
                    break;
                }
                // Sleep until the period elapses or someone asks for an
                // early refresh, which an action does after it succeeds.
                let _ = wake_rx.recv_timeout(period);
                while wake_rx.try_recv().is_ok() {}
            }
        });

        Poller { rx, wake: wake_tx, stop }
    }

    pub fn try_recv(&self) -> Option<Update> {
        self.rx.try_recv().ok()
    }

    /// Ask for a refresh now rather than at the next tick.
    pub fn refresh_now(&self) {
        let _ = self.wake.send(());
    }
}

impl Drop for Poller {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        let _ = self.wake.send(());
    }
}
