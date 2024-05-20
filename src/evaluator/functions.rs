use base64::Engine;
use std::borrow::Borrow;

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
        args: &'a Value<'a>,
    ) -> Result<&'a Value<'a>> {
        self.evaluator
            .apply_function(self.char_index, self.input, proc, args, &self.frame)
    }
}

// Version of append that takes a mutable arg1 - this could probably be collapsed
pub fn fn_append_internal<'a>(
    context: FunctionContext<'a, '_>,
    arg1: &'a mut Value<'a>,
    arg2: &'a Value<'a>,
) -> &'a mut Value<'a> {
    if arg2.is_undefined() {
        return arg1;
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
        result.push(&*arg1);
    }

    if arg2.is_array() {
        arg2.members().for_each(|m| result.push(m));
    } else {
        result.push(arg2);
    }

    result
}

pub fn fn_append<'a>(
    context: FunctionContext<'a, '_>,
    args: &'a Value<'a>,
) -> Result<&'a Value<'a>> {
    let arg1 = &args[0];
    let arg2 = &args[1];

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
    args: &'a Value<'a>,
) -> Result<&'a Value<'a>> {
    max_args!(context, args, 1);

    let arg = &args[0];
    Ok(match arg {
        Value::Undefined => Value::undefined(),
        Value::Null => Value::bool(context.arena, false),
        Value::Bool(b) => Value::bool(context.arena, *b),
        Value::Number(n) => {
            arg.is_valid_number()?;
            Value::bool(context.arena, *n != 0.0)
        }
        Value::String(ref str) => Value::bool(context.arena, !str.is_empty()),
        Value::Object(ref obj) => Value::bool(context.arena, !obj.is_empty()),
        Value::Array { .. } => match arg.len() {
            0 => Value::bool(context.arena, false),
            1 => fn_boolean(
                context.clone(),
                Value::wrap_in_array(context.arena, arg.get_member(0), ArrayFlags::empty()),
            )?,
            _ => {
                for item in arg.members() {
                    if fn_boolean(
                        context.clone(),
                        Value::wrap_in_array(context.arena, item, ArrayFlags::empty()),
                    )?
                    .as_bool()
                    {
                        return Ok(Value::bool(context.arena, true));
                    }
                }
                Value::bool(context.arena, false)
            }
        },
        Value::Lambda { .. } | Value::NativeFn { .. } | Value::Transformer { .. } => {
            Value::bool(context.arena, false)
        }
        Value::Range(ref range) => Value::bool(context.arena, !range.is_empty()),
    })
}

pub fn fn_map<'a>(context: FunctionContext<'a, '_>, args: &'a Value<'a>) -> Result<&'a Value<'a>> {
    let arr = &args[0];
    let func = &args[1];

    if arr.is_undefined() {
        return Ok(Value::undefined());
    }

    let arr = Value::wrap_in_array_if_needed(context.arena, arr, ArrayFlags::empty());

    assert_arg!(func.is_function(), context, 2);

    let result = Value::array(context.arena, ArrayFlags::SEQUENCE);

    for (index, item) in arr.members().enumerate() {
        let args = Value::array(context.arena, ArrayFlags::empty());
        let arity = func.arity();

        args.push(item);
        if arity >= 2 {
            args.push(Value::number(context.arena, index as f64));
        }
        if arity >= 3 {
            args.push(arr);
        }

        let mapped = context.evaluate_function(func, args)?;
        if !mapped.is_undefined() {
            result.push(mapped);
        }
    }

    Ok(result)
}

pub fn fn_filter<'a>(
    context: FunctionContext<'a, '_>,
    args: &'a Value<'a>,
) -> Result<&'a Value<'a>> {
    let arr = &args[0];
    let func = &args[1];

    if arr.is_undefined() {
        return Ok(Value::undefined());
    }

    let arr = Value::wrap_in_array_if_needed(context.arena, arr, ArrayFlags::empty());

    assert_arg!(func.is_function(), context, 2);

    let result = Value::array(context.arena, ArrayFlags::SEQUENCE);

    for (index, item) in arr.members().enumerate() {
        let args = Value::array(context.arena, ArrayFlags::empty());
        let arity = func.arity();

        args.push(item);
        if arity >= 2 {
            args.push(Value::number(context.arena, index as f64));
        }
        if arity >= 3 {
            args.push(arr);
        }

        let include = context.evaluate_function(func, args)?;

        if include.is_truthy() {
            result.push(item);
        }
    }

    Ok(result)
}

