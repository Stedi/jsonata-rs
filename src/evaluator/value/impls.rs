use std::ops::Index;

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
            Self::Lambda { .. } => write!(f, "<lambda>"),
            Self::NativeFn { .. } => write!(f, "<nativefn>"),
            Self::Transformer { .. } => write!(f, "<transformer>"),
            Self::Range(r) => write!(f, "<range({},{})>", r.start(), r.end()),
        }
    }
}

impl std::fmt::Display for Value<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:#?}", self)
    }
}
