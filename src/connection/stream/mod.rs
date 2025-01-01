mod send_buffer;
mod recv_buffer;

use send_buffer::SendBuffer;
use recv_buffer::RecvBuffer;

use crate::common::timer_list::{TimerEvent, TimeValue};

pub struct Stream {
    stream_id: u32,
    send_buffer: SendBuffer,
    recv_buffer: RecvBuffer,
}

impl Stream {
    pub fn new(stream_id: u32, send_limit: usize, recv_limit: u32, mss: usize) -> Self {
        Self {
            stream_id: stream_id,
            send_buffer: SendBuffer::new(send_limit, mss),
            recv_buffer: RecvBuffer::with_capacity(recv_limit),
        }
    }
}

impl TimerEvent for Stream {
    fn callback(self, now: TimeValue) {
        println!("stream expired at {:?}", now);
    }
}
