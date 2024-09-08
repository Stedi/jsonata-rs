#![cfg_attr(not(doctest), doc = include_str!("../README.md"))]
use std::collections::HashMap;

use bumpalo::Bump;

mod errors;
mod evaluator;
mod parser;

pub use errors::Error;
pub use evaluator::functions::FunctionContext;
pub use evaluator::value::{ArrayFlags, Value};

use evaluator::{frame::Frame, functions::*, Evaluator};
use parser::ast::Ast;

pub type Result<T> = std::result::Result<T, Error>;

pub struct JsonAta<'a> {
    ast: Ast,
    frame: Frame<'a>,
    arena: &'a Bump,
}

impl<'a> JsonAta<'a> {
    pub fn new(expr: &str, arena: &'a Bump) -> Result<JsonAta<'a>> {
        Ok(Self {
            ast: parser::parse(expr)?,
            frame: Frame::new(),
            arena,
        })
    }

    pub fn ast(&self) -> &Ast {
        &self.ast
    }

    pub fn assign_var(&self, name: &str, value: &'a Value<'a>) {
        self.frame.bind(name, value)
    }

    pub fn register_function(
        &self,
        name: &str,
        arity: usize,
        implementation: fn(FunctionContext<'a, '_>, &[&'a Value<'a>]) -> Result<&'a Value<'a>>,
    ) {
        self.frame.bind(
            name,
            Value::nativefn(self.arena, name, arity, implementation),
        );
    }

    fn json_value_to_value(&self, json_value: &serde_json::Value) -> &'a mut Value<'a> {
        return match json_value {
            serde_json::Value::Null => Value::null(self.arena),
            serde_json::Value::Bool(b) => self.arena.alloc(Value::Bool(*b)),
            serde_json::Value::Number(n) => Value::number(self.arena, n.as_f64().unwrap()),
            serde_json::Value::String(s) => Value::string(self.arena, s),

            serde_json::Value::Array(a) => {
                let array = Value::array_with_capacity(self.arena, a.len(), ArrayFlags::empty());
                for v in a.iter() {
                    array.push(self.json_value_to_value(v))
                }

                return array;
            }
            serde_json::Value::Object(o) => {
                let object = Value::object_with_capacity(self.arena, o.len());
                for (k, v) in o.iter() {
                    object.insert(k, self.json_value_to_value(v));
                }
                return object;
            }
        };
    }

    pub fn evaluate(
        &self,
        input: Option<&str>,
        bindings: Option<&HashMap<&str, &serde_json::Value>>,
    ) -> Result<&'a Value<'a>> {
        if let Some(bindings) = bindings {
            for (key, json_value) in bindings.iter() {
                let value = self.json_value_to_value(json_value);
                self.assign_var(key, value);
            }
        };

        self.evaluate_timeboxed(input, None, None)
    }

    pub fn evaluate_timeboxed(
        &self,
        input: Option<&str>,
        max_depth: Option<usize>,
        time_limit: Option<usize>,
    ) -> Result<&'a Value<'a>> {
        let input = match input {
            Some(input) => {
                let input_ast = parser::parse(input)?;
                let evaluator = Evaluator::new(None, self.arena, None, None);
                evaluator.evaluate(&input_ast, Value::undefined(), &Frame::new())?
            }
            None => Value::undefined(),
        };

        // If the input is an array, wrap it in an array so that it gets treated as a single input
        let input = if input.is_array() {
            Value::wrap_in_array(self.arena, input, ArrayFlags::WRAPPED)
        } else {
            input
        };

        macro_rules! bind_native {
            ($name:literal, $arity:literal, $fn:ident) => {
                self.frame
                    .bind($name, Value::nativefn(&self.arena, $name, $arity, $fn));
            };
        }

        self.frame.bind("$", input);
        bind_native!("abs", 1, fn_abs);
        bind_native!("append", 2, fn_append);
        bind_native!("assert", 2, fn_assert);
        bind_native!("base64decode", 1, fn_base64_decode);
        bind_native!("base64encode", 1, fn_base64_encode);
        bind_native!("boolean", 1, fn_boolean);
        bind_native!("ceil", 1, fn_ceil);
        bind_native!("contains", 2, fn_contains);
        bind_native!("count", 1, fn_count);
        bind_native!("distinct", 1, fn_distinct);
        bind_native!("each", 2, fn_each);
        bind_native!("error", 1, fn_error);
        bind_native!("exists", 1, fn_exists);
        bind_native!("filter", 2, fn_filter);
        bind_native!("floor", 1, fn_floor);
        bind_native!("join", 2, fn_join);
        bind_native!("keys", 1, fn_keys);
        bind_native!("length", 1, fn_length);
        bind_native!("lookup", 2, fn_lookup);
        bind_native!("lowercase", 1, fn_lowercase);
        bind_native!("map", 2, fn_map);
        bind_native!("max", 1, fn_max);
        bind_native!("merge", 1, fn_merge);
        bind_native!("min", 1, fn_min);
        bind_native!("not", 1, fn_not);
        bind_native!("now", 2, fn_now);
        bind_native!("number", 1, fn_number);
        bind_native!("random", 0, fn_random);
        bind_native!("power", 2, fn_power);
        bind_native!("replace", 4, fn_replace);
        bind_native!("reverse", 1, fn_reverse);
        bind_native!("round", 2, fn_round);
        bind_native!("sort", 2, fn_sort);
        bind_native!("split", 3, fn_split);
        bind_native!("sqrt", 1, fn_sqrt);
        bind_native!("string", 1, fn_string);
        bind_native!("substring", 3, fn_substring);
        bind_native!("sum", 1, fn_sum);
        bind_native!("trim", 1, fn_trim);
        bind_native!("uppercase", 1, fn_uppercase);

        let chain_ast = Some(parser::parse(
            "function($f, $g) { function($x){ $g($f($x)) } }",
        )?);
        let evaluator = Evaluator::new(chain_ast, self.arena, max_depth, time_limit);
        evaluator.evaluate(&self.ast, input, &self.frame)
    }
}

#[cfg(test)]
mod tests {
    use chrono::{DateTime, Offset};
    use regex::Regex;

