use base64::Engine;
use chrono::{TimeZone, Utc};
use hashbrown::{DefaultHashBuilder, HashMap};
use rand::Rng;
use serde_json::Value as JsonValue;
use std::borrow::{Borrow, Cow};
use std::collections::HashSet;
use std::time::{SystemTime, UNIX_EPOCH};
use uuid::Uuid;

use crate::datetime::{format_custom_date, parse_custom_format, parse_timezone_offset};
use crate::evaluator::RegexLiteral;
use crate::parser::expressions::check_balanced_brackets;

use bumpalo::collections::CollectIn;
use bumpalo::collections::String as BumpString;
use bumpalo::collections::Vec as BumpVec;
use bumpalo::Bump;

use crate::{Error, Result};

use super::frame::Frame;
use super::value::serialize::{DumpFormatter, PrettyFormatter, Serializer};
use super::value::{ArrayFlags, Value};
use super::Evaluator;

macro_rules! min_args {
    ($context:ident, $args:ident, $min:literal) => {
        if $args.len() < $min {
            return Err(Error::T0410ArgumentNotValid(
                $context.char_index,
                $min,
                $context.name.to_string(),
            ));
        }
    };
}

macro_rules! max_args {
    ($context:ident, $args:ident, $max:literal) => {
        if $args.len() > $max {
            return Err(Error::T0410ArgumentNotValid(
                $context.char_index,
                $max,
                $context.name.to_string(),
            ));
        }
    };
}

macro_rules! bad_arg {
    ($context:ident, $index:literal) => {
        return Err(Error::T0410ArgumentNotValid(
            $context.char_index,
            $index,
            $context.name.to_string(),
        ))
    };
}

macro_rules! assert_arg {
    ($condition: expr, $context:ident, $index:literal) => {
        if !($condition) {
            bad_arg!($context, $index);
        }
    };
}

macro_rules! assert_array_of_type {
    ($condition:expr, $context:ident, $index:literal, $t:literal) => {
        if !($condition) {
            return Err(Error::T0412ArgumentMustBeArrayOfType(
                $context.char_index,
                $index,
                $context.name.to_string(),
                $t.to_string(),
            ));
        };
    };
}

#[derive(Clone)]
pub struct FunctionContext<'a, 'e> {
    pub name: &'a str,
    pub char_index: usize,
    pub input: &'a Value<'a>,
    pub frame: Frame<'a>,
    pub evaluator: &'e Evaluator<'a>,
    pub arena: &'a Bump,
}

impl<'a, 'e> FunctionContext<'a, 'e> {
    pub fn evaluate_function(
        &self,
        proc: &'a Value<'a>,
        args: &[&'a Value<'a>],
    ) -> Result<&'a Value<'a>> {
        self.evaluator
            .apply_function(self.char_index, self.input, proc, args, &self.frame)
    }
}

/// Extend the given values with value.
///
/// If the value is a single value then, append the value as is.
/// If the value is an array, extends values with the value's members.
pub fn fn_append_internal<'a>(values: &mut BumpVec<&'a Value<'a>>, value: &'a Value<'a>) {
    if value.is_undefined() {
        return;
    }

    match value {
        Value::Array(a, _) => values.extend_from_slice(a),
        Value::Range(_) => values.extend(value.members()),
        _ => values.push(value),
    }
}

pub fn fn_append<'a>(
    context: FunctionContext<'a, '_>,
    args: &[&'a Value<'a>],
) -> Result<&'a Value<'a>> {
    let arg1 = args.first().copied().unwrap_or_else(Value::undefined);
    let arg2 = args.get(1).copied().unwrap_or_else(Value::undefined);

    if arg1.is_undefined() {
        return Ok(arg2);
    }

    if arg2.is_undefined() {
        return Ok(arg1);
    }

    let arg1_len = if arg1.is_array() { arg1.len() } else { 1 };
    let arg2_len = if arg2.is_array() { arg2.len() } else { 1 };

    let result = Value::array_with_capacity(
        context.arena,
        arg1_len + arg2_len,
        if arg1.is_array() {
            arg1.get_flags()
        } else {
            ArrayFlags::SEQUENCE
        },
    );

    if arg1.is_array() {
        arg1.members().for_each(|m| result.push(m));
    } else {
        result.push(arg1);
    }

    if arg2.is_array() {
        arg2.members().for_each(|m| result.push(m));
    } else {
        result.push(arg2)
    }

    Ok(result)
}

pub fn fn_boolean<'a>(
    context: FunctionContext<'a, '_>,
    args: &[&'a Value<'a>],
) -> Result<&'a Value<'a>> {
    max_args!(context, args, 1);

    let arg = args.first().copied().unwrap_or_else(Value::undefined);
    Ok(match arg {
        Value::Undefined => Value::undefined(),
        Value::Null => Value::bool(false),
        Value::Bool(b) => Value::bool(*b),
        Value::Number(n) => {
            arg.is_valid_number()?;
            Value::bool(*n != 0.0)
        }
        Value::String(ref str) => Value::bool(!str.is_empty()),
        Value::Object(ref obj) => Value::bool(!obj.is_empty()),
        Value::Array { .. } => match arg.len() {
            0 => Value::bool(false),
            1 => fn_boolean(context.clone(), &[arg.get_member(0)])?,
            _ => {
                for item in arg.members() {
                    if fn_boolean(context.clone(), &[item])?.as_bool() {
                        return Ok(Value::bool(true));
                    }
                }
                Value::bool(false)
            }
        },
        Value::Regex(_) => Value::bool(true),
        Value::Lambda { .. } | Value::NativeFn { .. } | Value::Transformer { .. } => {
            Value::bool(false)
        }
        Value::Range(ref range) => Value::bool(!range.is_empty()),
    })
}

pub fn fn_map<'a>(
    context: FunctionContext<'a, '_>,
    args: &[&'a Value<'a>],
) -> Result<&'a Value<'a>> {
    let arr = args.first().copied().unwrap_or_else(Value::undefined);
    let func = args.get(1).copied().unwrap_or_else(Value::undefined);

    if arr.is_undefined() {
        return Ok(Value::undefined());
    }

    let arr = Value::wrap_in_array_if_needed(context.arena, arr, ArrayFlags::empty());

    assert_arg!(func.is_function(), context, 2);

    let result = Value::array(context.arena, ArrayFlags::SEQUENCE);

    for (index, item) in arr.members().enumerate() {
        let mut args = Vec::new();
        let arity = func.arity();

        args.push(item);
        if arity >= 2 {
            args.push(Value::number(context.arena, index as f64));
        }
        if arity >= 3 {
            args.push(arr);
        }

        let mapped = context.evaluate_function(func, &args)?;
        if !mapped.is_undefined() {
            result.push(mapped);
        }
    }

    Ok(result)
}

pub fn fn_filter<'a>(
    context: FunctionContext<'a, '_>,
    args: &[&'a Value<'a>],
) -> Result<&'a Value<'a>> {
    let arr = args.first().copied().unwrap_or_else(Value::undefined);
    let func = args.get(1).copied().unwrap_or_else(Value::undefined);

    if arr.is_undefined() {
        return Ok(Value::undefined());
    }

    let arr = Value::wrap_in_array_if_needed(context.arena, arr, ArrayFlags::empty());

    assert_arg!(func.is_function(), context, 2);

    let result = Value::array(context.arena, ArrayFlags::SEQUENCE);

    for (index, item) in arr.members().enumerate() {
        let mut args = Vec::new();
        let arity = func.arity();

        args.push(item);
        if arity >= 2 {
            args.push(Value::number(context.arena, index as f64));
        }
        if arity >= 3 {
            args.push(arr);
        }

        let include = context.evaluate_function(func, &args)?;

        if include.is_truthy() {
            result.push(item);
        }
    }

    Ok(result)
}

