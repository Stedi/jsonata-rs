use std::{cell::RefCell, collections::HashMap, rc::Rc};

use super::value::Value;
use bumpalo::boxed::Box;
use bumpalo::Bump;

#[derive(Debug)]
pub struct Frame<'a> {
    data: Box<'a, Rc<RefCell<FrameData<'a>>>>,
    arena: &'a Bump,
}

impl<'a> Frame<'a> {
    pub fn new(arena: &'a Bump) -> Frame<'a> {
        Frame {
            data: Box::new_in(
                Rc::new(RefCell::new(FrameData {
                    bindings: HashMap::new(),
                    parent: None,
                })),
                arena,
            ),
            arena,
        }
    }

    pub fn new_with_parent(arena: &'a Bump, parent: &Frame<'a>) -> Frame<'a> {
        Frame {
            data: Box::new_in(
                Rc::new(RefCell::new(FrameData {
                    bindings: HashMap::new(),
                    parent: Some(parent.clone()),
                })),
                arena,
            ),
            arena,
        }
    }

    pub fn from_tuple(arena: &'a Bump, parent: &Frame<'a>, tuple: &'a Value<'a>) -> Frame<'a> {
        let mut bindings = HashMap::with_capacity(tuple.entries().len());
        for (key, value) in tuple.entries() {
            bindings.insert(key.to_string(), *value);
        }

        Frame {
            data: Box::new_in(
                Rc::new(RefCell::new(FrameData {
                    bindings,
                    parent: Some(parent.clone()),
                })),
                arena,
            ),
            arena,
        }
    }

    pub fn bind(&self, name: &str, value: &'a Value<'a>) {
        self.data
            .borrow_mut()
            .bindings
            .insert(name.to_string(), value);
    }

    pub fn lookup(&self, name: &str) -> Option<&'a Value<'a>> {
        match self.data.borrow().bindings.get(name) {
            Some(value) => Some(*value),
            None => match &self.data.borrow().parent {
                Some(parent) => parent.lookup(name),
                None => None,
            },
        }
    }
}

impl<'a> Clone for Frame<'a> {
    fn clone(&self) -> Self {
        // Clone the Rc to maintain sharing semantics, but allocate a new Box in the arena
        Frame {
            data: Box::new_in(self.data.as_ref().clone(), self.arena),
            arena: self.arena,
        }
    }
}

#[derive(Debug)]
pub struct FrameData<'a> {
    bindings: HashMap<String, &'a Value<'a>>,
    parent: Option<Frame<'a>>,
}

// #[cfg(test)]
// mod tests {
//     use super::*;

//     #[test]
//     fn bind() {
//         let frame = Frame::new();
//         frame.bind("a", Value::number(1));
//         let a = frame.lookup("a");
//         assert!(a.is_some());
//         assert_eq!(a.unwrap(), 1);
//     }

//     #[test]
//     fn lookup_through_parent() {
//         let parent = Frame::new();
//         parent.bind("a", &arena.number(1));
//         let frame = Frame::new_with_parent(&parent);
//         let a = frame.lookup("a");
//         assert!(a.is_some());
//         assert_eq!(a.unwrap(), 1);
//     }

//     #[test]
//     fn lookup_overriding_parent() {
//         let parent = Frame::new();
//         let arena = ValueArena::new();
//         parent.bind("a", arena.clone(), &arena.number(1));
//         let frame = Frame::new_with_parent(&parent);
//         frame.bind("a", arena.clone(), &arena.number(2));
//         let a = frame.lookup("a");
//         assert!(a.is_some());
//         assert_eq!(a.unwrap(), 2);
//     }
// }
