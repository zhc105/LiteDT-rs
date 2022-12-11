use std::collections::{BTreeMap, VecDeque};
use std::ops::Bound::{Included, Unbounded};
use bytes::{Bytes, BytesMut};

use crate::common::seq32::Seq32;

// Minimum size of recv buffer block allocation unit
const RBUF_BLOCK_BIT: u32 = 17;
const RBUF_BLOCK_SIZE: u32 = 1 << RBUF_BLOCK_BIT; // 128KB
const RBUF_BLOCK_MASK: u32 = RBUF_BLOCK_SIZE - 1;

pub struct RecvBuffer {
    start_pos: Seq32,
    max_blocks: u32,
    range_map: BTreeMap<Seq32, Seq32>,
    blocks: VecDeque<BytesMut>,
}

impl RecvBuffer {
    pub fn with_capacity(size: u32) -> Self {
        let mut bcnt = size / RBUF_BLOCK_SIZE;
        if size & RBUF_BLOCK_MASK != 0 {
            bcnt += 1;
        }

        RecvBuffer {
            start_pos: Seq32::from(0),
            max_blocks: bcnt,
            range_map: BTreeMap::new(),
            blocks: VecDeque::new(),
        }
    }

    pub fn readable_size(&self) -> usize {
        match self.range_map.iter().next() {
            Some(first) => {
                if *first.0 == self.start_pos {
                    *(*first.1 - *first.0) as usize
                } else {
                    0
                }
            }
            None => { 0 }
        }
    }

    pub fn peek(&self) -> Option<&[u8]> {
        let readable = self.readable_size();
        if readable > 0 {
            let blk_offset = *self.start_pos & RBUF_BLOCK_MASK;
            let peek_size = std::cmp::min(readable, (RBUF_BLOCK_SIZE - blk_offset) as usize);
            Some(&self.blocks[0][blk_offset as usize .. (blk_offset as usize) + peek_size])
        } else {
            None
        }
    }

    pub fn consume(&mut self, len: usize) -> Result<(), &'static str> {
        if len == 0 {
            return Ok(());
        }

        if self.readable_size() < len {
            return Err("no-enough-data");
        }

        let mut remain = len;
        let range = self.range_map.iter().next().unwrap();
        while remain > 0 {
            let block_size = (RBUF_BLOCK_SIZE - (*self.start_pos & RBUF_BLOCK_MASK)) as usize;
            if remain >= block_size {
                self.start_pos += block_size as u32;
                remain -= block_size;
                self.blocks.pop_front();
            } else {
                self.start_pos += remain as u32;
                remain = 0;
            }
        }

        let orig_pos = range.0.clone();
        let orig_end = range.1.clone();
        self.range_map.remove(&orig_pos);
        if self.start_pos != orig_end {
            self.range_map.insert(self.start_pos.clone(), orig_end);
        }
        
        Ok(())
    }

    pub fn write(&mut self, pos: Seq32, data: &Bytes) -> Result<(), &'static str> {
        let end = pos + (data.len() as u32);
        let max_size = RBUF_BLOCK_SIZE * self.max_blocks - (*self.start_pos & RBUF_BLOCK_MASK);
        if data.len() > max_size as usize {
            return Err("size-limit-exceed");
        }

        if *(pos - self.start_pos) > max_size || *(end - self.start_pos) > max_size {
            return Err("out-of-range");
        }

        // insert or update range map
        let mut merge_start = pos;
        let before = self.range_map.range_mut((Unbounded, Included(pos)));
        if let Some(last) = before.last() {
            if *last.1 >= pos {
                if *last.1 >= end {
                    return Err("duplicated-data");
                } else {
                    *last.1 = end;
                    merge_start = *last.0;
                }
            } else {
                self.range_map.insert(pos, end);
            }
        } else {
            self.range_map.insert(pos, end);
        }

        self.compress_range_map(&merge_start);

        // allocate and copy data to buffer block
        let block_start_pos = self.start_pos - (*self.start_pos & RBUF_BLOCK_MASK);
        let required_blocks = (*(end - block_start_pos) >> RBUF_BLOCK_BIT) +
            (if *end & RBUF_BLOCK_MASK == 0 { 0 } else { 1 });
        while self.blocks.len() < required_blocks as usize {
            self.blocks.push_back(BytesMut::zeroed(RBUF_BLOCK_SIZE as usize));
        }

        let mut remain = data.len();
        let mut buf_offset = pos;
        let mut data_offset = 0;
        while remain > 0 {
            let blk_id = *(buf_offset - block_start_pos) >> RBUF_BLOCK_BIT;
            let blk_offset = (*buf_offset & RBUF_BLOCK_MASK) as usize;
            let copy_len = std::cmp::min(remain, (RBUF_BLOCK_SIZE as usize - blk_offset) as usize);

            self.blocks[blk_id as usize][blk_offset .. blk_offset + copy_len].copy_from_slice(
                &data[data_offset .. data_offset + copy_len]);

            data_offset += copy_len;
            remain -= copy_len;
            buf_offset += copy_len as u32;
        }
        
        Ok(())
    }

    fn compress_range_map(&mut self, pos: &Seq32) {
        // merge overlapped intervals
        let mut keys_to_remove = vec![];
        let mut after = self.range_map.range_mut((Included(pos), Unbounded));
        let merge_range = after.next().unwrap();
        for (next_start, next_end) in after {
            if *merge_range.1 >= *next_start {
                if *next_end >= *merge_range.1 {
                    *merge_range.1 = *next_end;
                    keys_to_remove.push(*next_start);
                } else {
                    break;
                }
            } else {
                break;
            }
        }

        // remove overlapped intervals from range map
        keys_to_remove.iter().for_each(|x| { self.range_map.remove(x); });
    }
}

