#![cfg_attr(not(doctest), doc = include_str!("../README.md"))]
use std::collections::HashMap;

use bumpalo::Bump;

mod datetime;
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
            frame: Frame::new(arena),
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
        match json_value {
            serde_json::Value::Null => Value::null(self.arena),
            serde_json::Value::Bool(b) => self.arena.alloc(Value::Bool(*b)),
            serde_json::Value::Number(n) => Value::number(self.arena, n.as_f64().unwrap()),
            serde_json::Value::String(s) => Value::string(self.arena, s),

            serde_json::Value::Array(a) => {
                let array = Value::array_with_capacity(self.arena, a.len(), ArrayFlags::empty());
                for v in a.iter() {
                    array.push(self.json_value_to_value(v))
                }

                array
            }
            serde_json::Value::Object(o) => {
                let object = Value::object_with_capacity(self.arena, o.len());
                for (k, v) in o.iter() {
                    object.insert(k, self.json_value_to_value(v));
                }
                object
            }
        }
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
                evaluator.evaluate(&input_ast, Value::undefined(), &Frame::new(self.arena))?
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
        bind_native!("fromMillis", 3, from_millis);
        bind_native!("toMillis", 2, to_millis);
        bind_native!("single", 2, single);
        bind_native!("filter", 2, fn_filter);
        bind_native!("floor", 1, fn_floor);
        bind_native!("join", 2, fn_join);
        bind_native!("keys", 1, fn_keys);
        bind_native!("length", 1, fn_length);
        bind_native!("lookup", 2, fn_lookup);
        bind_native!("lowercase", 1, fn_lowercase);
        bind_native!("map", 2, fn_map);
        bind_native!("match", 2, fn_match);
        bind_native!("max", 1, fn_max);
        bind_native!("merge", 1, fn_merge);
        bind_native!("min", 1, fn_min);
        bind_native!("not", 1, fn_not);
        bind_native!("now", 2, fn_now);
        bind_native!("number", 1, fn_number);
        bind_native!("pad", 2, fn_pad);
        bind_native!("power", 2, fn_power);
        bind_native!("random", 0, fn_random);
        bind_native!("reduce", 3, fn_reduce);
        bind_native!("replace", 4, fn_replace);
        bind_native!("reverse", 1, fn_reverse);
        bind_native!("round", 2, fn_round);
        bind_native!("sort", 2, fn_sort);
        bind_native!("split", 3, fn_split);
        bind_native!("sqrt", 1, fn_sqrt);
        bind_native!("string", 1, fn_string);
        bind_native!("substring", 3, fn_substring);
        bind_native!("substringBefore", 2, fn_substring_before);
        bind_native!("substringAfter", 2, fn_substring_after);
        bind_native!("sum", 1, fn_sum);
        bind_native!("trim", 1, fn_trim);
        bind_native!("uppercase", 1, fn_uppercase);
        bind_native!("zip", 1, fn_zip);
        bind_native!("millis", 0, fn_millis);
        bind_native!("uuid", 0, fn_uuid);

        let chain_ast = Some(parser::parse(
            "function($f, $g) { function($x){ $g($f($x)) } }",
        )?);
        let evaluator = Evaluator::new(chain_ast, self.arena, max_depth, time_limit);
        evaluator.evaluate(&self.ast, input, &self.frame)
    }
}

#[cfg(test)]
mod tests {
    use chrono::{DateTime, Datelike, Offset, Utc};
    use regress::Regex;

    use bumpalo::collections::String as BumpString;

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
            Ok(Value::number(ctx.arena, (num.as_f64()).sqrt()))
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
            Ok(Value::bool((num.as_f64()) % 2.0 == 0.0))
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

        // Adjust the regex to allow both lowercase and uppercase AM/PM
        let expected_format =
            Regex::new(r"^\d{2}/\d{2}/\d{4} \d{1,2}:\d{2}(AM|PM|am|pm) GMT-05:00$").unwrap();

