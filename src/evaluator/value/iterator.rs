use std::iter::Iterator;

use super::Value;

pub struct MemberIterator<'a> {
    value: &'a Value<'a>,
    front: usize,
    back: usize,
    back_done: bool,
}

impl<'a> MemberIterator<'a> {
    pub fn new(value: &'a Value<'a>) -> Self {
        Self {
            value,
            front: 0,
            back: value.len().saturating_sub(1),
            back_done: false,
        }
    }
}

impl<'a> Iterator for MemberIterator<'a> {
    type Item = &'a Value<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.front < self.value.len() {
            let result = match self.value {
                Value::Array(array, _) => array.get(self.front).copied(),
                Value::Range(range) => range.nth(self.front),
                _ => unreachable!(),
            };
            self.front += 1;
            result
        } else {
            None
        }
    }
}

#[allow(clippy::needless_lifetimes)]
impl<'a> DoubleEndedIterator for MemberIterator<'a> {
    fn next_back(&mut self) -> Option<Self::Item> {
        if self.back_done {
            return None;
        }

        let result = match self.value {
            Value::Array(array, _) => array.get(self.back).copied(),
            Value::Range(range) => range.nth(self.back),
            _ => unreachable!(),
        };

        if self.back == 0 {
            self.back_done = true;
            return result;
        }

        self.back -= 1;
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bumpalo::Bump;

    #[test]
    fn forward() {
        let arena = Bump::new();
        let range = Value::range(&arena, 1, 5);
        let mut iter = MemberIterator::new(range);
        assert_eq!(*iter.next().unwrap(), 1_isize);
        assert_eq!(*iter.next().unwrap(), 2_isize);
        assert_eq!(*iter.next().unwrap(), 3_isize);
        assert_eq!(*iter.next().unwrap(), 4_isize);
        assert_eq!(*iter.next().unwrap(), 5_isize);
        assert!(iter.next().is_none());
    }

    #[test]
    fn backward() {
        let arena = Bump::new();
        let range = Value::range(&arena, 1, 5);
        let mut iter = MemberIterator::new(range);
        assert_eq!(*iter.next_back().unwrap(), 5_isize);
        assert_eq!(*iter.next_back().unwrap(), 4_isize);
        assert_eq!(*iter.next_back().unwrap(), 3_isize);
        assert_eq!(*iter.next_back().unwrap(), 2_isize);
        assert_eq!(*iter.next_back().unwrap(), 1_isize);
        assert!(iter.next_back().is_none());
    }

    #[test]
    fn reverse() {
        let arena = Bump::new();
        let range = Value::range(&arena, 1, 5);
        let mut iter = MemberIterator::new(range).rev();
        assert_eq!(*iter.next().unwrap(), 5_isize);
        assert_eq!(*iter.next().unwrap(), 4_isize);
        assert_eq!(*iter.next().unwrap(), 3_isize);
        assert_eq!(*iter.next().unwrap(), 2_isize);
        assert_eq!(*iter.next().unwrap(), 1_isize);
        assert!(iter.next().is_none());
    }
}
