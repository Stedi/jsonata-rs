#![cfg(test)]
extern crate test_generator;

use bumpalo::Bump;
use jsonata_rs::{ArrayFlags, JsonAta, Value};
use std::fs;
use std::path;

include!(concat!(env!("OUT_DIR"), "/generated_tests.rs"));

fn test_case(resource: &str) {
    let arena = Bump::new();
    let test_jsonata = JsonAta::new(
        &fs::read_to_string(path::Path::new(resource)).unwrap(),
        &arena,
    )
    .unwrap();
    let test = test_jsonata.evaluate(None, None).unwrap();
    let test = Value::wrap_in_array_if_needed(&arena, test, ArrayFlags::empty());

    for case in test.members() {
        let timelimit = &case["timelimit"];
        let timelimit = if timelimit.is_integer() {
            Some(timelimit.as_usize())
        } else {
            None
        };

        let depth = &case["depth"];
        let depth = if depth.is_integer() {
            Some(depth.as_usize())
        } else {
            None
        };

        let expr = &case["expr"];
        let expr_file = &case["expr-file"];

        let expr = if expr.is_string() {
            expr.as_str().to_string()
        } else if expr_file.is_string() {
            fs::read_to_string(
                path::Path::new(resource)
                    .parent()
                    .unwrap()
                    .join(expr_file.as_str().to_string()),
            )
            .unwrap()
        } else {
            panic!("No expression")
        };

        eprintln!("EXPR: {expr}");

        let data = &case["data"];
        let dataset = &case["dataset"];

        let data = if dataset.is_string() {
            let dataset = format!("tests/testsuite/datasets/{}.json", dataset.as_str());
            fs::read_to_string(&dataset).unwrap()
        } else if data.is_undefined() {
            "".to_string()
        } else {
            data.serialize(false)
        };

        let jsonata = JsonAta::new(&expr, &arena);

        match jsonata {
            Ok(jsonata) => {
                if case["bindings"].is_object() {
                    for (key, value) in case["bindings"].entries() {
                        jsonata.assign_var(key, value);
                    }
                }

                let data = if data.is_empty() {
                    None
                } else {
                    Some(data.as_str())
                };

                let result = jsonata.evaluate_timeboxed(data, depth, timelimit);

                match result {
                    Ok(result) => {
                        let expected_result = &case["result"];

                        if case["undefinedResult"] == true {
                            assert!(result.is_undefined());
                        } else if case["unordered"] == true {
                            // Some test cases specify that the expected array result can be unordered
                            // because the order is implementation dependent. To implement that here
                            // we do a pretty bad O(n^2) just to see if the test passes.
                            assert!(expected_result.is_array());
                            assert!(result.is_array());
                            for expected_member in expected_result.members() {
                                let mut found = false;
                                for member in result.members() {
                                    if member == expected_member {
                                        found = true;
                                        break;
                                    }
                                }
                                assert!(found);
                            }
                        } else if case["result_re"].is_string() {
                            // Ability to define a regular expression to match the result. This strategy is useful
                            // to validate the result of an expression that's not deterministic (like the $millis() function).
                            let regex_pattern =
                                regress::Regex::with_flags(case["result_re"].as_str().as_ref(), "")
                                    .expect("Should have a valid regex expression");

                            assert!(
                                regex_pattern.find(&result.as_str()).is_some(),
                                "Value: {result:?}, did not match expected result_re",
                            );
                        } else {
                            eprintln!("RESULT: {}", result);
                            assert_eq!(result, expected_result);
                        }
                    }
                    Err(error) => {
                        eprintln!("ERROR: {}", error);
                        let code = if !case["error"].is_undefined() {
                            &case["error"]["code"]
                        } else {
                            &case["code"]
                        };
                        assert_eq!(*code, error.code());
                    }
                }
            }
            Err(error) => {
                eprintln!("{}", error);
                let code = if !case["error"].is_undefined() {
                    &case["error"]["code"]
                } else {
                    &case["code"]
                };
                assert_eq!(*code, error.code());
            }
        }
    }
}