    use super::*;

    #[test]
    fn register_function_simple() {
        let arena = Bump::new();
        let jsonata = JsonAta::new("$test()", &arena).unwrap();
        jsonata.register_function("test", 0, |ctx, _| Ok(Value::number(ctx.arena, 1)));

        let result = jsonata.evaluate(Some(r#"anything"#), None);

        assert_eq!(result.unwrap(), Value::number(&arena, 1));
    }

    #[test]
    fn register_function_override_now() {
        let arena = Bump::new();
        let jsonata = JsonAta::new("$now()", &arena).unwrap();
        jsonata.register_function("now", 0, |ctx, _| {
            Ok(Value::string(ctx.arena, "time for tea"))
        });

        let result = jsonata.evaluate(None, None);

        assert_ne!(result.unwrap().as_str(), "time for tea");
    }

    #[test]
    fn register_function_map_squareroot() {
        let arena = Bump::new();
        let jsonata = JsonAta::new("$map([1,4,9,16], $squareroot)", &arena).unwrap();
        jsonata.register_function("squareroot", 1, |ctx, args| {
            let num = &args[0];
            return Ok(Value::number(ctx.arena, (num.as_f64()).sqrt()));
        });

        let result = jsonata.evaluate(Some(r#"anything"#), None);

        assert_eq!(
            result
                .unwrap()
                .members()
                .map(|v| v.as_f64())
                .collect::<Vec<f64>>(),
            vec![1.0, 2.0, 3.0, 4.0]
        );
    }

    #[test]
    fn register_function_filter_even() {
        let arena = Bump::new();
        let jsonata = JsonAta::new("$filter([1,4,9,16], $even)", &arena).unwrap();
        jsonata.register_function("even", 1, |_ctx, args| {
            let num = &args[0];
            return Ok(Value::bool((num.as_f64()) % 2.0 == 0.0));
        });

        let result = jsonata.evaluate(Some(r#"anything"#), None);

        assert_eq!(
            result
                .unwrap()
                .members()
                .map(|v| v.as_f64())
                .collect::<Vec<f64>>(),
            vec![4.0, 16.0]
        );
    }

    #[test]
    fn evaluate_with_bindings_simple() {
        let arena = Bump::new();
        let jsonata = JsonAta::new("$a + $b", &arena).unwrap();

        let a = &serde_json::Value::Number(serde_json::Number::from(1));
        let b = &serde_json::Value::Number(serde_json::Number::from(2));

        let mut bindings = HashMap::new();
        bindings.insert("a", a);
        bindings.insert("b", b);

        let result = jsonata.evaluate(None, Some(&bindings));

        assert_eq!(result.unwrap().as_f64(), 3.0);
    }

    #[test]
    fn evaluate_with_bindings_nested() {
        let arena = Bump::new();
        let jsonata = JsonAta::new("$foo[0].a + $foo[1].b", &arena).unwrap();

        let foo_string = r#"
            [
                {
                    "a": 1
                },
                {
                    "b": 2
                }
            ]
        "#;

        let foo: serde_json::Value = serde_json::from_str(foo_string).unwrap();

        let mut bindings = HashMap::new();
        bindings.insert("foo", &foo);

        let result = jsonata.evaluate(None, Some(&bindings));

        assert_eq!(result.unwrap().as_f64(), 3.0);
    }

    #[test]
    fn evaluate_with_random() {
        let arena = Bump::new();
        let jsonata = JsonAta::new("$random()", &arena).unwrap();

        let result = jsonata.evaluate(None, None).unwrap();

        assert!(result.as_f64() >= 0.0);
        assert!(result.as_f64() < 1.0);
    }

    #[test]
    fn test_now_default_utc() {
        let arena = Bump::new();
        let jsonata = JsonAta::new("$now()", &arena).unwrap();
        let result = jsonata.evaluate(None, None).unwrap();
        let result_str = result.as_str();

        println!("test_now_default_utc {}", result_str);

        // Check for valid ISO 8601 format and ensure it's in UTC
        let parsed_result = DateTime::parse_from_rfc3339(&result_str)
            .expect("Should parse valid ISO 8601 timestamp");

        assert_eq!(
            parsed_result.offset().fix().local_minus_utc(),
            0,
            "Should be UTC"
        );
    }

    #[test]
    fn test_now_with_valid_timezone_and_format() {
        let arena = Bump::new();
        let jsonata = JsonAta::new(
            "$now('[M01]/[D01]/[Y0001] [h#1]:[m01][P] [z]', '-0500')",
            &arena,
        )
        .unwrap();
        let result = jsonata.evaluate(None, None).unwrap();
        let result_str = result.as_str();

        println!("test_now_with_valid_timezone_and_format {}", result_str);

        // Check for custom formatted time with timezone
        assert!(
            Regex::new(r"^\d{2}/\d{2}/\d{4} \d{1,2}:\d{2}(AM|PM) GMT-05:00$")
                .unwrap()
                .is_match(&result_str),
            "Expected custom formatted time with timezone"
        );
    }

    #[test]
    fn test_now_with_valid_format_but_no_timezone() {
        let arena = Bump::new();
        let jsonata = JsonAta::new("$now('[M01]/[D01]/[Y0001] [h#1]:[m01][P]')", &arena).unwrap();
        let result = jsonata.evaluate(None, None).unwrap();
        let result_str = result.as_str();

        println!("test_now_with_valid_format_but_no_timezone {}", result_str);

        // Check for custom formatted time without timezone
        assert!(
            Regex::new(r"^\d{2}/\d{2}/\d{4} \d{1,2}:\d{2}(AM|PM)$")
                .unwrap()
                .is_match(&result_str),
            "Expected custom formatted time without timezone"
        );
    }

    #[test]
    fn test_now_with_invalid_timezone() {
        let arena = Bump::new();
        let jsonata = JsonAta::new("$now('', 'invalid')", &arena).unwrap();
        let result = jsonata.evaluate(None, None).unwrap();
        let result_str = result.as_str();

        println!("test_now_with_invalid_timezone {}", result_str);

        // Should return empty string for invalid timezone
        assert!(
            result_str.is_empty(),
            "Expected empty string for invalid timezone"
        );
    }

    #[test]
    fn test_now_with_valid_timezone_but_no_format() {
        let arena = Bump::new();
        let jsonata = JsonAta::new("$now('', '-0500')", &arena).unwrap();
        let result = jsonata.evaluate(None, None).unwrap();
        let result_str = result.as_str();

        println!("test_now_with_valid_timezone_but_no_format {}", result_str);

        // Should return empty string for valid timezone but empty format
        assert!(
            result_str.is_empty(),
            "Expected empty string for valid timezone but no format"
        );
    }

    #[test]
    fn test_now_with_too_many_arguments() {
        let arena = Bump::new();
        let jsonata = JsonAta::new("$now('', '-0500', 'extra')", &arena).unwrap();
        let result = jsonata.evaluate(None, None).unwrap();
        let result_str = result.as_str();

        println!("test_now_with_too_many_arguments {}", result_str);

        // Should return an empty string for too many arguments
        assert!(
            result_str.is_empty(),
            "Expected empty string for too many arguments"
        );
    }

    #[test]
    fn test_now_with_edge_case_timezones() {
        let arena = Bump::new();

        // Extreme positive timezone
        let jsonata = JsonAta::new("$now('[H01]:[m01] [z]', '+1440')", &arena).unwrap();
        let result = jsonata.evaluate(None, None).unwrap();
        let result_str = result.as_str();
        println!("test_now_with_extreme_positive_timezone {:?}", result_str);
        assert!(result_str.contains("GMT+14:40"), "Expected GMT+14:40");

        // Edge case: minimal valid timezone
        let jsonata_minimal = JsonAta::new("$now('[H01]:[m01] [z]', '-0000')", &arena).unwrap();
        let result_minimal = jsonata_minimal.evaluate(None, None).unwrap();
        let result_str_minimal = result_minimal.as_str();
        println!("test_now_with_minimal_timezone {:?}", result_str_minimal);

        // Corrected the assertion to check for "GMT+00:00"
        assert!(
            result_str_minimal.contains("GMT+00:00"),
            "Expected GMT+00:00"
        );
    }
}
