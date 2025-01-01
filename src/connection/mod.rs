use std::time::Instant;

use crate::common::seq32::Seq32;
use crate::common::timer_list::{TimerList, TimerEvent, TimeValue};

mod stream;
use stream::Stream;

/// A LiteDT connection between two peers.
pub struct Connection {
    peer_id: u16,
    send_queue: [TimerList<Seq32, PacketEntry>; 3],
    streams: TimerList<u32, Stream>,
    timewait_streams: TimerList<u32, NOP>,
}


impl Connection {
    pub fn new(peer_id: u16) -> Self {
        Self {
            peer_id,
            send_queue: [TimerList::new(), TimerList::new(), TimerList::new()],
            streams: TimerList::new(),
            timewait_streams: TimerList::new(),
        }
    }
}

/// Used to track the inflight packets of a stream.
struct PacketEntry {
    delivered: u32,
    stream_id: u32,
    seq: Seq32,
    len: usize,
    is_app_limited: bool,
    retrans_round: u32,
    sent_time: Instant,
    rto_time: Instant ,
    delivered_time: Option<Instant>,
    first_tx_time: Option<Instant>,
}

/// Timer event for `PacketEntry`.
impl TimerEvent for PacketEntry {
    fn callback(self, now: TimeValue) {
        println!("packet expired at {:?}", now);
    }
}

/// A timer event that does nothing.
struct NOP;

/// Timer event for `NOP`.
impl TimerEvent for NOP {
    fn callback(self, _now: TimeValue) {}
}
