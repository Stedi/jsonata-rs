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
        bind_native!("base64encode", 1, fn_base64_encode);
        bind_native!("base64decode", 1, fn_base64_decode);

        let chain_ast = Some(parser::parse(
            "function($f, $g) { function($x){ $g($f($x)) } }",
        )?);
        let evaluator = Evaluator::new(chain_ast, self.arena, max_depth, time_limit);
        evaluator.evaluate(&self.ast, input, &self.frame)
    }
}