pub fn fn_each<'a>(
    context: FunctionContext<'a, '_>,
    args: &[&'a Value<'a>],
) -> Result<&'a Value<'a>> {
    let (obj, func) = if args.len() == 1 {
        let obj_arg = if context.input.is_array() && context.input.has_flags(ArrayFlags::WRAPPED) {
            &context.input[0]
        } else {
            context.input
        };

        (obj_arg, args[0])
    } else {
        (args[0], args[1])
    };

    if obj.is_undefined() {
        return Ok(Value::undefined());
    }

    assert_arg!(obj.is_object(), context, 1);
    assert_arg!(func.is_function(), context, 2);

    let result = Value::array(context.arena, ArrayFlags::SEQUENCE);

    for (key, value) in obj.entries() {
        let key = Value::string(context.arena, key);

        let mapped = context.evaluate_function(func, &[value, key])?;
        if !mapped.is_undefined() {
            result.push(mapped);
        }
    }

    Ok(result)
}

pub fn fn_keys<'a>(
    context: FunctionContext<'a, '_>,
    args: &[&'a Value<'a>],
) -> Result<&'a Value<'a>> {
    let obj = if args.is_empty() {
        if context.input.is_array() && context.input.has_flags(ArrayFlags::WRAPPED) {
            &context.input[0]
        } else {
            context.input
        }
    } else {
        args[0]
    };

    if obj.is_undefined() {
        return Ok(Value::undefined());
    }

    let mut keys = Vec::new();

    if obj.is_array() && obj.members().all(|member| member.is_object()) {
        for sub_object in obj.members() {
            for (key, _) in sub_object.entries() {
                // deduplicating keys from multiple objects
                if !keys.iter().any(|item| item == key) {
                    keys.push(key.to_string());
                }
            }
        }
    } else if obj.is_object() {
        for (key, _) in obj.entries() {
            keys.push(key.to_string());
        }
    }

    let result = Value::array(context.arena, ArrayFlags::SEQUENCE);
    for key in keys {
        result.push(Value::string(context.arena, &key));
    }

    Ok(result)
}

pub fn fn_merge<'a>(
    context: FunctionContext<'a, '_>,
    args: &[&'a Value<'a>],
) -> Result<&'a Value<'a>> {
    let mut array_of_objects = if args.is_empty() {
        if context.input.is_array() && context.input.has_flags(ArrayFlags::WRAPPED) {
            &context.input[0]
        } else {
            context.input
        }
    } else {
        args[0]
    };

    if array_of_objects.is_undefined() {
        return Ok(Value::undefined());
    }

    if array_of_objects.is_object() {
        array_of_objects =
            Value::wrap_in_array(context.arena, array_of_objects, ArrayFlags::empty());
    }

    assert_arg!(
        array_of_objects.is_array() && array_of_objects.members().all(|member| member.is_object()),
        context,
        1
    );

    let result = Value::object(context.arena);

    for obj in array_of_objects.members() {
        for (key, value) in obj.entries() {
            result.insert(key, value);
        }
    }

    Ok(result)
}

pub fn fn_string<'a>(
    context: FunctionContext<'a, '_>,
    args: &[&'a Value<'a>],
) -> Result<&'a Value<'a>> {
    max_args!(context, args, 2);

    let input = if args.is_empty() {
        if context.input.is_array() && context.input.has_flags(ArrayFlags::WRAPPED) {
            &context.input[0]
        } else {
            context.input
        }
    } else {
        args.first().copied().unwrap_or_else(Value::undefined)
    };

    if input.is_undefined() {
        return Ok(Value::undefined());
    }

    let pretty = args.get(1).copied().unwrap_or_else(Value::undefined);
    assert_arg!(pretty.is_undefined() || pretty.is_bool(), context, 2);

    if input.is_string() {
        Ok(input)
    } else if input.is_function() {
        Ok(Value::string(context.arena, ""))
    } else if input.is_number() && !input.is_finite() {
        Err(Error::D3001StringNotFinite(context.char_index))
    } else if *pretty == true {
        let serializer = Serializer::new(PrettyFormatter::default(), true);
        let output = serializer.serialize(input)?;
        Ok(Value::string(context.arena, &output))
    } else {
        let serializer = Serializer::new(DumpFormatter, true);
        let output = serializer.serialize(input)?;
        Ok(Value::string(context.arena, &output))
    }
}

pub fn fn_substring_before<'a>(
    context: FunctionContext<'a, '_>,
    args: &[&'a Value<'a>],
) -> Result<&'a Value<'a>> {
    let string = args.first().copied().unwrap_or_else(Value::undefined);

    let chars = args.get(1).copied().unwrap_or_else(Value::undefined);

    if !string.is_string() {
        return Ok(Value::undefined());
    }

    if !chars.is_string() {
        return Err(Error::D3010EmptyPattern(context.char_index));
    }

    let string: &str = &string.as_str();
    let chars: &str = &chars.as_str();

    if let Some(index) = string.find(chars) {
        Ok(Value::string(context.arena, &string[..index]))
    } else {
        Ok(Value::string(context.arena, string))
    }
}

pub fn fn_substring_after<'a>(
    context: FunctionContext<'a, '_>,
    args: &[&'a Value<'a>],
) -> Result<&'a Value<'a>> {
    let string = args.first().copied().unwrap_or_else(Value::undefined);

    let chars = args.get(1).copied().unwrap_or_else(Value::undefined);

    if !string.is_string() {
        return Ok(Value::undefined());
    }

    if !chars.is_string() {
        return Err(Error::D3010EmptyPattern(context.char_index));
    }

    let string: &str = &string.as_str();
    let chars: &str = &chars.as_str();

    if let Some(index) = string.find(chars) {
        let after_index = index + chars.len();
        Ok(Value::string(context.arena, &string[after_index..]))
    } else {
        // Return the original string if 'chars' is not found
        Ok(Value::string(context.arena, string))
    }
}

pub fn fn_not<'a>(
    _context: FunctionContext<'a, '_>,
    args: &[&'a Value<'a>],
) -> Result<&'a Value<'a>> {
    let arg = args.first().copied().unwrap_or_else(Value::undefined);

    Ok(if arg.is_undefined() {
        Value::undefined()
    } else {
        Value::bool(!arg.is_truthy())
    })
}

pub fn fn_lowercase<'a>(
    context: FunctionContext<'a, '_>,
    args: &[&'a Value<'a>],
) -> Result<&'a Value<'a>> {
    let arg = args.first().copied().unwrap_or_else(Value::undefined);

    Ok(if !arg.is_string() {
        Value::undefined()
    } else {
        Value::string(context.arena, &arg.as_str().to_lowercase())
    })
}

pub fn fn_uppercase<'a>(
    context: FunctionContext<'a, '_>,
    args: &[&'a Value<'a>],
) -> Result<&'a Value<'a>> {
    let arg = args.first().copied().unwrap_or_else(Value::undefined);

    if !arg.is_string() {
        Ok(Value::undefined())
    } else {
        Ok(Value::string(context.arena, &arg.as_str().to_uppercase()))
    }
}

pub fn fn_trim<'a>(
    context: FunctionContext<'a, '_>,
    args: &[&'a Value<'a>],
) -> Result<&'a Value<'a>> {
    let arg = args.first().copied().unwrap_or_else(Value::undefined);

    if !arg.is_string() {
        Ok(Value::undefined())
    } else {
        let orginal = arg.as_str();
        let mut words = orginal.split_whitespace();
        let trimmed = match words.next() {
            None => String::new(),
            Some(first_word) => {
                // estimate lower bound of capacity needed
                let (lower, _) = words.size_hint();
                let mut result = String::with_capacity(lower);
                result.push_str(first_word);
                for word in words {
                    result.push(' ');
                    result.push_str(word);
                }
                result
            }
        };
        Ok(Value::string(context.arena, &trimmed))
    }
}

