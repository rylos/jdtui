//! Refreshes the snapshot on a background thread, and listens to the
//! JDownloader event channel on another.
//!
//! A refresh is four round trips through the My.JDownloader relay, well into
//! the hundreds of milliseconds; keeping it off the input thread is what
//! keeps the interface responsive. The slow-changing status (stop mark,
//! crawling, extraction, captchas) is four more, fetched every
//! `STATUS_EVERY` refreshes and right after an action or an event.
//!
//! The event listener holds its own session, because `listen` blocks for up
//! to 25 seconds and the shared api must stay free for actions. Any event
//! wakes the refresh thread. While the channel is alive and nothing is
//! downloading, the periodic refresh stretches to `IDLE_PERIOD`: events
//! cover the changes, and the relay sees far fewer calls.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, Sender, channel};
use std::thread;
use std::time::{Duration, Instant};

use crate::api::{JdApi, SharedApi, Snapshot, Status, describe_error};
use crate::myjd::MyJd;

const STATUS_EVERY: u32 = 5;
/// Refresh period while events are flowing and nothing downloads.
const IDLE_PERIOD: Duration = Duration::from_secs(30);
/// Wait before opening the channel again after it failed.
const RETRY_AFTER: Duration = Duration::from_secs(10);
/// Least time between a refresh and the next one triggered by a wake.
const WAKE_GAP: Duration = Duration::from_millis(500);

pub enum Update {
    Snapshot(Snapshot),
    Error(String),
    /// The event channel came up or went down.
    Events(bool),
}

/// Credentials for the listener's own session.
pub struct EventSource {
    pub email: String,
    pub password: String,
    pub device_id: String,
}

pub struct Poller {
    rx: Receiver<Update>,
    wake: Sender<()>,
    stop: Arc<AtomicBool>,
    /// Device switches for the listener; `None` without events.
    device: Option<Sender<String>>,
}

impl Poller {
    pub fn start(api: SharedApi, period: Duration, events: Option<EventSource>) -> Self {
        let (tx, rx) = channel::<Update>();
        let (wake_tx, wake_rx) = channel::<()>();
        let stop = Arc::new(AtomicBool::new(false));
        let events_alive = Arc::new(AtomicBool::new(false));

        let device = events.map(|source| {
            let (device_tx, device_rx) = channel::<String>();
            let (tx, wake, stop, alive) = (tx.clone(), wake_tx.clone(), stop.clone(), events_alive.clone());
            thread::spawn(move || listen(source, device_rx, tx, wake, stop, alive));
            device_tx
        });

        let stop_flag = stop.clone();
        thread::spawn(move || {
            let mut status = Status::default();
            let mut tick: u32 = 0;
            let mut woken = true;
            let mut running = true;
            while !stop_flag.load(Ordering::Relaxed) {
                let started = Instant::now();
                let result = api.lock().map(|mut a| {
                    if woken || tick.is_multiple_of(STATUS_EVERY) {
                        status = a.status()?;
                    }
                    a.snapshot(status.clone())
                });
                let update = match result {
                    Ok(Ok(s)) => {
                        running = s.is_running();
                        Update::Snapshot(s)
                    }
                    Ok(Err(e)) => Update::Error(describe_error(&e)),
                    Err(_) => Update::Error("api lock poisoned".into()),
                };
                if tx.send(update).is_err() {
                    break;
                }
                tick = tick.wrapping_add(1);
                // Sleep until the period elapses or someone asks for an
                // early refresh: an action after it succeeds, or an event.
                let wait =
                    if events_alive.load(Ordering::Relaxed) && !running { period.max(IDLE_PERIOD) } else { period };
                woken = wake_rx.recv_timeout(wait).is_ok();
                if woken && started.elapsed() < WAKE_GAP {
                    // Events come in bursts: let them settle before refreshing.
                    thread::sleep(WAKE_GAP - started.elapsed());
                }
                while wake_rx.try_recv().is_ok() {}
            }
        });

        Poller { rx, wake: wake_tx, stop, device }
    }

    pub fn try_recv(&self) -> Option<Update> {
        self.rx.try_recv().ok()
    }

    /// Ask for a refresh now rather than at the next tick.
    pub fn refresh_now(&self) {
        let _ = self.wake.send(());
    }

    /// Point the event listener at another JDownloader of the account.
    pub fn set_device(&self, device_id: String) {
        if let Some(d) = &self.device {
            let _ = d.send(device_id);
        }
    }
}

impl Drop for Poller {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        let _ = self.wake.send(());
    }
}

/// The listener thread: subscribe, then `listen` in a loop, waking the
/// refresh thread on every batch of events. Any failure closes the channel
/// and reopens it after a pause; a device switch resubscribes there.
fn listen(
    source: EventSource,
    device_rx: Receiver<String>,
    tx: Sender<Update>,
    wake: Sender<()>,
    stop: Arc<AtomicBool>,
    alive: Arc<AtomicBool>,
) {
    let mut device_id = source.device_id;
    let mut api: Option<JdApi> = None;
    while !stop.load(Ordering::Relaxed) {
        // A session of its own, reused across resubscriptions.
        let api = match &mut api {
            Some(a) => a,
            None => {
                let mut myjd = MyJd::new(&source.email, &source.password);
                if myjd.connect().is_err() {
                    thread::sleep(RETRY_AFTER);
                    continue;
                }
                api.insert(JdApi::new(myjd, device_id.clone()))
            }
        };
        api.set_device(device_id.clone());
        let subscription = match api.subscribe_events() {
            Ok(id) => id,
            Err(_) => {
                thread::sleep(RETRY_AFTER);
                continue;
            }
        };
        alive.store(true, Ordering::Relaxed);
        let _ = tx.send(Update::Events(true));

        let failed = loop {
            if stop.load(Ordering::Relaxed) {
                break false;
            }
            if let Ok(id) = device_rx.try_recv() {
                device_id = id;
                let _ = api.unsubscribe_events(subscription);
                break false;
            }
            match api.listen_events(subscription) {
                Ok(events) => {
                    if !events.is_empty() {
                        let _ = wake.send(());
                    }
                }
                Err(_) => break true,
            }
        };

        alive.store(false, Ordering::Relaxed);
        let _ = tx.send(Update::Events(false));
        if failed {
            // The session may be the problem: open a fresh one next time.
            api.set_device(device_id.clone());
            thread::sleep(RETRY_AFTER);
        }
    }
}

