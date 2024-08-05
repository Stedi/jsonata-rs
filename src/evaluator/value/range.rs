use std::{
    hash::{Hash, Hasher},
    ops::Index,
};

use bumpalo::Bump;

use super::Value;

#[derive(Debug, Clone)]
pub struct Range<'a> {
    arena: &'a Bump,
    start: isize,
    end: isize,
}

impl<'a> Range<'a> {
    pub fn new(arena: &'a Bump, start: isize, end: isize) -> Self {
        if end < start {
            panic!("Tried to contruct a range with negative length");
        }
        Self { arena, start, end }
    }

    pub fn start(&self) -> isize {
        self.start
    }

    pub fn end(&self) -> isize {
        self.end
    }

    pub fn len(&self) -> usize {
        (self.end - self.start + 1) as usize
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn nth(&self, index: usize) -> Option<&'a Value<'a>> {
        if index < self.len() {
            Some(Value::number(
                self.arena,
                (self.start + index as isize) as f64,
            ))
        } else {
            None
        }
    }
}

impl<'a> Index<usize> for Range<'a> {
    type Output = Value<'a>;

    fn index(&self, index: usize) -> &Self::Output {
        self.nth(index).unwrap_or_else(Value::undefined)
    }
}

impl<'a> PartialEq<Range<'_>> for Range<'a> {
    fn eq(&self, other: &Range<'_>) -> bool {
        self.start == other.start && self.end == other.end
    }
}

impl Hash for Range<'_> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.start.hash(state);
        self.end.hash(state);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bumpalo::Bump;

    #[test]
    fn len() {
        let arena = Bump::new();
        let range = Range::new(&arena, 1, 5);
        assert_eq!(range.start(), 1);
        assert_eq!(range.end(), 5);
        assert_eq!(range.len(), 5);
        assert!(!range.is_empty());
    }

    #[test]
    fn nth() {
        let arena = Bump::new();
        let range = Range::new(&arena, 1, 5);
        assert_eq!(*range.nth(0).unwrap(), 1_isize);
        assert_eq!(*range.nth(1).unwrap(), 2_isize);
        assert_eq!(*range.nth(2).unwrap(), 3_isize);
        assert_eq!(*range.nth(3).unwrap(), 4_isize);
        assert_eq!(*range.nth(4).unwrap(), 5_isize);
        assert!(range.nth(5).is_none());
    }

    #[test]
    fn eq() {
        let arena = Bump::new();
        let range1 = Range::new(&arena, 1, 5);
        let range2 = Range::new(&arena, 1, 5);
        assert_eq!(range1, range2);
    }

    #[test]
    fn ne() {
        let arena = Bump::new();
        let range1 = Range::new(&arena, 2, 5);
        let range2 = Range::new(&arena, 1, 5);
        assert_ne!(range1, range2);
    }

    #[test]
    fn index() {
        let arena = Bump::new();
        let range = Range::new(&arena, 1, 5);
        assert_eq!(range[0], 1_isize);
        assert_eq!(range[1], 2_isize);
        assert_eq!(range[2], 3_isize);
        assert_eq!(range[3], 4_isize);
        assert_eq!(range[4], 5_isize);
        assert!(range[5].is_undefined());
    }

    #[test]
    fn negative() {
        let arena = Bump::new();
        let range = Range::new(&arena, -10, -5);
        assert_eq!(range[0], -10_isize);
        assert_eq!(range[1], -9_isize);
        assert_eq!(range[2], -8_isize);
        assert_eq!(range[3], -7_isize);
        assert_eq!(range[4], -6_isize);
        assert_eq!(range[5], -5_isize);
        assert!(range[6].is_undefined());
    }
}
