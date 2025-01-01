extern crate alloc;

use alloc::{boxed::Box, collections::BinaryHeap};
use std::collections::HashMap;
use core::cmp::Reverse;
use core::time::Duration;
use std::hash::Hash;

/// The type of the time value.
///
/// Currently it is just an alias of [`core::time::Duration`].
pub type TimeValue = Duration;

/// A trait that all timed events must implement.
pub trait TimerEvent {
    /// Callback function that will be called when the timer expires.
    fn callback(self, now: TimeValue);
}

/// A list of timed events.
///
/// It internally uses a min-heap to store the events by deadline, make it
/// possible to trigger these events sequentially.
pub struct TimerList<K: Ord + Hash + Eq + Clone, E: TimerEvent> {
    map: HashMap<K, (TimeValue, E)>,
    events: BinaryHeap<Reverse<(TimeValue, K)>>,
}

impl<K: Ord + Hash + Eq + Clone, E: TimerEvent> TimerList<K, E> {
    /// Creates a new empty timer list.
    pub fn new() -> Self {
        Self {
            map: HashMap::new(),
            events: BinaryHeap::new(),
        }
    }

    /// Whether there is no timed event.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.map.is_empty()
    }

    /// Set a timed event that will be triggered at `deadline`.
    pub fn set(&mut self, deadline: TimeValue, key: K, event: E) {
        self.events.push(Reverse((deadline, key.clone())));
        self.map.insert(key, (deadline, event));
    }

    /// Get the event and deadline associated with the given key.
    pub fn get(&self, key: K) -> Option<(TimeValue, &E)> {
        self.map.get(&key).map(|e| (e.0, &e.1))
    }

    /// Cancel the event associated with the given key.
    pub fn cancel(&mut self, key: K)
    {
        if let Some((deadline, _)) = self.map.remove(&key) {
            self.events.retain(|e| e.0.0 != deadline || e.0.1 != key);
        }
    }

    /// Get the deadline of the most recent event.
    #[inline]
    pub fn next_deadline(&self) -> Option<TimeValue> {
        self.events.peek().map(|e| e.0.0)
    }

    /// Try to expire the earliest event that passed the deadline at the given
    /// time.
    ///
    /// Returns `None` if no event is expired.
    pub fn expire_one(&mut self, now: TimeValue) -> Option<(TimeValue, E)> {
        if let Some(e) = self.events.peek() {
            if e.0.0 <= now {
                let value = self.map.remove(&e.0.1).unwrap();
                return self.events.pop().map(|e| (e.0.0, value.1));
            }
        }
        None
    }
}

impl<K: Ord + Hash + Eq + Clone, E: TimerEvent> Default for TimerList<K, E> {
    fn default() -> Self {
        Self::new()
    }
}

/// A simple wrapper of a closure that implements the [`TimerEvent`] trait.
///
/// So that it can be used as in the [`TimerList`].
pub struct TimerEventFn(Box<dyn FnOnce(TimeValue) + 'static>);

impl TimerEventFn {
    /// Constructs a new [`TimerEventFn`] from a closure.
    pub fn new<F>(f: F) -> Self
    where
        F: FnOnce(TimeValue) + 'static,
    {
        Self(Box::new(f))
    }
}

impl TimerEvent for TimerEventFn {
    fn callback(self, now: TimeValue) {
        (self.0)(now)
    }
}

#[cfg(test)]
mod tests {
    use super::{TimeValue, TimerEvent, TimerEventFn, TimerList};
    use core::sync::atomic::{AtomicUsize, Ordering};
    use std::time::{Duration, Instant};

    #[test]
    fn test_timer_list() {
        const EVENT_ORDER: &[usize; 4] = &[1, 4, 3, 0]; // timer 2 was canceled
        static COUNT: AtomicUsize = AtomicUsize::new(0);

        struct TestTimerEvent(usize, TimeValue);

        impl TimerEvent for TestTimerEvent {
            fn callback(self, now: TimeValue) {
                let idx = COUNT.fetch_add(1, Ordering::SeqCst);
                assert_eq!(self.0, EVENT_ORDER[idx]);
                println!(
                    "timer {} expired at {:?}, delta = {:?}",
                    self.0,
                    now,
                    now - self.1
                );
            }
        }

        let mut timer_list = TimerList::new();
        let start_time = Instant::now();
        let deadlines = [
            Duration::new(3, 0),            // 3.0 sec
            Duration::from_micros(500_000), // 0.5 sec
            Duration::from_secs(4),         // 4.0 sec, canceled
            Duration::new(2, 990_000_000),  // 2.99 sec
            Duration::from_millis(1000),    // 1.0 sec,
        ];

        for (i, &ddl) in deadlines.iter().enumerate() {
            timer_list.set(ddl, i, TestTimerEvent(i, ddl));
        }

        assert_eq!(timer_list.next_deadline().unwrap(), Duration::from_micros(500_000));
        assert_eq!(timer_list.get(2).unwrap().0, Duration::from_secs(4));

        while !timer_list.is_empty() {
            let now = Instant::now().duration_since(start_time);
            if now.as_secs() > 3 {
                timer_list.cancel(2);
            }
            while let Some((_deadline, event)) = timer_list.expire_one(now) {
                event.callback(now);
            }
            std::thread::sleep(Duration::from_millis(10)); // sleep 10 ms
        }

        assert_eq!(COUNT.load(Ordering::SeqCst), 4);
        assert_eq!(timer_list.next_deadline(), None);
    }

    #[test]
    fn test_timer_list_fn() {
        let mut timer_list = TimerList::new();
        let start_time = Instant::now();
        let deadlines = [
            Duration::new(1, 1_000_000),    // 1.001 sec
            Duration::from_micros(750_000), // 0.75 sec
        ];
        let keys = [1, 2];

        for (i, &ddl) in deadlines.iter().enumerate() {
            timer_list.set(
                ddl,
                keys[i],
                TimerEventFn::new(|now| println!("timer fn expired at {:?}", now)),
            );
        }

        while !timer_list.is_empty() {
            let now = Instant::now().duration_since(start_time);
            while let Some((_deadline, event)) = timer_list.expire_one(now) {
                event.callback(now);
            }
        }
    }
}