pub fn fn_substring<'a>(
    context: FunctionContext<'a, '_>,
    args: &[&'a Value<'a>],
) -> Result<&'a Value<'a>> {
    let string = args.first().copied().unwrap_or_else(Value::undefined);
    let start = args.get(1).copied().unwrap_or_else(Value::undefined);
    let length = args.get(2).copied().unwrap_or_else(Value::undefined);

    if string.is_undefined() {
        return Ok(Value::undefined());
    }

    assert_arg!(string.is_string(), context, 1);
    assert_arg!(start.is_number(), context, 2);

    let string = string.as_str();

    // Scan the string chars for the actual number of characters.
    // NOTE: Chars are not grapheme clusters, so for some inputs like "नमस्ते" we will get 6
    //       as it will include the diacritics.
    //       See: https://doc.rust-lang.org/nightly/book/ch08-02-strings.html
    let len = string.chars().count() as isize;
    let mut start = start.as_isize();

    // If start is negative and runs off the front of the string
    if len + start < 0 {
        start = 0;
    }

    // If start is negative, count from the end of the string
    let start = if start < 0 { len + start } else { start };

    if length.is_undefined() {
        Ok(Value::string(context.arena, &string[start as usize..]))
    } else {
        assert_arg!(length.is_number(), context, 3);

        let length = length.as_isize();
        if length < 0 {
            Ok(Value::string(context.arena, ""))
        } else {
            let end = if start >= 0 {
                (start + length) as usize
            } else {
                (len + start + length) as usize
            };

            let substring = string
                .chars()
                .skip(start as usize)
                .take(end - start as usize)
                .collect::<String>();

            Ok(Value::string(context.arena, &substring))
        }
    }
}

pub fn fn_contains<'a>(
    context: FunctionContext<'a, '_>,
    args: &[&'a Value<'a>],
) -> Result<&'a Value<'a>> {
    let str_value = args.first().copied().unwrap_or_else(Value::undefined);
    let token_value = args.get(1).copied().unwrap_or_else(Value::undefined);

    if str_value.is_undefined() {
        return Ok(Value::undefined());
    }

    assert_arg!(str_value.is_string(), context, 1);
    assert_arg!(token_value.is_string(), context, 2);

    let str_value = str_value.as_str();
    let token_value = token_value.as_str();

    Ok(Value::bool(str_value.contains(&token_value.to_string())))
}

pub fn fn_replace<'a>(
    context: FunctionContext<'a, '_>,
    args: &[&'a Value<'a>],
) -> Result<&'a Value<'a>> {
    let str_value = args.first().copied().unwrap_or_else(Value::undefined);
    let pattern_value = args.get(1).copied().unwrap_or_else(Value::undefined);
    let replacement_value = args.get(2).copied().unwrap_or_else(Value::undefined);
    let limit_value = args.get(3).copied().unwrap_or_else(Value::undefined);

    if str_value.is_undefined() {
        return Ok(Value::undefined());
    }

    if pattern_value.is_string() && pattern_value.as_str().is_empty() {
        return Err(Error::D3010EmptyPattern(context.char_index));
    }

    assert_arg!(str_value.is_string(), context, 1);
    assert_arg!(pattern_value.is_string(), context, 2);
    assert_arg!(replacement_value.is_string(), context, 3);

    let str_value = str_value.as_str();
    let pattern_value = pattern_value.as_str();
    let replacement_value = replacement_value.as_str();
    let limit_value = if limit_value.is_undefined() {
        None
    } else {
        assert_arg!(limit_value.is_number(), context, 4);
        if limit_value.as_isize().is_negative() {
            return Err(Error::D3011NegativeLimit(context.char_index));
        }
        Some(limit_value.as_isize())
    };

    let replaced_string = if let Some(limit) = limit_value {
        str_value.replacen(
            &pattern_value.to_string(),
            &replacement_value,
            limit as usize,
        )
    } else {
        str_value.replace(&pattern_value.to_string(), &replacement_value)
    };

    Ok(Value::string(context.arena, &replaced_string))
}

pub fn fn_split<'a>(
    context: FunctionContext<'a, '_>,
    args: &[&'a Value<'a>],
) -> Result<&'a Value<'a>> {
    let str_value = args.first().copied().unwrap_or_else(Value::undefined);
    let separator_value = args.get(1).copied().unwrap_or_else(Value::undefined);
    let limit_value = args.get(2).copied().unwrap_or_else(Value::undefined);

    if str_value.is_undefined() {
        return Ok(Value::undefined());
    }

    assert_arg!(str_value.is_string(), context, 1);
    assert_arg!(separator_value.is_string(), context, 2);

    let str_value = str_value.as_str();
    let separator_value = separator_value.as_str();
    let limit_value = if limit_value.is_undefined() {
        None
    } else {
        assert_arg!(limit_value.is_number(), context, 4);
        if limit_value.as_isize().is_negative() {
            return Err(Error::D3020NegativeLimit(context.char_index));
        }
        Some(limit_value.as_isize())
    };

    let substrings: Vec<&str> = if let Some(limit) = limit_value {
        str_value
            .split(&separator_value.to_string())
            .take(limit as usize)
            .collect()
    } else {
        str_value.split(&separator_value.to_string()).collect()
    };

    let substrings_count = substrings.len();

    let result = Value::array_with_capacity(context.arena, substrings_count, ArrayFlags::empty());
    for (index, substring) in substrings.into_iter().enumerate() {
        if substring.is_empty() && (index == 0 || index == substrings_count - 1) {
            continue;
        }
        result.push(Value::string(context.arena, substring));
    }
    Ok(result)
}

pub fn fn_abs<'a>(
    context: FunctionContext<'a, '_>,
    args: &[&'a Value<'a>],
) -> Result<&'a Value<'a>> {
    let arg = args.first().copied().unwrap_or_else(Value::undefined);

    if arg.is_undefined() {
        return Ok(Value::undefined());
    }

    assert_arg!(arg.is_number(), context, 1);

    Ok(Value::number(context.arena, arg.as_f64().abs()))
}

pub fn fn_floor<'a>(
    context: FunctionContext<'a, '_>,
    args: &[&'a Value<'a>],
) -> Result<&'a Value<'a>> {
    let arg = args.first().copied().unwrap_or_else(Value::undefined);

    if arg.is_undefined() {
        return Ok(Value::undefined());
    }

    assert_arg!(arg.is_number(), context, 1);

    Ok(Value::number(context.arena, arg.as_f64().floor()))
}

pub fn fn_ceil<'a>(
    context: FunctionContext<'a, '_>,
    args: &[&'a Value<'a>],
) -> Result<&'a Value<'a>> {
    let arg = args.first().copied().unwrap_or_else(Value::undefined);

    if arg.is_undefined() {
        return Ok(Value::undefined());
    }

    assert_arg!(arg.is_number(), context, 1);

    Ok(Value::number(context.arena, arg.as_f64().ceil()))
}

pub fn fn_lookup_internal<'a>(
    context: FunctionContext<'a, '_>,
    input: &'a Value<'a>,
    key: &str,
) -> &'a Value<'a> {
    match input {
        Value::Array { .. } => {
            let result = Value::array(context.arena, ArrayFlags::SEQUENCE);

            for input in input.members() {
                let res = fn_lookup_internal(context.clone(), input, key);
                match res {
                    Value::Undefined => {}
                    Value::Array { .. } => {
                        res.members().for_each(|item| result.push(item));
                    }
                    _ => result.push(res),
                };
            }

            result
        }
        Value::Object(..) => input.get_entry(key),
        _ => Value::undefined(),
    }
}

pub fn fn_lookup<'a>(
    context: FunctionContext<'a, '_>,
    args: &[&'a Value<'a>],
) -> Result<&'a Value<'a>> {
    let input = args.first().copied().unwrap_or_else(Value::undefined);
    let key = args.get(1).copied().unwrap_or_else(Value::undefined);
    assert_arg!(key.is_string(), context, 2);
    Ok(fn_lookup_internal(context.clone(), input, &key.as_str()))
}

