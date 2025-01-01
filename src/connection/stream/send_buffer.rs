use std::collections::BTreeMap;
use bytes::{Bytes, BytesMut, BufMut};
use std::ops::Bound::{Included, Excluded};

use crate::common::seq32::Seq32;

pub struct SendBuffer {
    queue: BTreeMap<Seq32, BytesMut>,
    enqueue: Seq32,
    unsent: Seq32,
    size: usize,
    limit: usize,
    mss: usize,
}

impl SendBuffer {
    pub fn new(limit: usize, mss: usize) -> Self {
        SendBuffer {
            queue: BTreeMap::new(),
            enqueue: Seq32::from(0),
            unsent: Seq32::from(0),
            size: 0,
            limit: limit,
            mss: mss,
        }
    }

    pub fn writable_size(&self) -> usize {
        self.limit - self.size
    }

    pub fn push_back(&mut self, data: &Bytes) -> bool {
        if self.size + data.len() > self.limit {
            return false;
        }
        // if last segment was not full, extend last segment first
        let mut offset = 0;
        if let Some((&pos, last_buf)) = self.queue.iter_mut().last() {
            if pos >= self.unsent && self.mss > last_buf.len() {
                let copy_len = if self.mss - last_buf.len() < data.len() - offset {
                    self.mss - last_buf.len()
                } else {
                    data.len() - offset
                };
                last_buf.put_slice(&data[offset .. offset + copy_len]);
                offset += copy_len;
                self.enqueue += copy_len as u32;
            }
        }
        // create new segments
        while offset < data.len() {
            let copy_len = if self.mss < data.len() - offset {
                self.mss
            } else {
                data.len() - offset
            };

            self.queue.insert(self.enqueue, BytesMut::from(&data[offset .. offset + copy_len]));
            offset += copy_len;
            self.enqueue += copy_len as u32;
        }

        self.size += data.len();
        true
    }

    pub fn pop_unsent(&mut self) -> Option<(Seq32, &[u8])> {
        if self.unsent >= self.enqueue {
            return None;
        }

        let data = self.queue.get(&self.unsent).unwrap();
        let pos = self.unsent;
        self.unsent += data.len() as u32;

        Some((pos, data))
    }

    pub fn get(&self, pos: Seq32) -> Option<&[u8]> {
        self.queue.get(&pos).map(|x| x as &[u8])
    }

    pub fn ack(&mut self, mut start: Seq32, end: Seq32) -> usize {
        if end <= start || start >= self.unsent || end > self.unsent {
            return 0;
        }

        let mut acked = 0;
        if let Some((&orig_start, _)) = self.queue.iter().next() {
            while let Some((&pos, _)) = self.queue.range((Included(start), Excluded(end))).next() {
                self.queue.remove(&pos);
                start = pos + 1;
                acked += 1;
            }   
            if let Some((&new_start, _)) = self.queue.iter().next() {
                self.size -= *(new_start - orig_start) as usize;
            } else {
                self.size = 0;
            }         
        }
        acked
    }
}

mod tests {
    // Note this useful idiom: importing names from outer (for mod tests) scope.
    use super::*;
    use rand::Rng;

    #[test]
    fn send_buffer_basic_test() {
        let mut sbuf = SendBuffer::new(102, 10);
        assert_eq!(sbuf.push_back(&Bytes::from("12345678")), true);
        assert_eq!(sbuf.push_back(&Bytes::from("123456")), true);
        assert_eq!(sbuf.pop_unsent(), Some((Seq32::from(0), &Bytes::from("1234567812") as &[u8])));
        assert_eq!(sbuf.writable_size(), 88);
        assert_eq!(sbuf.ack(Seq32::from(0), Seq32::from(10)), 1);
        assert_eq!(sbuf.writable_size(), 98);
        assert_eq!(sbuf.ack(Seq32::from(0), Seq32::from(10)), 0);
        assert_eq!(sbuf.ack(Seq32::from(10), Seq32::from(14)), 0);
        assert_eq!(sbuf.pop_unsent(), Some((Seq32::from(10), &Bytes::from("3456") as &[u8])));
        assert_eq!(sbuf.pop_unsent(), None);
        assert_eq!(sbuf.get(Seq32::from(10)), Some(&Bytes::from("3456") as &[u8]));
        assert_eq!(sbuf.ack(Seq32::from(10), Seq32::from(14)), 1);
        assert_eq!(sbuf.writable_size(), 102);
        while sbuf.writable_size() > 0 {
            assert_eq!(sbuf.push_back(&Bytes::from("12")), true);
        }
        assert_eq!(sbuf.push_back(&Bytes::from("1")), false);
        for i in 0..10 {
            assert_eq!(sbuf.pop_unsent(), Some((Seq32::from(14 + 10 * i), &Bytes::from("1212121212") as &[u8])));
        }
        assert_eq!(sbuf.writable_size(), 0);
        assert_eq!(sbuf.ack(Seq32::from(14), Seq32::from(114)), 10);
        assert_eq!(sbuf.writable_size(), 100);
    }

    #[test]
    fn send_buffer_5gb_read_write_test() {
        let mut rng = rand::thread_rng();
        let mut random_buf0 = BytesMut::with_capacity(10000);
        let mut random_buf1 = BytesMut::with_capacity(10000);
        for _ in 0..2500 {
            random_buf0.put_u32(rng.gen::<u32>());
            random_buf1.put_u32(rng.gen::<u32>());
        }

        let data = [random_buf0.freeze(), random_buf1.freeze()];
        let limit = 10485760;
        let mut sbuf = SendBuffer::new(limit, 1400);
        let mut test_bytes = 0;
        let mut pos = Seq32::from(0);
        let mut slot = 0;
        while test_bytes < 5368709120u64 {
            assert_eq!(sbuf.push_back(&data[slot]), true);
            assert_eq!(sbuf.writable_size(), limit - (slot + 1) * data[0].len());

            let mut cmp_offset = 0;
            while let Some(segment) = sbuf.pop_unsent() {
                let len = segment.1.len();
                assert_eq!(segment.0, pos);
                assert_eq!(segment.1, &data[slot][cmp_offset .. cmp_offset + len]);
                pos += len as u32;
                cmp_offset += len;
            }

            test_bytes += data[slot].len() as u64;
            slot ^= 1;
            // cleanup send buffer every 2 rounds
            if slot == 0 {
                sbuf.ack(pos - 2 * data[0].len() as u32, pos);
            }
        }
    }
}
