use std::collections::{btree_map, BTreeMap};
use std::ops::Bound::{Included, Excluded, Unbounded};

use crate::common::seq32::Seq32;

#[derive(Debug, Default, Clone)]
pub struct RangeSet(BTreeMap<Seq32, Seq32>);

impl <'a> RangeSet {
    pub fn new() -> Self {
        Default::default()
    }

    pub fn iter(&'_ self) -> btree_map::Iter<'_, Seq32, Seq32> {
        self.0.iter()
    }

    fn pred(&self, pos: Seq32) -> Option<(Seq32, Seq32)> {
        self.0
            .range((Unbounded, Included(pos)))
            .last()
            .map(|(&x, &y)| (x, y))
    }

    fn succ(&self, pos: Seq32) -> Option<(Seq32, Seq32)> {
        self.0
            .range((Excluded(pos), Unbounded))
            .next()
            .map(|(&x, &y)| (x, y))
    }

    pub fn insert(&mut self, mut start: Seq32, mut end: Seq32) -> bool {
        if end <= start {
            return false;
        }
        // extend predecessor range
        if let Some((p_start, p_end)) = self.pred(start) {
            if p_end >= start {
                if p_end >= end {
                    return false;
                } else {
                    self.0.remove(&p_start);
                    start = p_start;
                }
            }
        }
        // merge and remove overlapped successors
        while let Some((s_start, s_end)) = self.succ(start) {
            if s_start > end {
                break;
            }
            self.0.remove(&s_start);
            if end <= s_end {
                end = s_end;
                break;
            }
        }
        // insert current range
        self.0.insert(start, end);
        true
    }

    pub fn remove(&mut self, start: &Seq32) -> Option<Seq32> {
        self.0.remove(start)
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }
}

pub struct RangeSetIter<I: Iterator> {
    iter: I,
}

impl<I: Iterator> Iterator for RangeSetIter<I> {
    type Item = <I as Iterator>::Item;

    fn next(&mut self) -> Option<Self::Item> {
        self.iter.next()
    }
}

mod tests {
    // Note this useful idiom: importing names from outer (for mod tests) scope.
    use super::*;

    #[test]
    fn range_set_basic_test() {
        let mut range_set = RangeSet::new();
        assert_eq!(range_set.insert(Seq32::from(100), Seq32::from(101)), true);
        assert_eq!(range_set.insert(Seq32::from(103), Seq32::from(200)), true);
        assert_eq!(range_set.insert(Seq32::from(100), Seq32::from(101)), false);
        assert_eq!(range_set.insert(Seq32::from(110), Seq32::from(111)), false);
        assert_eq!(range_set.len(), 2);

        assert_eq!(range_set.insert(Seq32::from(4294967290), Seq32::from(4294967293)), true);
        assert_eq!(range_set.iter().next(), Some((&Seq32::from(4294967290), &Seq32::from(4294967293))));
        assert_eq!(range_set.insert(Seq32::from(4294967280), Seq32::from(0)), true);
        assert_eq!(range_set.len(), 3);
        assert_eq!(range_set.insert(Seq32::from(4294967280), Seq32::from(200)), true);
        assert_eq!(range_set.len(), 1);
        assert_eq!(range_set.insert(Seq32::from(0), Seq32::from(4294967280)), false);
    }
}