pub fn fn_count<'a>(
    context: FunctionContext<'a, '_>,
    args: &[&'a Value<'a>],
) -> Result<&'a Value<'a>> {
    max_args!(context, args, 1);

    let count = match args.first() {
        Some(Value::Array(a, _)) => a.len() as f64,
        Some(Value::Undefined) => 0.0,
        Some(_) => 1.0,
        None => 0.0,
    };

    Ok(Value::number(context.arena, count))
}

pub fn fn_max<'a>(
    context: FunctionContext<'a, '_>,
    args: &[&'a Value<'a>],
) -> Result<&'a Value<'a>> {
    max_args!(context, args, 1);

    let arg = args.first().copied().unwrap_or_else(Value::undefined);

    // $max(undefined) and $max([]) return undefined
    if arg.is_undefined() || (arg.is_array() && arg.is_empty()) {
        return Ok(Value::undefined());
    }

    let arr = Value::wrap_in_array_if_needed(context.arena, arg, ArrayFlags::empty());

    let mut max = f64::MIN;

    for member in arr.members() {
        assert_array_of_type!(member.is_number(), context, 1, "number");
        max = f64::max(max, member.as_f64());
    }
    Ok(Value::number(context.arena, max))
}

pub fn fn_min<'a>(
    context: FunctionContext<'a, '_>,
    args: &[&'a Value<'a>],
) -> Result<&'a Value<'a>> {
    max_args!(context, args, 1);

    let arg = args.first().copied().unwrap_or_else(Value::undefined);

    // $min(undefined) and $min([]) return undefined
    if arg.is_undefined() || (arg.is_array() && arg.is_empty()) {
        return Ok(Value::undefined());
    }

    let arr = Value::wrap_in_array_if_needed(context.arena, arg, ArrayFlags::empty());

    let mut min = f64::MAX;

    for member in arr.members() {
        assert_array_of_type!(member.is_number(), context, 1, "number");
        min = f64::min(min, member.as_f64());
    }
    Ok(Value::number(context.arena, min))
}

pub fn fn_sum<'a>(
    context: FunctionContext<'a, '_>,
    args: &[&'a Value<'a>],
) -> Result<&'a Value<'a>> {
    max_args!(context, args, 1);

    let arg = args.first().copied().unwrap_or_else(Value::undefined);

    // $sum(undefined) returns undefined
    if arg.is_undefined() {
        return Ok(Value::undefined());
    }

    let arr = Value::wrap_in_array_if_needed(context.arena, arg, ArrayFlags::empty());

    let mut sum = 0.0;

    for member in arr.members() {
        assert_array_of_type!(member.is_number(), context, 1, "number");
        sum += member.as_f64();
    }
    Ok(Value::number(context.arena, sum))
}

pub fn fn_number<'a>(
    context: FunctionContext<'a, '_>,
    args: &[&'a Value<'a>],
) -> Result<&'a Value<'a>> {
    max_args!(context, args, 1);

    let arg = args.first().copied().unwrap_or_else(Value::undefined);

    match arg {
        Value::Undefined => Ok(Value::undefined()),
        Value::Number(..) => Ok(arg),
        Value::Bool(true) => Ok(Value::number(context.arena, 1)),
        Value::Bool(false) => Ok(Value::number(context.arena, 0)),
        Value::String(s) => {
            let result: f64 = s
                .parse()
                .map_err(|_e| Error::D3030NonNumericCast(context.char_index, arg.to_string()))?;

            if !result.is_nan() && !result.is_infinite() {
                Ok(Value::number(context.arena, result))
            } else {
                Ok(Value::undefined())
            }
        }
        _ => bad_arg!(context, 1),
    }
}

pub fn fn_random<'a>(
    context: FunctionContext<'a, '_>,
    args: &[&'a Value<'a>],
) -> Result<&'a Value<'a>> {
    max_args!(context, args, 0);

    let v: f32 = rand::thread_rng().gen();
    Ok(Value::number(context.arena, v))
}

pub fn fn_now<'a>(
    context: FunctionContext<'a, '_>,
    args: &[&'a Value<'a>],
) -> Result<&'a Value<'a>> {
    max_args!(context, args, 2);

    let now = Utc::now();

    let (picture, timezone) = match args {
        [picture, timezone] => (picture.as_str(), timezone.as_str()),
        [picture] => (picture.as_str(), Cow::Borrowed("")),
        [] => (Cow::Borrowed(""), Cow::Borrowed("")),
        _ => return Ok(Value::string(context.arena, "")),
    };

    if picture.is_empty() && timezone.is_empty() {
        return Ok(Value::string(
            context.arena,
            &now.to_rfc3339_opts(chrono::SecondsFormat::Millis, true),
        ));
    }

    let adjusted_time = if !timezone.is_empty() {
        parse_timezone_offset(&timezone)
            .map(|offset| now.with_timezone(&offset))
            .ok_or_else(|| Error::T0410ArgumentNotValid(2, 1, context.name.to_string()))?
    } else {
        now.into()
    };

    // If a valid picture is provided, format the time accordingly
    if !picture.is_empty() {
        // Handle the Result<String, Error> from format_custom_date
        let formatted_date = format_custom_date(&adjusted_time, &picture)?;
        return Ok(Value::string(context.arena, &formatted_date));
    }

    // Return an empty string if the picture is empty but a valid timezone is provided
    Ok(Value::string(context.arena, ""))
}

pub fn fn_exists<'a>(
    context: FunctionContext<'a, '_>,
    args: &[&'a Value<'a>],
) -> Result<&'a Value<'a>> {
    min_args!(context, args, 1);
    max_args!(context, args, 1);

    let arg = args.first().copied().unwrap_or_else(Value::undefined);

    match arg {
        Value::Undefined => Ok(Value::bool(false)),
        _ => Ok(Value::bool(true)),
    }
}

pub fn from_millis<'a>(
    context: FunctionContext<'a, '_>,
    args: &[&'a Value<'a>],
) -> Result<&'a Value<'a>> {
    let arr = args.first().copied().unwrap_or_else(Value::undefined);

    if arr.is_undefined() {
        return Ok(Value::undefined());
    }

    max_args!(context, args, 3);
    assert_arg!(args[0].is_number(), context, 1);

    let millis = args[0].as_f64() as i64;

    let Some(timestamp) = Utc.timestamp_millis_opt(millis).single() else {
        bad_arg!(context, 1);
    };

    let (picture, timezone) = match args {
        [_, picture, timezone] if picture.is_undefined() => {
            assert_arg!(timezone.is_string(), context, 3);
            (Cow::Borrowed(""), timezone.as_str())
        }
        [_, picture, timezone] => {
            assert_arg!(picture.is_string(), context, 2);
            assert_arg!(timezone.is_string(), context, 3);
            (picture.as_str(), timezone.as_str())
        }
        [_, picture] => {
            assert_arg!(picture.is_string(), context, 2);
            (picture.as_str(), Cow::Borrowed(""))
        }
        _ => (Cow::Borrowed(""), Cow::Borrowed("")),
    };

    // Handle default case: ISO 8601 format in UTC
    if picture.is_empty() && timezone.is_empty() {
        return Ok(Value::string(
            context.arena,
            &timestamp.to_rfc3339_opts(chrono::SecondsFormat::Millis, true),
        ));
    }

    // Check for balanced brackets in the picture string
    if let Err(err) = check_balanced_brackets(&picture) {
        return Err(Error::D3135PictureStringNoClosingBracketError(err));
    }

    let adjusted_time = if !timezone.is_empty() {
        parse_timezone_offset(&timezone)
            .map(|offset| timestamp.with_timezone(&offset))
            .ok_or_else(|| Error::T0410ArgumentNotValid(0, 1, context.name.to_string()))?
    } else {
        timestamp.into()
    };

    // If a picture is provided, format the timestamp accordingly
    if !picture.is_empty() {
        // Call format_custom_date and handle its result
        let formatted_result = format_custom_date(&adjusted_time, &picture)?;

        return Ok(Value::string(context.arena, &formatted_result));
    }

    // Return ISO 8601 if only timezone is provided
    Ok(Value::string(
        context.arena,
        &adjusted_time.to_rfc3339_opts(chrono::SecondsFormat::Millis, true),
    ))
}