pub fn fn_each<'a>(context: FunctionContext<'a, '_>, args: &'a Value<'a>) -> Result<&'a Value<'a>> {
    let (obj, func) = if args.len() == 1 {
        let obj_arg = if context.input.is_array() && context.input.has_flags(ArrayFlags::WRAPPED) {
            &context.input[0]
        } else {
            context.input
        };

        (obj_arg, &args[0])
    } else {
        (&args[0], &args[1])
    };

    if obj.is_undefined() {
        return Ok(Value::undefined());
    }

    assert_arg!(obj.is_object(), context, 1);
    assert_arg!(func.is_function(), context, 2);

    let result = Value::array(context.arena, ArrayFlags::SEQUENCE);

    for (key, value) in obj.entries() {
        let args = Value::array(context.arena, ArrayFlags::empty());
        let key = Value::string(context.arena, key.to_string());

        args.push(value);
        args.push(key);

        let mapped = context.evaluate_function(func, args)?;
        if !mapped.is_undefined() {
            result.push(mapped);
        }
    }

    Ok(result)
}

pub fn fn_keys<'a>(context: FunctionContext<'a, '_>, args: &'a Value<'a>) -> Result<&'a Value<'a>> {
    let obj = if args.is_empty() {
        if context.input.is_array() && context.input.has_flags(ArrayFlags::WRAPPED) {
            &context.input[0]
        } else {
            context.input
        }
    } else {
        &args[0]
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
        result.push(Value::string(context.arena, key));
    }

    Ok(result)
}

pub fn fn_merge<'a>(
    context: FunctionContext<'a, '_>,
    args: &'a Value<'a>,
) -> Result<&'a Value<'a>> {
    let mut array_of_objects = if args.is_empty() {
        if context.input.is_array() && context.input.has_flags(ArrayFlags::WRAPPED) {
            &context.input[0]
        } else {
            context.input
        }
    } else {
        &args[0]
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
    args: &'a Value<'a>,
) -> Result<&'a Value<'a>> {
    max_args!(context, args, 2);

    let input = if args.is_empty() {
        if context.input.is_array() && context.input.has_flags(ArrayFlags::WRAPPED) {
            &context.input[0]
        } else {
            context.input
        }
    } else {
        &args[0]
    };

    if input.is_undefined() {
        return Ok(Value::undefined());
    }

    let pretty = &args[1];
    assert_arg!(pretty.is_undefined() || pretty.is_bool(), context, 2);

    if input.is_string() {
        Ok(input)
    } else if input.is_function() {
        Ok(Value::string(context.arena, String::from("")))
    } else if input.is_number() && !input.is_finite() {
        Err(Error::D3001StringNotFinite(context.char_index))
    } else if *pretty == true {
        let serializer = Serializer::new(PrettyFormatter::default(), true);
        let output = serializer.serialize(input)?;
        Ok(Value::string(context.arena, output))
    } else {
        let serializer = Serializer::new(DumpFormatter, true);
        let output = serializer.serialize(input)?;
        Ok(Value::string(context.arena, output))
    }
}

pub fn fn_not<'a>(context: FunctionContext<'a, '_>, args: &'a Value<'a>) -> Result<&'a Value<'a>> {
    let arg = &args[0];

    Ok(if arg.is_undefined() {
        Value::undefined()
    } else {
        Value::bool(context.arena, !arg.is_truthy())
    })
}

pub fn fn_lowercase<'a>(
    context: FunctionContext<'a, '_>,
    args: &'a Value<'a>,
) -> Result<&'a Value<'a>> {
    let arg = &args[0];

    Ok(if !arg.is_string() {
        Value::undefined()
    } else {
        Value::string(context.arena, arg.as_str().to_lowercase())
    })
}

pub fn fn_uppercase<'a>(
    context: FunctionContext<'a, '_>,
    args: &'a Value<'a>,
) -> Result<&'a Value<'a>> {
    let arg = &args[0];

    if !arg.is_string() {
        Ok(Value::undefined())
    } else {
        Ok(Value::string(context.arena, arg.as_str().to_uppercase()))
    }
}

pub fn fn_trim<'a>(context: FunctionContext<'a, '_>, args: &'a Value<'a>) -> Result<&'a Value<'a>> {
    let arg = &args[0];

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
        Ok(Value::string(context.arena, trimmed))
    }
}

