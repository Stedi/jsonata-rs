use std::{cell::RefCell, collections::HashMap, rc::Rc};

use super::value::Value;

#[derive(Debug)]
pub struct Frame<'a>(Rc<RefCell<FrameData<'a>>>);

impl<'a> Frame<'a> {
    pub fn new() -> Frame<'a> {
        Frame(Rc::new(RefCell::new(FrameData {
            bindings: HashMap::new(),
            parent: None,
        })))
    }

    pub fn new_with_parent(parent: &Frame<'a>) -> Frame<'a> {
        Frame(Rc::new(RefCell::new(FrameData {
            bindings: HashMap::new(),
            parent: Some(parent.clone()),
        })))
    }

    pub fn from_tuple(parent: &Frame<'a>, tuple: &'a Value<'a>) -> Frame<'a> {
        let mut bindings = HashMap::with_capacity(tuple.entries().len());
        for (key, value) in tuple.entries() {
            bindings.insert(key.to_string(), *value);
        }

        Frame(Rc::new(RefCell::new(FrameData {
            bindings,
            parent: Some(parent.clone()),
        })))
    }

    pub fn bind(&self, name: &str, value: &'a Value<'a>) {
        self.0.borrow_mut().bindings.insert(name.to_string(), value);
    }

    pub fn lookup(&self, name: &str) -> Option<&'a Value<'a>> {
        match self.0.borrow().bindings.get(name) {
            Some(value) => Some(*value),
            None => match &self.0.borrow().parent {
                Some(parent) => parent.lookup(name),
                None => None,
            },
        }
    }
}

impl Default for Frame<'_> {
    fn default() -> Self {
        Self::new()
    }
}

impl Clone for Frame<'_> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
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