pub fn fn_millis<'a>(
    context: FunctionContext<'a, '_>,
    args: &[&'a Value<'a>],
) -> Result<&'a Value<'a>> {
    max_args!(context, args, 0);

    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("Time went backwards");

    Ok(Value::number_from_u128(
        context.arena,
        timestamp.as_millis(),
    )?)
}

pub fn fn_uuid<'a>(
    context: FunctionContext<'a, '_>,
    args: &[&'a Value<'a>],
) -> Result<&'a Value<'a>> {
    max_args!(context, args, 0);

    Ok(Value::string(
        context.arena,
        Uuid::new_v4().to_string().as_str(),
    ))
}

pub fn to_millis<'a>(
    context: FunctionContext<'a, '_>,
    args: &[&'a Value<'a>],
) -> Result<&'a Value<'a>> {
    let arr: &Value<'a> = args.first().copied().unwrap_or_else(Value::undefined);

    if arr.is_undefined() {
        return Ok(Value::undefined());
    }

    max_args!(context, args, 2);
    assert_arg!(args[0].is_string(), context, 1);

    // Extract the timestamp string
    let timestamp_str = args[0].as_str();
    if timestamp_str.is_empty() {
        return Ok(Value::undefined());
    }

    // Extract the optional picture string
    let picture = match args {
        [_, picture] if picture.is_undefined() => Cow::Borrowed(""),
        [_, picture] => {
            assert_arg!(picture.is_string(), context, 2);
            picture.as_str()
        }
        _ => Cow::Borrowed(""),
    };

    // Handle different formats using a match handler function
    match parse_custom_format(&timestamp_str, &picture) {
        Some(millis) => Ok(Value::number(context.arena, millis as f64)),
        None => Ok(Value::undefined()),
    }
}

pub fn fn_zip<'a>(
    context: FunctionContext<'a, '_>,
    args: &[&'a Value<'a>],
) -> Result<&'a Value<'a>> {
    // Check for null or undefined values in the arguments
    if args.iter().any(|arg| arg.is_null() || arg.is_undefined()) {
        return Ok(Value::array(context.arena, ArrayFlags::empty()));
    }

    let arrays: Vec<&bumpalo::collections::Vec<'a, &'a Value<'a>>> = args
        .iter()
        .filter_map(|arg| match *arg {
            Value::Array(arr, _) => Some(arr),
            _ => None,
        })
        .collect();

    if arrays.is_empty() {
        let result: bumpalo::collections::Vec<&Value<'a>> =
            args.iter().copied().collect_in(context.arena);

        let outer_array =
            Value::array_from(context.arena, result, ArrayFlags::empty()) as &Value<'a>;

        let outer_array_alloc: bumpalo::collections::Vec<&Value<'a>> =
            bumpalo::vec![in context.arena; outer_array];

        return Ok(Value::array_from(
            context.arena,
            outer_array_alloc,
            ArrayFlags::empty(),
        ));
    }

    let min_length = arrays.iter().map(|arr| arr.len()).min().unwrap_or(0);
    let mut iterators: Vec<_> = arrays
        .iter()
        .map(|arr| arr.iter().take(min_length))
        .collect();

    // Use an iterator of zipping all the array iterators and collect the result in bumpalo
    let result: bumpalo::collections::Vec<&Value<'a>> = std::iter::repeat(())
        .take(min_length)
        .map(|_| {
            let zipped: bumpalo::collections::Vec<&Value<'a>> = iterators
                .iter_mut()
                .map(|it| *it.next().unwrap()) // Dereference here to get `&Value<'a>`
                .collect_in(context.arena);

            // Allocate the zipped tuple as a new array in the bumpalo arena
            context
                .arena
                .alloc(Value::Array(zipped, ArrayFlags::empty())) as &Value<'a>
        })
        .collect_in(context.arena);

    // Return the final result array created from the zipped arrays
    Ok(Value::array_from(
        context.arena,
        result,
        ArrayFlags::empty(),
    ))
}

pub fn single<'a>(
    context: FunctionContext<'a, '_>,
    args: &[&'a Value<'a>],
) -> Result<&'a Value<'a>> {
    max_args!(context, args, 2);

    let arr: &Value<'a> = args.first().copied().unwrap_or_else(Value::undefined);
    if arr.is_undefined() {
        return Ok(Value::undefined());
    }

    let func = args
        .get(1)
        .filter(|f| f.is_function())
        .copied()
        .unwrap_or_else(|| {
            // Default function that always returns true
            context
                .arena
                .alloc(Value::nativefn(context.arena, "default_true", 1, |_, _| {
                    Ok(&Value::Bool(true))
                }))
        });

    if !arr.is_array() {
        let res = context.evaluate_function(func, &[arr])?;
        return if res.as_bool() {
            Ok(arr)
        } else {
            Err(Error::D3139Error(
                "No value matched the predicate.".to_string(),
            ))
        };
    }

    if let Value::Array(elements, _) = arr {
        let mut result: Option<&'a Value<'a>> = None;

        for (index, entry) in elements.iter().enumerate() {
            let res = context.evaluate_function(
                func,
                &[entry, Value::number(context.arena, index as f64), arr],
            )?;

            if res.as_bool() {
                if result.is_some() {
                    return Err(Error::D3138Error(format!(
                        "More than one value matched the predicate at index {}",
                        index
                    )));
                } else {
                    result = Some(entry);
                }
            }
        }

        result.ok_or_else(|| Error::D3139Error("No values matched the predicate.".to_string()))
    } else {
        Err(Error::T0410ArgumentNotValid(0, 2, context.name.to_string()))
    }
}

pub fn fn_assert<'a>(
    context: FunctionContext<'a, '_>,
    args: &[&'a Value<'a>],
) -> Result<&'a Value<'a>> {
    let condition = args.first().copied().unwrap_or_else(Value::undefined);
    let message = args.get(1).copied().unwrap_or_else(Value::undefined);

    assert_arg!(condition.is_bool(), context, 1);

    if let Value::Bool(false) = condition {
        Err(Error::D3141Assert(if message.is_string() {
            message.as_str().to_string()
        } else {
            "$assert() statement failed".to_string()
        }))
    } else {
        Ok(Value::undefined())
    }
}

pub fn fn_error<'a>(
    context: FunctionContext<'a, '_>,
    args: &[&'a Value<'a>],
) -> Result<&'a Value<'a>> {
    let message = args.first().copied().unwrap_or_else(Value::undefined);

    assert_arg!(message.is_undefined() || message.is_string(), context, 1);

    Err(Error::D3137Error(if message.is_string() {
        message.as_str().to_string()
    } else {
        "$error() function evaluated".to_string()
    }))
}

pub fn fn_length<'a>(
    context: FunctionContext<'a, '_>,
    args: &[&'a Value<'a>],
) -> Result<&'a Value<'a>> {
    max_args!(context, args, 1);

    let arg1 = args.first().copied().unwrap_or_else(Value::undefined);

    if arg1.is_undefined() {
        return Ok(Value::undefined());
    }

    assert_arg!(arg1.is_string(), context, 1);

    Ok(Value::number(
        context.arena,
        arg1.as_str().chars().count() as f64,
    ))
}