        // Check if the pattern exists within the result_str
        let is_match = expected_format.find_iter(&result_str).next().is_some();
        assert!(
            is_match,
            "Expected custom formatted time with timezone, got: {}",
            result_str
        );
    }

    #[test]
    fn test_now_with_valid_format_but_no_timezone() {
        let arena = Bump::new();
        let jsonata = JsonAta::new("$now('[M01]/[D01]/[Y0001] [h#1]:[m01][P]')", &arena).unwrap();
        let result = jsonata.evaluate(None, None).unwrap();
        let result_str = result.as_str();

        let expected_format =
            Regex::new(r"^\d{2}/\d{2}/\d{4} \d{1,2}:\d{2}(AM|PM|am|pm)$").unwrap();

        // Allow both AM/PM and am/pm in the regex
        let is_match = expected_format.find_iter(&result_str).next().is_some();
        assert!(
            is_match,
            "Expected custom formatted time without timezone, got: {}",
            result_str
        );
    }

    #[test]
    fn test_now_with_invalid_timezone() {
        let arena = Bump::new();
        let jsonata = JsonAta::new("$now('', 'invalid')", &arena).unwrap();
        let result = jsonata.evaluate(None, None);

        // Ensure that the evaluation results in an error
        assert!(
            result.is_err(),
            "Expected a runtime error for invalid timezone"
        );

        // Get the error and check if it's the correct one
        if let Err(err) = result {
            // We expect a T0410ArgumentNotValid error
            assert_eq!(
                err.to_string(),
                "T0410 @ 2: Argument 1 of function now does not match function signature"
            );
        }
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

        // Call $now() with more than two arguments (this should cause an error due to max_args! constraint)
        let jsonata = JsonAta::new("$now('', '-0500', 'extra')", &arena).unwrap();
        let result = jsonata.evaluate(None, None);

        // Ensure that an error is returned for too many arguments
        assert!(
            result.is_err(),
            "Expected an error due to too many arguments, but got a result"
        );

        if let Err(e) = result {
            // You can also add an assertion to ensure the correct error type is returned
            assert!(
                matches!(e, Error::T0410ArgumentNotValid { .. }),
                "Expected TooManyArguments error, but got: {:?}",
                e
            );
        }
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

    #[test]
    fn test_custom_format_with_components() {
        let arena = Bump::new();

        // 12-hour format with AM/PM and timezone
        let jsonata = JsonAta::new(
            "$now('[M01]/[D01]/[Y0001] [h#1]:[m01][P] [z]', '-0500')",
            &arena,
        )
        .unwrap();
        let result = jsonata.evaluate(None, None).unwrap();
        let result_str = result.as_str();

        println!("Formatted date: {}", result_str);

        // Create the regex with regress::Regex
        let expected_format =
            Regex::new(r"^\d{2}/\d{2}/\d{4} \d{1,2}:\d{2}(am|pm|AM|PM) GMT-05:00$").unwrap();

        // Check if the pattern exists within result_str using find_iter
        let is_match = expected_format.find_iter(&result_str).next().is_some();
        assert!(
            is_match,
            "Expected 12-hour format with timezone, got: {}",
            result_str
        );
    }

    #[test]
    fn test_now_with_invalid_timezone_should_fail() {
        let arena = Bump::new();
        let jsonata = JsonAta::new("$now('', 'invalid')", &arena).unwrap();
        let result = jsonata.evaluate(None, None);

        assert!(
            result.is_err(),
            "Expected a runtime error for invalid timezone"
        );

        let error = result.unwrap_err();
        assert_eq!(
            error.to_string(),
            "T0410 @ 2: Argument 1 of function now does not match function signature"
        );
    }

    #[test]
    fn test_now_invalid_timezone_with_format() {
        let arena = Bump::new();
        let jsonata = JsonAta::new("$now('[h]:[m]:[s]', 'xx')", &arena).unwrap();
        let result = jsonata.evaluate(None, None);

        assert!(
            result.is_err(),
            "Expected a runtime error for invalid timezone"
        );
        if let Err(err) = result {
            assert_eq!(
                err.to_string(),
                "T0410 @ 2: Argument 1 of function now does not match function signature"
            );
        }
    }

    #[test]
    fn test_now_invalid_format_with_timezone() {
        let arena = Bump::new();
        let jsonata = JsonAta::new("$now('[h01]:[m01]:[s01]', 'xx')", &arena).unwrap();
        let result = jsonata.evaluate(None, None);

        assert!(
            result.is_err(),
            "Expected a runtime error for invalid format"
        );
        if let Err(err) = result {
            assert_eq!(
                err.to_string(),
                "T0410 @ 2: Argument 1 of function now does not match function signature"
            );
        }
    }

    #[test]
    fn test_from_millis_with_custom_format() {
        let arena = Bump::new();

        // Create an instance of JsonAta with a custom format for the date
        let jsonata = JsonAta::new(
            "$fromMillis(1726148700000, '[M01]/[D01]/[Y0001] [H01]:[m01]:[s01] [z]')",
            &arena,
        )
        .unwrap();

        // Evaluate the expression and unwrap the result
        let result = jsonata.evaluate(None, None).unwrap();
        let result_str = result.as_str();

        println!("Formatted date: {}", result_str);

        // Define the expected format using regress::Regex
        let expected_format =
            Regex::new(r"^\d{2}/\d{2}/\d{4} \d{2}:\d{2}:\d{2} GMT\+\d{2}:\d{2}$").unwrap();

        // Simulate `is_match` by checking if there's at least one match in the string
        let is_match = expected_format.find_iter(&result_str).next().is_some();
        assert!(
            is_match,
            "Expected custom formatted date with timezone, got: {}",
            result_str
        );
    }

    #[test]
    fn test_to_millis_with_custom_format() {
        let arena = Bump::new();

        // Create an instance of JsonAta with a custom format to convert to millis
        let jsonata = JsonAta::new(
            "$toMillis('13/09/2024 13:45:00', '[D01]/[M01]/[Y0001] [H01]:[m01]:[s01]')",
            &arena,
        )
        .unwrap();

        // Evaluate the expression and unwrap the result
        let result = jsonata.evaluate(None, None);

        match result {
            Ok(value) => {
                if value.is_number() {
                    let millis = value.as_f64();

                    // Check if the milliseconds value matches the expected timestamp
                    assert_eq!(
                        millis, 1726235100000.0,
                        "Expected milliseconds for the given date"
                    );
                } else {
                    println!("Result is not a number: {:?}", value);
                    panic!("Expected a number, but got something else.");
                }
            }
            Err(err) => {
                println!("Evaluation error: {:?}", err);
                panic!("Failed to evaluate the expression.");
            }
        }
    }

    #[test]
    fn evaluate_with_reduce() {
        let arena = Bump::new(); // Initialize the memory arena
        let jsonata = JsonAta::new(
            "$reduce([1..5], function($i, $j){$i * $j})", // Example expression
            &arena,
        )
        .unwrap();

        let result = jsonata.evaluate(None, None).unwrap(); // Evaluate the expression

        // Assert that the result is the expected product of 1 * 2 * 3 * 4 * 5 = 120
        assert_eq!(result.as_f64(), 120.0);
    }

    #[test]
    fn evaluate_with_reduce_sum() {
        let arena = Bump::new();
        let jsonata = JsonAta::new(
            "$reduce([1..5], function($i, $j){$i + $j})", // Adding 1 to 5
            &arena,
        )
        .unwrap();

        let result = jsonata.evaluate(None, None).unwrap();

        // Assert that the result is 15 (1 + 2 + 3 + 4 + 5 = 15)
        assert_eq!(result.as_f64(), 15.0);
    }

    #[test]
    fn evaluate_with_reduce_custom_initial_value() {
        let arena = Bump::new();
        let jsonata = JsonAta::new(
            "$reduce([1..5], function($i, $j){$i * $j}, 10)", // Multiply with initial value 10
            &arena,
        )
        .unwrap();

        let result = jsonata.evaluate(None, None).unwrap();

        // Assert that the result is 1200 (10 * 1 * 2 * 3 * 4 * 5 = 1200)
        assert_eq!(result.as_f64(), 1200.0);
    }

    #[test]
    fn evaluate_with_reduce_empty_array() {
        let arena = Bump::new();
        let jsonata = JsonAta::new(
            "$reduce([], function($i, $j){$i + $j})", // Reducing an empty array
            &arena,
        )
        .unwrap();

        let result = jsonata.evaluate(None, None).unwrap();

        // Since the array is empty, the result should be `undefined`
        assert!(result.is_undefined());
    }

    #[test]
    fn evaluate_with_reduce_single_element() {
        let arena = Bump::new();
        let jsonata = JsonAta::new(
            "$reduce([42], function($i, $j){$i + $j})", // Single element array
            &arena,
        )
        .unwrap();

        let result = jsonata.evaluate(None, None).unwrap();

        // Assert that the result is 42 (only one element, so it returns that element)
        assert_eq!(result.as_f64(), 42.0);
    }

    #[test]
    fn evaluate_with_reduce_subtraction() {
        let arena = Bump::new();
        let jsonata = JsonAta::new(
            "$reduce([10, 3, 2], function($i, $j){$i - $j})", // Subtracting elements
            &arena,
        )
        .unwrap();

        let result = jsonata.evaluate(None, None).unwrap();

        // Assert that the result is 5 (10 - 3 - 2 = 5)
        assert_eq!(result.as_f64(), 5.0);
    }

    #[test]
    fn evaluate_with_reduce_initial_value_greater() {
        let arena = Bump::new();
        let jsonata = JsonAta::new(
            "$reduce([1..3], function($i, $j){$i - $j}, 10)", // Initial value is greater than array elements
            &arena,
        )
        .unwrap();

        let result = jsonata.evaluate(None, None).unwrap();

        // Assert that the result is 4 (10 - 1 - 2 - 3 = 4)
        assert_eq!(result.as_f64(), 4.0);
    }

    #[test]
    fn test_match_regex_with_jsonata() {
        let arena = Bump::new();

        // Test case with a valid postal code
        let jsonata = JsonAta::new(r#"$match("123456789", /^[0-9]{9}$/)"#, &arena).unwrap();
        let result = jsonata.evaluate(None, None).unwrap();

        // Expected output: an array with a single match object for "123456789"
        let match_value: &Value = arena.alloc(Value::string(&arena, "123456789"));
        let index_value: &Value = arena.alloc(Value::number(&arena, 0.0));
        let groups_array: &Value = &*arena.alloc(Value::Array(
            bumpalo::collections::Vec::new_in(&arena),
            ArrayFlags::empty(),
        ));

        let mut match_obj = hashbrown::HashMap::with_capacity_in(3, &arena);
        match_obj.insert(BumpString::from_str_in("match", &arena), match_value);
        match_obj.insert(BumpString::from_str_in("index", &arena), index_value);
        match_obj.insert(BumpString::from_str_in("groups", &arena), groups_array);

        let expected_match: &Value = &*arena.alloc(Value::Object(match_obj));

        assert_eq!(
            result,
            &*arena.alloc(Value::Array(
                bumpalo::collections::Vec::from_iter_in([expected_match], &arena),
                ArrayFlags::empty()
            ))
        );

        // Test case with an invalid postal code
        let jsonata_invalid =
            JsonAta::new(r#"$match("12345-6789", /^[0-9]{9}$/)"#, &arena).unwrap();
        let result_invalid = jsonata_invalid.evaluate(None, None).unwrap();

        // Expected output for invalid input: an empty array
        let empty_array: &Value = &*arena.alloc(Value::Array(
            bumpalo::collections::Vec::new_in(&arena), // Empty array for no matches
            ArrayFlags::empty(),
        ));
        assert_eq!(result_invalid, empty_array);
    }

    #[test]
    fn evaluate_expect_type_errors() {
        for expr in [
            "$fromMillis('foo')",
            "$fromMillis(1, 1)",
            "$fromMillis(1, '', 1)",
            "$toMillis(1)",
            "$toMillis('1970-01-01T00:00:00.000Z', 1)",
        ] {
            let arena = Bump::new();
            let jsonata = JsonAta::new(expr, &arena).unwrap();
            let err = jsonata.evaluate(None, None).unwrap_err();

            assert_eq!(err.code(), "T0410", "Expected type error from {expr}");
        }
    }

    #[test]
    fn evaluate_millis_returns_number() {
        let arena = Bump::new();
        let jsonata = JsonAta::new("$millis()", &arena).unwrap();
        let result = jsonata.evaluate(None, None).unwrap();

        assert!(result.is_number());
    }

    #[test]
    fn test_from_millis_formats_date() {
        // Initialize the arena (memory pool) for JSONata
        let arena = Bump::new();

        // Define the JSONata expression for formatting the date
        let jsonata = JsonAta::new("$fromMillis($millis(),'[Y01][M01][D01]')", &arena).unwrap();

        // Evaluate the expression
        let result = jsonata.evaluate(None, None).unwrap();

        // Dynamically compute the expected result
        let now = Utc::now();
        let expected = format!(
            "{:02}{:02}{:02}",
            now.year() % 100, // Last two digits of the year
            now.month(),
            now.day()
        );

        // Store the result of `to_string` in a variable to ensure it lives long enough
        let result_string = result.to_string();
        let actual = result_string.trim_matches('"'); // Trim quotes if present

        // Verify the result matches the expected value
        assert_eq!(actual, expected);
    }
}
