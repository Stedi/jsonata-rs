use std::{
    hash::{Hash, Hasher},
    ops::Index,
};

use rand::Rng;

use super::Value;

impl<'a> PartialEq<Value<'a>> for Value<'a> {
    fn eq(&self, other: &Value<'a>) -> bool {
        match (self, other) {
            (Value::Undefined, Value::Undefined) => true,
            (Value::Null, Value::Null) => true,
            (Value::Number(l), Value::Number(r)) => *l == *r,
            (Value::Bool(l), Value::Bool(r)) => *l == *r,
            (Value::String(l), Value::String(r)) => *l == *r,
            (Value::Array(l, ..), Value::Array(r, ..)) => *l == *r,
            (Value::Object(l), Value::Object(r)) => *l == *r,
            (Value::Range(l), Value::Range(r)) => *l == *r,
            (Value::Regex(l), Value::Regex(r)) => l == r,
            _ => false,
        }
    }
}

impl PartialEq<bool> for Value<'_> {
    fn eq(&self, other: &bool) -> bool {
        match *self {
            Value::Bool(ref b) => *b == *other,
            _ => false,
        }
    }
}

impl PartialEq<usize> for Value<'_> {
    fn eq(&self, other: &usize) -> bool {
        match self {
            Value::Number(..) => self.as_usize() == *other,
            _ => false,
        }
    }
}

impl PartialEq<isize> for Value<'_> {
    fn eq(&self, other: &isize) -> bool {
        match self {
            Value::Number(..) => self.as_isize() == *other,
            _ => false,
        }
    }
}

impl PartialEq<&str> for Value<'_> {
    fn eq(&self, other: &&str) -> bool {
        match self {
            Value::String(ref s) => s == *other,
            _ => false,
        }
    }
}

impl<'a> Index<&str> for Value<'a> {
    type Output = Value<'a>;

    fn index(&self, index: &str) -> &Self::Output {
        match *self {
            Value::Object(..) => self.get_entry(index),
            _ => Value::undefined(),
        }
    }
}

impl<'a> Index<usize> for Value<'a> {
    type Output = Value<'a>;

    fn index(&self, index: usize) -> &Self::Output {
        match *self {
            Value::Array(..) | Value::Range(..) => self.get_member(index),
            _ => Value::undefined(),
        }
    }
}

impl std::fmt::Debug for Value<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Undefined => write!(f, "undefined"),
            Self::Null => write!(f, "null"),
            Self::Number(n) => n.fmt(f),
            Self::Bool(b) => b.fmt(f),
            Self::String(s) => s.fmt(f),
            Self::Array(a, _) => a.fmt(f),
            Self::Object(o) => o.fmt(f),
            Self::Regex(r) => write!(f, "<regex({:?})>", r),
            Self::Lambda { .. } => write!(f, "<lambda>"),
            Self::NativeFn { .. } => write!(f, "<nativefn>"),
            Self::Transformer { .. } => write!(f, "<transformer>"),
            Self::Range(r) => write!(f, "<range({},{})>", r.start(), r.end()),
        }
    }
}

impl std::fmt::Display for Value<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Regex(r) => write!(f, "<regex({:?})>", r),
            _ => write!(f, "{:#?}", self),
        }
    }
}

impl Hash for Value<'_> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        match self {
            Value::Undefined => 0.hash(state),
            Value::Null => 1.hash(state),
            Value::Number(n) => n.to_bits().hash(state),
            Value::Bool(b) => b.hash(state),
            Value::String(s) => s.hash(state),
            Value::Array(a, _) => a.hash(state),
            Value::Object(map) => {
                let mut keys_sorted = map.keys().collect::<Vec<_>>();
                keys_sorted.sort();

                for key in keys_sorted {
                    key.hash(state);
                    map.get(key).hash(state);
                }
            }
            Value::Regex(r) => r.hash(state),
            Value::Range(r) => r.hash(state),
            Value::Lambda { .. } => generate_random_hash(state),
            Value::NativeFn { name, .. } => name.hash(state),
            Value::Transformer { .. } => generate_random_hash(state),
        }
    }
}

impl Eq for Value<'_> {}

fn generate_random_hash<H: Hasher>(state: &mut H) {
    let random_number: u64 = rand::thread_rng().gen();
    random_number.hash(state);
}