#[cfg(test)]
mod live {
    //! Against the real service: with a one-minute period, a change made
    //! from another session must show up within seconds, through the
    //! event channel. Run with `cargo test -- --ignored --nocapture`.
    use super::*;
    use crate::api::AddLinks;
    use crate::config::Config;
    use std::sync::Mutex;

    const NAME: &str = "jdtui-events-test";

    fn session(cfg: &Config) -> JdApi {
        let mut myjd = MyJd::new(cfg.email.as_deref().unwrap(), cfg.password.as_deref().unwrap());
        myjd.connect().expect("connect");
        let device = cfg.device.clone().expect("a device in the config");
        JdApi::new(myjd, device)
    }

    fn wait_for<T>(poller: &Poller, what: &str, seconds: u64, mut f: impl FnMut(Update) -> Option<T>) -> T {
        let deadline = Instant::now() + Duration::from_secs(seconds);
        while Instant::now() < deadline {
            if let Some(u) = poller.try_recv() {
                match &u {
                    Update::Snapshot(s) => println!("  [update] snapshot: {} grabber packages", s.grabber.len()),
                    Update::Error(e) => println!("  [update] error: {e}"),
                    Update::Events(live) => println!("  [update] events live: {live}"),
                }
                if let Some(v) = f(u) {
                    return v;
                }
            } else {
                thread::sleep(Duration::from_millis(100));
            }
        }
        panic!("timed out after {seconds}s waiting for {what}");
    }

    #[test]
    #[ignore]
    fn events_wake_the_refresh_long_before_the_period() {
        let cfg = Config::load().expect("config");
        let api = Arc::new(Mutex::new(session(&cfg)));
        let mut other = session(&cfg);
        let source = EventSource {
            email: cfg.email.clone().unwrap(),
            password: cfg.password.clone().unwrap(),
            device_id: cfg.device.clone().unwrap(),
        };
        // Leftovers of an earlier run would confuse the counts below.
        let old: Vec<i64> = other.grabber().unwrap().iter().filter(|p| p.name == NAME).map(|p| p.uuid).collect();
        if !old.is_empty() {
            other.remove(&[], &old, true).expect("remove leftovers");
        }
        let poller = Poller::start(api, Duration::from_secs(60), Some(source));

        // The channel and the first snapshot come up in either order.
        let (mut snapshot, mut channel) = (false, false);
        wait_for(&poller, "the first snapshot and the event channel", 30, |u| {
            match u {
                Update::Snapshot(_) => snapshot = true,
                Update::Events(true) => channel = true,
                _ => {}
            }
            (snapshot && channel).then_some(())
        });
        println!("channel up");

        let t = Instant::now();
        other
            .add_links(&AddLinks {
                links: "http://example.com/jdtui-events-test.bin".into(),
                package_name: NAME.into(),
                ..Default::default()
            })
            .expect("add links");
        let uuid = wait_for(&poller, "a snapshot with the new package", 15, |u| match u {
            Update::Snapshot(s) => s.grabber.iter().find(|p| p.name == NAME).map(|p| p.uuid),
            _ => None,
        });
        println!("package seen after {:?}", t.elapsed());
        assert!(t.elapsed() < Duration::from_secs(15));

        let t = Instant::now();
        other.remove(&[], &[uuid], true).expect("remove");
        wait_for(&poller, "a snapshot without the package", 15, |u| match u {
            Update::Snapshot(s) => s.grabber.iter().all(|p| p.uuid != uuid).then_some(()),
            _ => None,
        });
        println!("removal seen after {:?}", t.elapsed());

        // Switching device resubscribes: the channel drops and comes back,
        // and still wakes the refresh afterwards.
        poller.set_device(cfg.device.clone().unwrap());
        wait_for(&poller, "the channel to drop", 40, |u| matches!(u, Update::Events(false)).then_some(()));
        wait_for(&poller, "the channel to come back", 40, |u| matches!(u, Update::Events(true)).then_some(()));
        println!("channel back after the device switch");
        let t = Instant::now();
        other
            .add_links(&AddLinks {
                links: "http://example.com/jdtui-events-test-2.bin".into(),
                package_name: NAME.into(),
                ..Default::default()
            })
            .expect("add links again");
        let uuid = wait_for(&poller, "a snapshot with the second package", 15, |u| match u {
            Update::Snapshot(s) => s.grabber.iter().find(|p| p.name == NAME).map(|p| p.uuid),
            _ => None,
        });
        println!("second package seen after {:?}", t.elapsed());
        other.remove(&[], &[uuid], true).expect("remove");
        wait_for(&poller, "a snapshot without it", 15, |u| match u {
            Update::Snapshot(s) => s.grabber.iter().all(|p| p.uuid != uuid).then_some(()),
            _ => None,
        });
    }
}