pub fn fn_substring<'a>(
    context: FunctionContext<'a, '_>,
    args: &'a Value<'a>,
) -> Result<&'a Value<'a>> {
    let string = &args[0];
    let start = &args[1];
    let length = &args[2];

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
        Ok(Value::string(
            context.arena,
            string[start as usize..].to_string(),
        ))
    } else {
        assert_arg!(length.is_number(), context, 3);

        let length = length.as_isize();
        if length < 0 {
            Ok(Value::string(context.arena, String::from("")))
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

            Ok(Value::string(context.arena, substring))
        }
    }
}

pub fn fn_contains<'a>(
    context: FunctionContext<'a, '_>,
    args: &'a Value<'a>,
) -> Result<&'a Value<'a>> {
    let str_value = &args[0];
    let token_value = &args[1];

    if str_value.is_undefined() {
        return Ok(Value::undefined());
    }

    assert_arg!(str_value.is_string(), context, 1);
    assert_arg!(token_value.is_string(), context, 2);

    let str_value = str_value.as_str();
    let token_value = token_value.as_str();

    Ok(Value::bool(
        context.arena,
        str_value.contains(&token_value.to_string()),
    ))
}

pub fn fn_abs<'a>(context: FunctionContext<'a, '_>, args: &'a Value<'a>) -> Result<&'a Value<'a>> {
    let arg = &args[0];

    if arg.is_undefined() {
        return Ok(Value::undefined());
    }

    assert_arg!(arg.is_number(), context, 1);

    Ok(Value::number(context.arena, arg.as_f64().abs()))
}

pub fn fn_floor<'a>(
    context: FunctionContext<'a, '_>,
    args: &'a Value<'a>,
) -> Result<&'a Value<'a>> {
    let arg = &args[0];

    if arg.is_undefined() {
        return Ok(Value::undefined());
    }

    assert_arg!(arg.is_number(), context, 1);

    Ok(Value::number(context.arena, arg.as_f64().floor()))
}

pub fn fn_ceil<'a>(context: FunctionContext<'a, '_>, args: &'a Value<'a>) -> Result<&'a Value<'a>> {
    let arg = &args[0];

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
    args: &'a Value<'a>,
) -> Result<&'a Value<'a>> {
    let input = &args[0];
    let key = &args[1];
    assert_arg!(key.is_string(), context, 2);
    Ok(fn_lookup_internal(context.clone(), input, &key.as_str()))
}

pub fn fn_count<'a>(
    context: FunctionContext<'a, '_>,
    args: &'a Value<'a>,
) -> Result<&'a Value<'a>> {
    max_args!(context, args, 1);

    let arg = &args[0];

    Ok(Value::number(
        context.arena,
        if arg.is_undefined() {
            0.0
        } else if arg.is_array() {
            arg.len() as f64
        } else {
            1.0
        },
    ))
}

pub fn fn_max<'a>(context: FunctionContext<'a, '_>, args: &'a Value<'a>) -> Result<&'a Value<'a>> {
    max_args!(context, args, 1);

    let arg = &args[0];

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

pub fn fn_min<'a>(context: FunctionContext<'a, '_>, args: &'a Value<'a>) -> Result<&'a Value<'a>> {
    max_args!(context, args, 1);

    let arg = &args[0];

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

pub fn fn_sum<'a>(context: FunctionContext<'a, '_>, args: &'a Value<'a>) -> Result<&'a Value<'a>> {
    max_args!(context, args, 1);

    let arg = &args[0];

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
    args: &'a Value<'a>,
) -> Result<&'a Value<'a>> {
    max_args!(context, args, 1);

    let arg = &args[0];

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

pub fn fn_exists<'a>(
    context: FunctionContext<'a, '_>,
    args: &'a Value<'a>,
) -> Result<&'a Value<'a>> {
    min_args!(context, args, 1);
    max_args!(context, args, 1);

    let arg = &args[0];

    match arg {
        Value::Undefined => Ok(Value::bool(context.arena, false)),
        _ => Ok(Value::bool(context.arena, true)),
    }
}

