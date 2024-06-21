use std::borrow::Cow;
use std::collections::{hash_map, HashMap};

use bitflags::bitflags;
use bumpalo::boxed::Box;
use bumpalo::Bump;

use super::frame::Frame;
use super::functions::FunctionContext;
use crate::parser::ast::{Ast, AstKind};
use crate::{Error, Result};

pub mod impls;
pub mod iterator;
mod range;
pub mod serialize;

use self::range::Range;
use self::serialize::{DumpFormatter, PrettyFormatter, Serializer};
pub use iterator::MemberIterator;

bitflags! {
    #[derive(Clone)]
    pub struct ArrayFlags: u8 {
        const SEQUENCE     = 0b00000001;
        const SINGLETON    = 0b00000010;
        const CONS         = 0b00000100;
        const WRAPPED      = 0b00001000;
        const TUPLE_STREAM = 0b00010000;
    }
}

pub const UNDEFINED: Value = Value::Undefined;

/// The core value type for input, output and evaluation. There's a lot of lifetimes here to avoid
/// cloning any part of the input that should be kept in the output, avoiding heap allocations for
/// every Value, and allowing structural sharing.
///
/// Values are all allocated in a Bump arena, making them contiguous in memory and further avoiding
/// heap allocations for every one.
pub enum Value<'a> {
    Undefined,
    Null,
    Number(f64),
    Bool(bool),
    String(bumpalo::collections::String<'a>),
    Array(Box<'a, Vec<&'a Value<'a>>>, ArrayFlags),
    Object(Box<'a, HashMap<String, &'a Value<'a>>>),
    Range(Range<'a>),
    Lambda {
        ast: Box<'a, Ast>,
        input: &'a Value<'a>,
        frame: Frame<'a>,
    },
    NativeFn {
        name: String,
        arity: usize,
        func: fn(FunctionContext<'a, '_>, &'a Value<'a>) -> Result<&'a Value<'a>>,
    },
    Transformer {
        pattern: std::boxed::Box<Ast>,
        update: std::boxed::Box<Ast>,
        delete: Option<std::boxed::Box<Ast>>,
    },
}

#[allow(clippy::mut_from_ref)]
impl<'a> Value<'a> {
    pub fn undefined() -> &'a Value<'a> {
        // SAFETY: The UNDEFINED const is Value<'static>, it doesn't reference any other Values,
        // and there's no Drop implementation, so there shouldn't be an issue casting it to Value<'a>.
        unsafe { std::mem::transmute::<&Value<'static>, &'a Value<'a>>(&UNDEFINED) }
    }

    pub fn null(arena: &Bump) -> &mut Value {
        arena.alloc(Value::Null)
    }

    pub fn bool(arena: &Bump, value: bool) -> &mut Value {
        arena.alloc(Value::Bool(value))
    }

    pub fn number(arena: &Bump, value: impl Into<f64>) -> &mut Value {
        arena.alloc(Value::Number(value.into()))
    }

    pub fn string(arena: &'a Bump, value: &str) -> &'a mut Value<'a> {
        arena.alloc(Value::String(bumpalo::collections::String::from_str_in(
            value, arena,
        )))
    }

    pub fn array(arena: &Bump, flags: ArrayFlags) -> &mut Value {
        arena.alloc(Value::Array(Box::new_in(Vec::new(), arena), flags))
    }

    pub fn array_from(
        arr: &'a [&Value<'a>],
        arena: &'a Bump,
        flags: ArrayFlags,
    ) -> &'a mut Value<'a> {
        let result = Value::array_with_capacity(arena, arr.len(), flags);
        if let Value::Array(a, _) = result {
            a.extend(arr);
        }
        result
    }

    pub fn array_with_capacity(arena: &Bump, capacity: usize, flags: ArrayFlags) -> &mut Value {
        arena.alloc(Value::Array(
            Box::new_in(Vec::with_capacity(capacity), arena),
            flags,
        ))
    }

    pub fn object(arena: &Bump) -> &mut Value {
        arena.alloc(Value::Object(Box::new_in(HashMap::new(), arena)))
    }

    pub fn object_from(
        hash: &HashMap<String, &'a Value<'a>>,
        arena: &'a Bump,
    ) -> &'a mut Value<'a> {
        let result = Value::object_with_capacity(arena, hash.len());
        if let Value::Object(o) = result {
            o.extend(hash.iter().map(|(k, v)| (k.clone(), *v)));
        }
        result
    }

    pub fn object_with_capacity(arena: &Bump, capacity: usize) -> &mut Value {
        arena.alloc(Value::Object(Box::new_in(
            HashMap::with_capacity(capacity),
            arena,
        )))
    }

    pub fn lambda(
        arena: &'a Bump,
        node: &Ast,
        input: &'a Value<'a>,
        frame: Frame<'a>,
    ) -> &'a mut Value<'a> {
        arena.alloc(Value::Lambda {
            ast: Box::new_in(node.clone(), arena),
            input,
            frame,
        })
    }

    pub fn nativefn(
        arena: &'a Bump,
        name: &str,
        arity: usize,
        func: fn(FunctionContext<'a, '_>, &'a Value<'a>) -> Result<&'a Value<'a>>,
    ) -> &'a mut Value<'a> {
        arena.alloc(Value::NativeFn {
            name: name.to_string(),
            arity,
            func,
        })
    }

    pub fn transformer(
        arena: &'a Bump,
        pattern: &std::boxed::Box<Ast>,
        update: &std::boxed::Box<Ast>,
        delete: &Option<std::boxed::Box<Ast>>,
    ) -> &'a mut Value<'a> {
        arena.alloc(Value::Transformer {
            pattern: pattern.clone(),
            update: update.clone(),
            delete: delete.clone(),
        })
    }

    pub fn range(arena: &'a Bump, start: isize, end: isize) -> &'a mut Value<'a> {
        arena.alloc(Value::Range(Range::new(arena, start, end)))
    }

    pub fn range_from(arena: &'a Bump, range: &'a Range) -> &'a mut Value<'a> {
        arena.alloc(Value::Range(range.clone()))
    }

    pub fn is_undefined(&self) -> bool {
        matches!(*self, Value::Undefined)
    }

    pub fn is_null(&self) -> bool {
        matches!(*self, Value::Null)
    }

    pub fn is_bool(&self) -> bool {
        matches!(&self, Value::Bool(..))
    }

    pub fn is_number(&self) -> bool {
        matches!(&self, Value::Number(..))
    }

    pub fn is_integer(&self) -> bool {
        match self {
            Value::Number(n) => match n.classify() {
                std::num::FpCategory::Nan
                | std::num::FpCategory::Infinite
                | std::num::FpCategory::Subnormal => false,
                _ => {
                    let mantissa = n.trunc();
                    n - mantissa == 0.0
                }
            },
            _ => false,
        }
    }

    pub fn is_array_of_valid_numbers(&self) -> Result<bool> {
        match self {
            Value::Array(ref a, _) => {
                for member in a.iter() {
                    if !member.is_valid_number()? {
                        return Ok(false);
                    }
                }
                Ok(true)
            }
            _ => Ok(false),
        }
    }

    pub fn is_array_of_strings(&self) -> bool {
        match self {
            Value::Array(ref a, _) => {
                for member in a.iter() {
                    if !member.is_string() {
                        return false;
                    }
                }
                true
            }
            _ => false,
        }
    }

    pub fn is_valid_number(&self) -> Result<bool> {
        match self {
            Value::Number(n) => {
                if n.is_nan() {
                    Ok(false)
                } else if n.is_infinite() {
                    Err(Error::D1001NumberOfOutRange(*n))
                } else {
                    Ok(true)
                }
            }
            _ => Ok(false),
        }
    }

    pub fn is_nan(&self) -> bool {
        matches!(*self, Value::Number(n) if n.is_nan())
    }

    pub fn is_finite(&self) -> bool {
        match self {
            Value::Number(n) => n.is_finite(),
            _ => false,
        }
    }

    pub fn is_string(&self) -> bool {
        matches!(*self, Value::String(..))
    }

    pub fn is_array(&self) -> bool {
        matches!(*self, Value::Array(..) | Value::Range(..))
    }

    pub fn is_object(&self) -> bool {
        matches!(*self, Value::Object(..))
    }

    pub fn is_function(&self) -> bool {
        matches!(
            *self,
            Value::Lambda { .. } | Value::NativeFn { .. } | Value::Transformer { .. }
        )
    }

    pub fn is_truthy(&'a self) -> bool {
        match *self {
            Value::Undefined => false,
            Value::Null => false,
            Value::Number(n) => n != 0.0,
            Value::Bool(ref b) => *b,
            Value::String(ref s) => !s.is_empty(),
            Value::Array(ref a, _) => match a.len() {
                0 => false,
                1 => self.get_member(0).is_truthy(),
                _ => {
                    for item in self.members() {
                        if item.is_truthy() {
                            return true;
                        }
                    }
                    false
                }
            },
            Value::Object(ref o) => !o.is_empty(),
            Value::Lambda { .. } | Value::NativeFn { .. } | Value::Transformer { .. } => false,
            Value::Range(ref r) => !r.is_empty(),
        }
    }

    pub fn get_member(&self, index: usize) -> &'a Value<'a> {
        match *self {
            Value::Array(ref array, _) => {
                array.get(index).copied().unwrap_or_else(Value::undefined)
            }
            Value::Range(ref range) => range.nth(index).unwrap_or_else(Value::undefined),
            _ => panic!("Not an array"),
        }
    }

    pub fn members(&'a self) -> MemberIterator<'a> {
        match self {
            Value::Array(..) | Value::Range(..) => MemberIterator::new(self),
            _ => panic!("Not an array"),
        }
    }

    pub fn entries(&self) -> hash_map::Iter<'_, String, &'a Value> {
        match self {
            Value::Object(map) => map.iter(),
            _ => panic!("Not an object"),
        }
    }

    pub fn arity(&self) -> usize {
        match *self {
            Value::Lambda { ref ast, .. } => {
                if let AstKind::Lambda { ref args, .. } = ast.kind {
                    args.len()
                } else {
                    panic!("Not a lambda function")
                }
            }
            Value::NativeFn { arity, .. } => arity,
            Value::Transformer { .. } => 1,
            _ => panic!("Not a function"),
        }
    }

    pub fn as_bool(&self) -> bool {
        match *self {
            Value::Bool(ref b) => *b,
            _ => panic!("Not a bool"),
        }
    }

    pub fn as_f64(&self) -> f64 {
        match *self {
            Value::Number(n) => n,
            _ => panic!("Not a number"),
        }
    }

    // TODO(math): Completely unchecked, audit usage
    pub fn as_usize(&self) -> usize {
        match *self {
            Value::Number(n) => n as usize,
            _ => panic!("Not a number"),
        }
    }

    // TODO(math): Completely unchecked, audit usage
    pub fn as_isize(&self) -> isize {
        match *self {
            Value::Number(n) => n as isize,
            _ => panic!("Not a number"),
        }
    }

    pub fn as_str(&self) -> Cow<'_, str> {
        match *self {
            Value::String(ref s) => Cow::from(s.as_str()),
            _ => panic!("Not a string"),
        }
    }

    pub fn len(&self) -> usize {
        match *self {
            Value::Array(ref array, _) => array.len(),
            Value::Range(ref range) => range.len(),
            _ => panic!("Not an array"),
        }
    }

    pub fn is_empty(&self) -> bool {
        match *self {
            Value::Array(ref array, _) => array.is_empty(),
            Value::Range(ref range) => range.is_empty(),
            _ => panic!("Not an array"),
        }
    }

    pub fn get_entry(&self, key: &str) -> &'a Value<'a> {
        match *self {
            Value::Object(ref map) => match map.get(key) {
                Some(value) => value,
                None => Value::undefined(),
            },
            _ => panic!("Not an object"),
        }
    }

    pub fn remove_entry(&mut self, key: &str) {
        match *self {
            Value::Object(ref mut map) => map.remove(key),
            _ => panic!("Not an object"),
        };
    }

    pub fn push(&mut self, value: &'a Value<'a>) {
        match *self {
            Value::Array(ref mut array, _) => array.push(value),
            _ => panic!("Not an array"),
        }
    }

    pub fn insert(&mut self, key: &str, value: &'a Value<'a>) {
        match *self {
            Value::Object(ref mut map) => {
                map.insert(key.to_owned(), value);
            }
            _ => panic!("Not an object"),
        }
    }

    pub fn remove(&mut self, key: &str) {
        match *self {
            Value::Object(ref mut map) => map.remove(key),
            _ => panic!("Not an object"),
        };
    }

    pub fn flatten(&'a self, arena: &'a Bump) -> &'a mut Value<'a> {
        let flattened = Self::array(arena, ArrayFlags::empty());
        self._flatten(flattened)
    }

    fn _flatten(&'a self, flattened: &'a mut Value<'a>) -> &'a mut Value<'a> {
        let mut flattened = flattened;

        if self.is_array() {
            for member in self.members() {
                flattened = member._flatten(flattened);
            }
        } else {
            flattened.push(self)
        }

        flattened
    }

    pub fn wrap_in_array(
        arena: &'a Bump,
        value: &'a Value<'a>,
        flags: ArrayFlags,
    ) -> &'a mut Value<'a> {
        arena.alloc(Value::Array(Box::new_in(vec![value], arena), flags))
    }

    pub fn wrap_in_array_if_needed(
        arena: &'a Bump,
        value: &'a Value<'a>,
        flags: ArrayFlags,
    ) -> &'a Value<'a> {
        if value.is_array() {
            value
        } else {
            Value::wrap_in_array(arena, value, flags)
        }
    }

    pub fn get_flags(&self) -> ArrayFlags {
        match self {
            Value::Array(_, flags) => flags.clone(),
            _ => panic!("Not an array"),
        }
    }

    pub fn has_flags(&self, check_flags: ArrayFlags) -> bool {
        match self {
            Value::Array(_, flags) => flags.contains(check_flags),
            _ => false,
        }
    }

    pub fn clone(&'a self, arena: &'a Bump) -> &'a mut Value<'a> {
        match self {
            Self::Undefined => arena.alloc(Value::Undefined),
            Self::Null => Value::null(arena),
            Self::Number(n) => Value::number(arena, *n),
            Self::Bool(b) => Value::bool(arena, *b),
            Self::String(s) => arena.alloc(Value::String(
                bumpalo::collections::String::from_str_in(s.as_str(), arena),
            )),
            // Self::String(s) => Value::string(arena, s),
            Self::Array(a, f) => Value::array_from(a, arena, f.clone()),
            Self::Object(o) => Value::object_from(o, arena),
            Self::Lambda { ast, input, frame } => Value::lambda(arena, ast, input, frame.clone()),
            Self::NativeFn { name, arity, func } => Value::nativefn(arena, name, *arity, *func),
            Self::Transformer {
                pattern,
                update,
                delete,
            } => Value::transformer(arena, pattern, update, delete),
            Self::Range(range) => Value::range_from(arena, range),
        }
    }

    pub fn clone_array_with_flags(&self, arena: &'a Bump, flags: ArrayFlags) -> &'a mut Value<'a> {
        match *self {
            Value::Array(ref array, _) => arena.alloc(Value::Array(
                Box::new_in(array.as_ref().clone(), arena),
                flags,
            )),
            _ => panic!("Not an array"),
        }
    }

    pub fn serialize(&'a self, pretty: bool) -> String {
        if pretty {
            let serializer = Serializer::new(PrettyFormatter::default(), false);
            serializer.serialize(self).expect("Shouldn't fail")
        } else {
            let serializer = Serializer::new(DumpFormatter, false);
            serializer.serialize(self).expect("Shouldn't fail")
        }
    }

    // TODO: I don't have a good way to make modifications to values right now, so here's this absolutely
    // no good, very bad, shouldn't exist reference transmuter :(
    //
    // This only exists for object transfomers, which specifically reach into existing values to make
    // changes by updating and removing keys.
    //
    // Need to think up another way, but the whole evaluation pipeline is based on the immutability of Value,
    // so something needs to give.
    #[allow(invalid_reference_casting)]
    pub fn __very_unsafe_make_mut(&'a self) -> &'a mut Value<'a> {
        unsafe {
            let const_ptr = self as *const Value<'a>;
            let mut_ptr = const_ptr as *mut Value<'a>;
            &mut *mut_ptr
        }
    }
}