#[cfg(test)]
mod tests {
    // Note this useful idiom: importing names from outer (for mod tests) scope.
    use super::*;
    use rand::Rng;
    use bytes::BufMut;

    #[test]
    fn recv_buffer_basic_test() {
        let mut rbuf = RecvBuffer::with_capacity(13107200);

        assert_eq!(rbuf.write(Seq32::from(10), &Bytes::from("hello world")), Ok(()));
        assert_eq!(rbuf.readable_size(), 0);

        assert_eq!(rbuf.write(Seq32::from(524388), &Bytes::from("test~test~")), Ok(()));
        assert_eq!(rbuf.write(Seq32::from(200), &Bytes::from("test word1")), Ok(()));
        assert_eq!(rbuf.write(Seq32::from(0), &Bytes::from("test word2")), Ok(()));
        assert_eq!(rbuf.readable_size(), 21);

        let s = Bytes::from("test word2hello world");
        for i in 0..20 {
            assert_eq!(rbuf.peek(), Some(&s[i..]));
            assert_eq!(rbuf.consume(1), Ok(()));
        }

        assert_eq!(rbuf.write(Seq32::from(21), &Bytes::from("append new")), Ok(()));
        assert_eq!(rbuf.write(Seq32::from(10), &Bytes::from("error")), Err("out-of-range"));
        assert_eq!(rbuf.write(Seq32::from(21), &Bytes::from("duplicate")), Err("duplicated-data"));
        assert_eq!(rbuf.write(Seq32::from(22), &Bytes::from("duplicate")), Err("duplicated-data"));

        let s = Bytes::from("dappend new");
        for i in 0..11 {
            assert_eq!(rbuf.peek(), Some(&s[i..]));
            assert_eq!(rbuf.consume(1), Ok(()));
        }

        assert_eq!(rbuf.consume(1), Err("no-enough-data"));
    }

    #[test]
    fn recv_buffer_5gb_read_write_test() {
        let mut rng = rand::thread_rng();
        let mut random_buf = BytesMut::with_capacity(10000);
        for _ in 0..2500 {
            random_buf.put_u32(rng.gen::<u32>());
        }

        let data = random_buf.freeze();
        let mut rbuf = RecvBuffer::with_capacity(13107200);
        let mut test_bytes = 0;
        let mut pos = Seq32::from(0);
        while test_bytes < 5368709120u64 {
            assert_eq!(rbuf.write(pos, &data), Ok(()));
            assert_eq!(rbuf.readable_size(), data.len());

            let mut cmp_offset = 0;
            while rbuf.readable_size() > 0 {
                let left = rbuf.peek().unwrap();
                let len = left.len();
                assert_eq!(left, &data[cmp_offset .. cmp_offset + len]);
                assert_eq!(rbuf.consume(len), Ok(()));
                cmp_offset += len;
            }

            pos += data.len() as u32;
            test_bytes += data.len() as u64;
        }
    }
}