pub fn fn_sqrt<'a>(
    context: FunctionContext<'a, '_>,
    args: &[&'a Value<'a>],
) -> Result<&'a Value<'a>> {
    max_args!(context, args, 1);
    let arg1 = args.first().copied().unwrap_or_else(Value::undefined);

    if arg1.is_undefined() {
        return Ok(Value::undefined());
    }

    assert_arg!(arg1.is_number(), context, 1);

    let n = arg1.as_f64();
    if n.is_sign_negative() {
        Err(Error::D3060SqrtNegative(context.char_index, n.to_string()))
    } else {
        Ok(Value::number(context.arena, n.sqrt()))
    }
}

pub fn fn_power<'a>(
    context: FunctionContext<'a, '_>,
    args: &[&'a Value<'a>],
) -> Result<&'a Value<'a>> {
    max_args!(context, args, 2);

    let number = args.first().copied().unwrap_or_else(Value::undefined);
    let exp = args.get(1).copied().unwrap_or_else(Value::undefined);

    if number.is_undefined() {
        return Ok(Value::undefined());
    }

    assert_arg!(number.is_number(), context, 1);
    assert_arg!(exp.is_number(), context, 2);

    let result = number.as_f64().powf(exp.as_f64());

    if !result.is_finite() {
        Err(Error::D3061PowUnrepresentable(
            context.char_index,
            number.to_string(),
            exp.to_string(),
        ))
    } else {
        Ok(Value::number(context.arena, result))
    }
}

pub fn fn_reverse<'a>(
    context: FunctionContext<'a, '_>,
    args: &[&'a Value<'a>],
) -> Result<&'a Value<'a>> {
    max_args!(context, args, 1);

    let arr = args.first().copied().unwrap_or_else(Value::undefined);

    if arr.is_undefined() {
        return Ok(Value::undefined());
    }

    assert_arg!(arr.is_array(), context, 1);

    let result = Value::array_with_capacity(context.arena, arr.len(), ArrayFlags::empty());
    arr.members().rev().for_each(|member| result.push(member));
    Ok(result)
}

#[allow(clippy::mutable_key_type)]
pub fn fn_distinct<'a>(
    context: FunctionContext<'a, '_>,
    args: &[&'a Value<'a>],
) -> Result<&'a Value<'a>> {
    max_args!(context, args, 1);

    let arr = args.first().copied().unwrap_or_else(Value::undefined);
    if !arr.is_array() || arr.len() <= 1 {
        return Ok(arr);
    }

    let result = Value::array_with_capacity(context.arena, arr.len(), ArrayFlags::empty());
    let mut set = HashSet::new();
    for member in arr.members() {
        if set.contains(member) {
            continue;
        }
        set.insert(member);
        result.push(member);
    }

    Ok(result)
}

pub fn fn_join<'a>(
    context: FunctionContext<'a, '_>,
    args: &[&'a Value<'a>],
) -> Result<&'a Value<'a>> {
    max_args!(context, args, 2);
    let strings = args.first().copied().unwrap_or_else(Value::undefined);

    if strings.is_undefined() {
        return Ok(Value::undefined());
    }

    if strings.is_string() {
        return Ok(strings);
    }

    assert_array_of_type!(strings.is_array(), context, 1, "string");

    let separator = args.get(1).copied().unwrap_or_else(Value::undefined);
    assert_arg!(
        separator.is_undefined() || separator.is_string(),
        context,
        2
    );

    let separator = if separator.is_string() {
        separator.as_str()
    } else {
        "".into()
    };

    let mut result = String::with_capacity(1024);
    for (index, member) in strings.members().enumerate() {
        assert_array_of_type!(member.is_string(), context, 1, "string");
        result.push_str(member.as_str().borrow());
        if index != strings.len() - 1 {
            result.push_str(&separator);
        }
    }

    Ok(Value::string(context.arena, &result))
}

pub fn fn_sort<'a, 'e>(
    context: FunctionContext<'a, 'e>,
    args: &[&'a Value<'a>],
) -> Result<&'a Value<'a>> {
    max_args!(context, args, 2);

    let arr = args.first().copied().unwrap_or_else(Value::undefined);

    if arr.is_undefined() {
        return Ok(Value::undefined());
    }

    if !arr.is_array() || arr.len() <= 1 {
        return Ok(Value::wrap_in_array_if_needed(
            context.arena,
            arr,
            ArrayFlags::empty(),
        ));
    }

    // TODO: This is all a bit inefficient, copying Vecs of references around, but
    // at least it's just references.

    let unsorted = arr.members().collect::<Vec<&'a Value<'a>>>();
    let sorted = if args.get(1).is_none() {
        merge_sort(
            unsorted,
            &|a: &'a Value<'a>, b: &'a Value<'a>| match (a, b) {
                (Value::Number(a), Value::Number(b)) => Ok(a > b),
                (Value::String(a), Value::String(b)) => Ok(a > b),
                _ => Err(Error::D3070InvalidDefaultSort(context.char_index)),
            },
        )?
    } else {
        let comparator = args.get(1).copied().unwrap_or_else(Value::undefined);
        assert_arg!(comparator.is_function(), context, 2);
        merge_sort(unsorted, &|a: &'a Value<'a>, b: &'a Value<'a>| {
            let result = context.evaluate_function(comparator, &[a, b])?;
            Ok(result.is_truthy())
        })?
    };

    let result = Value::array_with_capacity(context.arena, sorted.len(), arr.get_flags());
    sorted.iter().for_each(|member| result.push(member));

    Ok(result)
}

pub fn merge_sort<'a, F>(items: Vec<&'a Value<'a>>, comp: &F) -> Result<Vec<&'a Value<'a>>>
where
    F: Fn(&'a Value<'a>, &'a Value<'a>) -> Result<bool>,
{
    fn merge_iter<'a, F>(
        result: &mut Vec<&'a Value<'a>>,
        left: &[&'a Value<'a>],
        right: &[&'a Value<'a>],
        comp: &F,
    ) -> Result<()>
    where
        F: Fn(&'a Value<'a>, &'a Value<'a>) -> Result<bool>,
    {
        if left.is_empty() {
            result.extend(right);
            Ok(())
        } else if right.is_empty() {
            result.extend(left);
            Ok(())
        } else if comp(left[0], right[0])? {
            result.push(right[0]);
            merge_iter(result, left, &right[1..], comp)
        } else {
            result.push(left[0]);
            merge_iter(result, &left[1..], right, comp)
        }
    }

    fn merge<'a, F>(
        left: &[&'a Value<'a>],
        right: &[&'a Value<'a>],
        comp: &F,
    ) -> Result<Vec<&'a Value<'a>>>
    where
        F: Fn(&'a Value<'a>, &'a Value<'a>) -> Result<bool>,
    {
        let mut merged = Vec::with_capacity(left.len() + right.len());
        merge_iter(&mut merged, left, right, comp)?;
        Ok(merged)
    }

    if items.len() <= 1 {
        return Ok(items);
    }
    let middle = (items.len() as f64 / 2.0).floor() as usize;
    let (left, right) = items.split_at(middle);
    let left = merge_sort(left.to_vec(), comp)?;
    let right = merge_sort(right.to_vec(), comp)?;
    merge(&left, &right, comp)
}

pub fn fn_base64_encode<'a>(
    context: FunctionContext<'a, '_>,
    args: &[&'a Value<'a>],
) -> Result<&'a Value<'a>> {
    max_args!(context, args, 1);
    let arg = args.first().copied().unwrap_or_else(Value::undefined);
    if arg.is_undefined() {
        return Ok(Value::undefined());
    }
    assert_arg!(arg.is_string(), context, 1);

    let base64 = base64::engine::general_purpose::STANDARD;

    let encoded = base64.encode(arg.as_str().as_bytes());

    Ok(Value::string(context.arena, &encoded))
}

