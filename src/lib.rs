#![cfg_attr(not(doctest), doc = include_str!("../README.md"))]
use bumpalo::Bump;

mod errors;
mod evaluator;
mod parser;

pub use errors::Error;
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
        implementation: fn(FunctionContext<'a, '_>, &'a Value<'a>) -> Result<&'a Value<'a>>,
    ) {
        self.frame.bind(
            name,
            Value::nativefn(self.arena, name, arity, implementation),
        );
    }

    pub fn evaluate(&self, input: Option<&str>) -> Result<&'a Value<'a>> {
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
        bind_native!("boolean", 1, fn_boolean);
        bind_native!("ceil", 1, fn_ceil);
        bind_native!("count", 1, fn_count);
        bind_native!("error", 1, fn_error);
        bind_native!("exists", 1, fn_exists);
        bind_native!("filter", 2, fn_filter);
        bind_native!("floor", 1, fn_floor);
        bind_native!("join", 2, fn_join);
        bind_native!("length", 1, fn_length);
        bind_native!("lookup", 2, fn_lookup);
        bind_native!("lowercase", 1, fn_lowercase);
        bind_native!("map", 2, fn_map);
        bind_native!("max", 1, fn_max);
        bind_native!("min", 1, fn_min);
        bind_native!("not", 1, fn_not);
        bind_native!("number", 1, fn_number);
        bind_native!("power", 2, fn_power);
        bind_native!("reverse", 1, fn_reverse);
        bind_native!("sort", 2, fn_sort);
        bind_native!("string", 1, fn_string);
        bind_native!("sqrt", 1, fn_sqrt);
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
    use super::*;

    #[test]
    fn register_function_simple() {
        let arena = Bump::new();
        let jsonata = JsonAta::new("$test()", &arena).unwrap();
        jsonata.register_function("test", 0, |ctx, _| Ok(Value::number(ctx.arena, 1)));

        let result = jsonata.evaluate(Some(r#"anything"#));

        assert_eq!(result.unwrap(), Value::number(&arena, 1));
    }

    #[test]
    fn register_function_override_now() {
        let arena = Bump::new();
        let jsonata = JsonAta::new("$now()", &arena).unwrap();
        jsonata.register_function("now", 0, |ctx, _| {
            Ok(Value::string(ctx.arena, "time for tea"))
        });

        let result = jsonata.evaluate(Some(r#"anything"#));

        assert_eq!(result.unwrap(), Value::string(&arena, "time for tea"));
    }

    #[test]
    fn register_function_map_squareroot() {
        let arena = Bump::new();
        let jsonata = JsonAta::new("$map([1,4,9,16], $squareroot)", &arena).unwrap();
        jsonata.register_function("squareroot", 1, |ctx, args| {
            let num = &args[0];
            return Ok(Value::number(ctx.arena, (num.as_f64()).sqrt()));
        });

        let result = jsonata.evaluate(Some(r#"anything"#));

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
        jsonata.register_function("even", 1, |ctx, args| {
            let num = &args[0];
            return Ok(Value::bool(ctx.arena, (num.as_f64()) % 2.0 == 0.0));
        });

        let result = jsonata.evaluate(Some(r#"anything"#));

        assert_eq!(
            result
                .unwrap()
                .members()
                .map(|v| v.as_f64())
                .collect::<Vec<f64>>(),
            vec![4.0, 16.0]
        );
    }
}