pub fn fn_assert<'a>(
    context: FunctionContext<'a, '_>,
    args: &'a Value<'a>,
) -> Result<&'a Value<'a>> {
    let condition = &args[0];
    let message = &args[1];

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
    args: &'a Value<'a>,
) -> Result<&'a Value<'a>> {
    let message = &args[0];

    assert_arg!(message.is_undefined() || message.is_string(), context, 1);

    Err(Error::D3137Error(if message.is_string() {
        message.as_str().to_string()
    } else {
        "$error() function evaluated".to_string()
    }))
}

pub fn fn_length<'a>(
    context: FunctionContext<'a, '_>,
    args: &'a Value<'a>,
) -> Result<&'a Value<'a>> {
    max_args!(context, args, 1);

    let arg1 = &args[0];

    if arg1.is_undefined() {
        return Ok(Value::undefined());
    }

    assert_arg!(arg1.is_string(), context, 1);

    Ok(Value::number(
        context.arena,
        arg1.as_str().chars().count() as f64,
    ))
}

pub fn fn_sqrt<'a>(context: FunctionContext<'a, '_>, args: &'a Value<'a>) -> Result<&'a Value<'a>> {
    max_args!(context, args, 1);

    let arg1 = &args[0];

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
    args: &'a Value<'a>,
) -> Result<&'a Value<'a>> {
    max_args!(context, args, 2);

    let number = &args[0];
    let exp = &args[1];

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
    args: &'a Value<'a>,
) -> Result<&'a Value<'a>> {
    max_args!(context, args, 1);

    let arr = &args[0];

    if arr.is_undefined() {
        return Ok(Value::undefined());
    }

    assert_arg!(arr.is_array(), context, 1);

    let result = Value::array_with_capacity(context.arena, arr.len(), ArrayFlags::empty());
    arr.members().rev().for_each(|member| result.push(member));
    Ok(result)
}

pub fn fn_join<'a>(context: FunctionContext<'a, '_>, args: &'a Value<'a>) -> Result<&'a Value<'a>> {
    max_args!(context, args, 2);

    let strings = &args[0];

    if strings.is_undefined() {
        return Ok(Value::undefined());
    }

    if strings.is_string() {
        return Ok(strings);
    }

    assert_array_of_type!(strings.is_array(), context, 1, "string");

    let separator = &args[1];
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

    Ok(Value::string(context.arena, result))
}

pub fn fn_sort<'a, 'e>(
    context: FunctionContext<'a, 'e>,
    args: &'a Value<'a>,
) -> Result<&'a Value<'a>> {
    max_args!(context, args, 2);

    let arr = &args[0];

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
    let sorted = if args[1].is_undefined() {
        merge_sort(
            unsorted,
            &|a: &'a Value<'a>, b: &'a Value<'a>| match (a, b) {
                (Value::Number(a), Value::Number(b)) => Ok(a > b),
                (Value::String(a), Value::String(b)) => Ok(a > b),
                _ => Err(Error::D3070InvalidDefaultSort(context.char_index)),
            },
        )?
    } else {
        let comparator = args.get_member(1);
        assert_arg!(comparator.is_function(), context, 2);
        merge_sort(unsorted, &|a: &'a Value<'a>, b: &'a Value<'a>| {
            let args = Value::array_with_capacity(context.arena, 2, ArrayFlags::empty());
            args.push(a);
            args.push(b);
            let result = context.evaluate_function(comparator, args)?;
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
    args: &'a Value<'a>,
) -> Result<&'a Value<'a>> {
    max_args!(context, args, 1);
    let arg = &args[0];
    if arg.is_undefined() {
        return Ok(Value::undefined());
    }
    assert_arg!(arg.is_string(), context, 1);

    let base64 = base64::engine::general_purpose::STANDARD;

    let encoded = base64.encode(arg.as_str().as_bytes());

    Ok(Value::string(context.arena, encoded))
}

pub fn fn_base64_decode<'a>(
    context: FunctionContext<'a, '_>,
    args: &'a Value<'a>,
) -> Result<&'a Value<'a>> {
    max_args!(context, args, 1);
    let arg = &args[0];
    if arg.is_undefined() {
        return Ok(Value::undefined());
    }
    assert_arg!(arg.is_string(), context, 1);

    let base64 = base64::engine::general_purpose::STANDARD;

    let decoded = base64.decode(arg.as_str().as_bytes());
    let data = decoded.map_err(|e| Error::D3137Error(e.to_string()))?;
    let decoded = String::from_utf8(data).map_err(|e| Error::D3137Error(e.to_string()))?;

    Ok(Value::string(context.arena, decoded))
}