pub fn fn_base64_decode<'a>(
    context: FunctionContext<'a, '_>,
    args: &[&'a Value<'a>],
) -> Result<&'a Value<'a>> {
    max_args!(context, args, 1);
    let arg = args.first().copied().unwrap_or_else(Value::undefined);
    if arg.is_undefined() {
        return Ok(Value::undefined());
    }
    assert_arg!(arg.is_string(), context, 1);

    let base64 = base64::engine::general_purpose::STANDARD;

    let decoded = base64.decode(arg.as_str().as_bytes());
    let data = decoded.map_err(|e| Error::D3137Error(e.to_string()))?;
    let decoded = String::from_utf8(data).map_err(|e| Error::D3137Error(e.to_string()))?;

    Ok(Value::string(context.arena, &decoded))
}

pub fn fn_round<'a>(
    context: FunctionContext<'a, '_>,
    args: &[&'a Value<'a>],
) -> Result<&'a Value<'a>> {
    max_args!(context, args, 2);
    let number = &args[0];
    if number.is_undefined() {
        return Ok(Value::undefined());
    }
    assert_arg!(number.is_number(), context, 1);

    let precision = if let Some(precision) = args.get(1) {
        assert_arg!(precision.is_integer(), context, 2);
        precision.as_isize()
    } else {
        0
    };

    let num = multiply_by_pow10(number.as_f64(), precision)?;
    let num = num.round_ties_even();
    let num = multiply_by_pow10(num, -precision)?;

    Ok(Value::number(context.arena, num))
}

fn is_array_of_strings(value: &Value) -> bool {
    if let Value::Array(elements, _) = value {
        elements.iter().all(|v| v.is_string())
    } else {
        false
    }
}

pub fn fn_reduce<'a>(
    context: FunctionContext<'a, '_>,
    args: &[&'a Value<'a>],
) -> Result<&'a Value<'a>> {
    max_args!(context, args, 3);

    if args.len() < 2 {
        return Err(Error::T0410ArgumentNotValid(0, 2, context.name.to_string()));
    }

    let original_value = args[0];
    let func = args[1];
    let init = args.get(2).copied();

    if func.is_function() && func.arity() < 2 {
        return Err(Error::D3050SecondArguement(context.name.to_string()));
    }

    if !original_value.is_array() {
        if original_value.is_number() {
            return Ok(original_value);
        }

        if original_value.is_string() {
            return Ok(original_value);
        }

        return Ok(Value::undefined());
    }

    let (elements, _extra_field) = match original_value {
        Value::Array(elems, extra) => (elems, extra),
        _ => return Err(Error::D3050SecondArguement(context.name.to_string())),
    };

    if elements.is_empty() {
        return Ok(init.unwrap_or_else(|| Value::undefined()));
    }

    if !func.is_function() {
        return Err(Error::T0410ArgumentNotValid(1, 1, context.name.to_string()));
    }

    let mut accumulator = init.unwrap_or_else(|| elements[0]);

    let has_init_value = init.is_some();
    let is_non_single_array_of_strings = is_array_of_strings(original_value) && elements.len() > 1;

    let start_index = if has_init_value || is_non_single_array_of_strings {
        0
    } else {
        1
    };

    for (index, value) in elements[start_index..].iter().enumerate() {
        let index_value = Value::number(context.arena, index as f64);

        let result =
            context.evaluate_function(func, &[accumulator, value, index_value, original_value]);

        match result {
            Ok(new_accumulator) => {
                accumulator = new_accumulator;
            }
            Err(_) => {
                return Err(Error::T0410ArgumentNotValid(1, 1, context.name.to_string()));
            }
        }
    }

    Ok(accumulator)
}

// We need to do this multiplication by powers of 10 in a string to avoid
// floating point precision errors which will affect the rounding algorithm
fn multiply_by_pow10(num: f64, pow: isize) -> Result<f64> {
    let num_str = format!("{}e{}", num, pow);
    num_str
        .parse::<f64>()
        .map_err(|e| Error::D3137Error(e.to_string()))
}

pub fn fn_pad<'a>(
    context: FunctionContext<'a, '_>,
    args: &[&'a Value<'a>],
) -> Result<&'a Value<'a>> {
    let str_value = args.first().copied().unwrap_or_else(Value::undefined);
    if !str_value.is_string() {
        return Ok(Value::undefined());
    }

    let width_value = args.get(1).copied().unwrap_or_else(Value::undefined);
    if !width_value.is_number() {
        return Ok(Value::undefined());
    }

    let str_to_pad = str_value.as_str(); // as_str returns Cow<'_, str>

    let width_i64 = width_value.as_f64().round() as i64;
    let width = width_i64.unsigned_abs() as usize;
    let is_right_padding = width_i64 > 0; // Positive width means right padding

    let pad_char = args
        .get(2)
        .map(|v| v.as_str())
        .filter(|c| !c.is_empty())
        .unwrap_or(Cow::Borrowed(" "));

    let pad_length = width.saturating_sub(str_to_pad.chars().count());

    // Early return if no padding is needed
    if pad_length == 0 {
        return Ok(Value::string(context.arena, &str_to_pad));
    }

    let padding = pad_char
        .chars()
        .cycle()
        .take(pad_length)
        .collect::<String>();

    // Depending on whether it's right or left padding, append or prepend the padding
    let result = if is_right_padding {
        format!("{}{}", str_to_pad, padding)
    } else {
        format!("{}{}", padding, str_to_pad)
    };

    Ok(Value::string(context.arena, &result))
}

pub fn fn_match<'a>(
    context: FunctionContext<'a, '_>,
    args: &[&'a Value<'a>],
) -> Result<&'a Value<'a>> {
    let value_to_validate = match args.first().copied() {
        Some(val) if !val.is_undefined() => val,
        _ => return Ok(Value::undefined()),
    };
    assert_arg!(value_to_validate.is_string(), context, 1);

    let pattern_value = match args.get(1).copied() {
        Some(val) => val,
        _ => return Err(Error::D3010EmptyPattern(context.char_index)),
    };

    let regex_literal = match pattern_value {
        Value::Regex(ref regex_literal) => regex_literal,
        Value::String(ref s) => {
            let regex = RegexLiteral::new(s.as_str(), false, false)
                .map_err(|_| Error::D3010EmptyPattern(context.char_index))?;
            &*context.arena.alloc(regex)
        }
        _ => return Err(Error::D3010EmptyPattern(context.char_index)),
    };

    let limit = args
        .get(2)
        .and_then(|val| {
            if val.is_number() {
                Some(val.as_f64() as usize)
            } else {
                None
            }
        })
        .unwrap_or(usize::MAX);

    let key_match = BumpString::from_str_in("match", context.arena);
    let key_index = BumpString::from_str_in("index", context.arena);
    let key_groups = BumpString::from_str_in("groups", context.arena);

    let mut matches: bumpalo::collections::Vec<&Value<'a>> =
        bumpalo::collections::Vec::new_in(context.arena);

    for (i, m) in regex_literal
        .get_regex()
        .find_iter(&value_to_validate.as_str())
        .enumerate()
    {
        if i >= limit {
            break;
        }

        let matched_text = &value_to_validate.as_str()[m.start()..m.end()];
        let match_str = context
            .arena
            .alloc(Value::string(context.arena, matched_text));

        let index_val = context
            .arena
            .alloc(Value::number(context.arena, m.start() as f64));

        let group_vec: bumpalo::collections::Vec<&Value<'a>> =
            bumpalo::collections::Vec::new_in(context.arena);
        let groups_val = context
            .arena
            .alloc(Value::Array(group_vec, ArrayFlags::empty()));

        let mut match_obj: HashMap<BumpString, &Value<'a>, DefaultHashBuilder, &Bump> =
            HashMap::with_capacity_and_hasher_in(3, DefaultHashBuilder::default(), context.arena);
        match_obj.insert(key_match.clone(), match_str);
        match_obj.insert(key_index.clone(), index_val);
        match_obj.insert(key_groups.clone(), groups_val);

        matches.push(context.arena.alloc(Value::Object(match_obj)));
    }

    Ok(context
        .arena
        .alloc(Value::Array(matches, ArrayFlags::empty())))
}

pub fn fn_eval<'a>(
    context: FunctionContext<'a, '_>,
    args: &[&'a Value<'a>],
) -> Result<&'a Value<'a>> {
    max_args!(context, args, 2);

    let expr = args.first().copied().unwrap_or_else(Value::undefined);

    if expr.is_null() {
        Ok(Value::null(context.arena))
    } else if expr.is_undefined() || !expr.is_string() {
        Ok(Value::undefined())
    } else {
        let expr_str = expr.as_str();
        if expr_str.is_empty() {
            Ok(Value::null(context.arena))
        } else {
            let override_context = args.get(1).copied().unwrap_or(context.input);
            preprocess_and_eval_jsonata(&context, &expr_str, override_context)
                .or_else(|_| Ok(Value::undefined()))
        }
    }
}

fn preprocess_and_eval_jsonata<'a>(
    context: &FunctionContext<'a, '_>,
    expr_str: &str,
    override_context: &'a Value<'a>,
) -> Result<&'a Value<'a>> {
    // Check if the input contains embedded JSONata expressions like $string(2)
    let preprocessed_str = preprocess_jsonata_expressions(context, expr_str, override_context)?;

    // Now attempt to parse the preprocessed string as JSON
    match serde_json::from_str::<JsonValue>(&preprocessed_str) {
        Ok(parsed_json) => {
            // Traverse and evaluate JSONata expressions inside the parsed JSON
            traverse_and_eval_json(context, &parsed_json, override_context)
        }
        Err(_) => {
            // If JSON parsing fails, treat it as a JSONata expression
            match evaluate_jsonata_expression(context.clone(), expr_str, &[override_context]) {
                Some(result) => Ok(result),
                None => Ok(Value::undefined()),
            }
        }
    }
}

fn preprocess_jsonata_expressions<'a>(
    context: &FunctionContext<'a, '_>,
    expr_str: &str,
    override_context: &'a Value<'a>,
) -> Result<String> {
    let re = regress::Regex::new(r"\$string\((\d+)\)")
        .map_err(|e| Error::S0303InvalidRegex(context.char_index, e.to_string()))?;

    let mut result_str = String::new();
    let mut last_end = 0;

    for m in re.find_iter(expr_str) {
        let match_str = &expr_str[m.start()..m.end()];

        let group_start = match_str
            .find('(')
            .ok_or(Error::S0302UnterminatedRegex(context.char_index))?
            + 1;
        let group_end = match_str
            .find(')')
            .ok_or(Error::S0302UnterminatedRegex(context.char_index))?;
        let arg = &match_str[group_start..group_end];

        let expression = format!("$string({})", arg);

        let replacement =
            match evaluate_jsonata_expression(context.clone(), &expression, &[override_context]) {
                Some(Value::String(s)) => format!("\"{}\"", s),
                Some(Value::Number(n)) => n.to_string(),
                Some(Value::Bool(b)) => b.to_string(),
                Some(Value::Null) => "null".to_string(),
                _ => "null".to_string(),
            };

        result_str.push_str(&expr_str[last_end..m.start()]);
        result_str.push_str(&replacement);
        last_end = m.end();
    }

    result_str.push_str(&expr_str[last_end..]);
    Ok(result_str)
}

fn traverse_and_eval_json<'a>(
    context: &FunctionContext<'a, '_>,
    json: &JsonValue,
    override_context: &'a Value<'a>,
) -> Result<&'a Value<'a>> {
    match json {
        JsonValue::Null => Ok(context.arena.alloc(Value::Null)),
        JsonValue::Bool(b) => Ok(context.arena.alloc(Value::Bool(*b))),
        JsonValue::Number(n) => {
            if let Some(i) = n.as_i64() {
                Ok(context.arena.alloc(Value::Number(i as f64)))
            } else if let Some(f) = n.as_f64() {
                Ok(context.arena.alloc(Value::Number(f)))
            } else {
                Ok(context.arena.alloc(Value::Undefined))
            }
        }
        JsonValue::String(s) => {
            if s.starts_with('$') {
                evaluate_jsonata_expression(context.clone(), s, &[override_context])
                    .ok_or_else(|| Error::D3137Error("JSONata expression failed".to_string()))
            } else {
                Ok(context
                    .arena
                    .alloc(Value::String(bumpalo::collections::String::from_str_in(
                        s,
                        context.arena,
                    ))))
            }
        }
        JsonValue::Array(arr) => {
            let array_values: Result<BumpVec<'a, &'a Value<'a>>> = arr
                .iter()
                .map(|v| traverse_and_eval_json(context, v, override_context))
                .collect_in(context.arena);

            Ok(context
                .arena
                .alloc(Value::Array(array_values?, ArrayFlags::empty())))
        }
        JsonValue::Object(obj) => {
            let mut map = HashMap::with_capacity_in(obj.len(), context.arena);
            for (k, v) in obj {
                let key = bumpalo::collections::String::from_str_in(k, context.arena);
                let value = traverse_and_eval_json(context, v, override_context)?;
                map.insert(key, value);
            }
            Ok(context.arena.alloc(Value::Object(map)))
        }
    }
}

fn evaluate_jsonata_expression<'a>(
    context: FunctionContext<'a, '_>,
    expr_str: &str,
    args: &[&'a Value<'a>],
) -> Option<&'a Value<'a>> {
    if expr_str.starts_with("$string(") && expr_str.ends_with(')') {
        let inner_expr = &expr_str[8..expr_str.len() - 1];
        if let Ok(num) = inner_expr.parse::<i64>() {
            let string_value = num.to_string();
            return Some(context.arena.alloc(Value::String(
                bumpalo::collections::String::from_str_in(&string_value, context.arena),
            )));
        }
    }

    if expr_str.starts_with("$eval(") && expr_str.ends_with(')') {
        let inner_expr = &expr_str[6..expr_str.len() - 1];
        return evaluate_jsonata_expression(context.clone(), inner_expr, args);
    }

    let parts: Vec<&str> = expr_str.split("~>").map(str::trim).collect();
    if parts.is_empty() {
        return None;
    }

    let lhs_expr = parts[0];
    let root = args.first()?;

    // Traverse and collect values for the left-hand side (LHS) expression
    let lhs_value = traverse_and_collect_values(&context, root, lhs_expr);

    let rhs_expr = parts.get(1).unwrap_or(&"");

    match *rhs_expr {
        "$sum()" => {
            // Implement `$sum()` to sum all collected values
            fn_sum(context, &[lhs_value]).ok()
        }
        "$string()" => {
            // Convert the collected values to a string
            fn_string(context, &[lhs_value]).ok()
        }
        _ => None,
    }
}

fn traverse_and_collect_values<'a>(
    context: &FunctionContext<'a, '_>,
    value: &'a Value<'a>,
    path: &str, // JSON path like "Account.Order.Product.Quantity"
) -> &'a Value<'a> {
    let mut extracted_values = BumpVec::new_in(context.arena);

    traverse_recursive(
        value,
        &path.split('.').collect::<Vec<&str>>(),
        &mut extracted_values,
    );

    context
        .arena
        .alloc(Value::Array(extracted_values, ArrayFlags::empty()))
}

fn traverse_recursive<'a>(
    current_value: &'a Value<'a>,
    path_parts: &[&str],
    extracted_values: &mut BumpVec<'a, &'a Value<'a>>,
) {
    if path_parts.is_empty() {
        extracted_values.push(current_value);
        return;
    }

    let key = path_parts[0];
    let remaining_path = &path_parts[1..];

    match current_value {
        Value::Object(obj) => {
            if let Some(next_value) = obj.get(key) {
                traverse_recursive(next_value, remaining_path, extracted_values);
            }
        }
        Value::Array(arr, _) => {
            for item in arr.iter() {
                traverse_recursive(item, path_parts, extracted_values);
            }
        }
        _ => {}
    }
}
