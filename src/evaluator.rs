pub mod frame;
pub mod functions;
pub mod value;

use frame::Frame;
use functions::*;
use value::{ArrayFlags, Value};

use bumpalo::collections::Vec as BumpVec;
use bumpalo::Bump;
use std::cell::RefCell;
use std::collections::{hash_map, HashMap};
use std::time::Instant;

use super::parser::ast::*;
use crate::{Error, Result};

struct EvaluatorInternal {
    depth: usize,
    started_at: Option<Instant>,
    max_depth: Option<usize>,
    time_limit: Option<usize>,
}

pub struct Evaluator<'a> {
    chain_ast: Option<Ast>,
    arena: &'a Bump,
    internal: RefCell<EvaluatorInternal>,
}

impl<'a> Evaluator<'a> {
    pub fn new(
        chain_ast: Option<Ast>,
        arena: &'a Bump,
        max_depth: Option<usize>,
        time_limit: Option<usize>,
    ) -> Self {
        Evaluator {
            chain_ast,
            arena,
            internal: RefCell::new(EvaluatorInternal {
                depth: 0,
                started_at: None,
                max_depth,
                time_limit,
            }),
        }
    }

    fn fn_context<'e>(
        &'e self,
        name: &'a str,
        char_index: usize,
        input: &'a Value<'a>,
        frame: &Frame<'a>,
    ) -> FunctionContext<'a, 'e> {
        FunctionContext {
            name,
            char_index,
            input,
            frame: frame.clone(),
            arena: self.arena,
            evaluator: self,
        }
    }

    fn check_limits(&self, inc_or_dec: bool) -> Result<()> {
        let mut internal = self.internal.borrow_mut();
        internal.depth = if inc_or_dec {
            internal.depth + 1
        } else {
            internal.depth - 1
        };
        if let Some(started_at) = internal.started_at {
            if let Some(time_limit) = internal.time_limit {
                if started_at.elapsed().as_millis() >= time_limit as u128 {
                    return Err(Error::U1001Timeout);
                }
            }
        } else {
            internal.started_at = Some(Instant::now());
        }
        if let Some(max_depth) = internal.max_depth {
            if internal.depth > max_depth {
                return Err(Error::U1001StackOverflow);
            }
        }
        Ok(())
    }

    pub fn evaluate(
        &self,
        node: &Ast,
        input: &'a Value<'a>,
        frame: &Frame<'a>,
    ) -> Result<&'a Value<'a>> {
        self.check_limits(true)?;

        let mut result = match node.kind {
            AstKind::Null => Value::null(self.arena),
            AstKind::Bool(b) => Value::bool(b),
            AstKind::String(ref s) => Value::string(self.arena, s),
            AstKind::Number(n) => Value::number(self.arena, n),
            AstKind::Block(ref exprs) => self.evaluate_block(exprs, input, frame)?,
            AstKind::Unary(ref op) => self.evaluate_unary_op(node, op, input, frame)?,
            AstKind::Binary(ref op, ref lhs, ref rhs) => {
                self.evaluate_binary_op(node, op, lhs, rhs, input, frame)?
            }
            AstKind::Var(ref name) => self.evaluate_var(name, input, frame)?,
            AstKind::Ternary {
                ref cond,
                ref truthy,
                ref falsy,
            } => self.evaluate_ternary(cond, truthy, falsy.as_deref(), input, frame)?,
            AstKind::Path(ref steps) => self.evaluate_path(node, steps, input, frame)?,
            AstKind::Name(ref name) => fn_lookup_internal(
                self.fn_context("lookup", node.char_index, input, frame),
                input,
                name,
            ),
            AstKind::Lambda { .. } => Value::lambda(self.arena, node, input, frame.clone()),
            AstKind::Function {
                ref proc,
                ref args,
                is_partial,
                ..
            } => self.evaluate_function(input, proc, args, is_partial, frame, None)?,
            AstKind::Wildcard => self.evaluate_wildcard(input)?,
            AstKind::Descendent => self.evaluate_descendants(input)?,
            AstKind::Transform {
                ref pattern,
                ref update,
                ref delete,
            } => Value::transformer(self.arena, pattern, update, delete),
            _ => unimplemented!("TODO: node kind not yet supported: {:#?}", node.kind),
        };

        if let Some(filters) = &node.predicates {
            for filter in filters {
                if let AstKind::Filter(ref expr) = filter.kind {
                    result = self.evaluate_filter(expr, result, frame)?
                }
            }
        }

        self.check_limits(false)?;

        Ok(
            if result.has_flags(ArrayFlags::SEQUENCE) && !result.has_flags(ArrayFlags::TUPLE_STREAM)
            {
                if node.keep_array {
                    result = result.clone_array_with_flags(
                        self.arena,
                        result.get_flags() | ArrayFlags::SINGLETON,
                    )
                }
                if result.is_empty() {
                    Value::undefined()
                } else if result.len() == 1 {
                    if result.has_flags(ArrayFlags::SINGLETON) {
                        result
                    } else {
                        result.get_member(0)
                    }
                } else {
                    result
                }
            } else {
                result
            },
        )
    }

    fn evaluate_block(
        &self,
        exprs: &[Ast],
        input: &'a Value<'a>,
        frame: &Frame<'a>,
    ) -> Result<&'a Value<'a>> {
        let frame = Frame::new_with_parent(frame);
        if exprs.is_empty() {
            return Ok(Value::undefined());
        }

        let mut result = Value::undefined();
        for expr in exprs {
            result = self.evaluate(expr, input, &frame)?;
        }

        Ok(result)
    }

    fn evaluate_var(
        &self,
        name: &str,
        input: &'a Value<'a>,
        frame: &Frame<'a>,
    ) -> Result<&'a Value<'a>> {
        Ok(if name.is_empty() {
            if input.has_flags(ArrayFlags::WRAPPED) {
                input.get_member(0)
            } else {
                input
            }
        } else if let Some(value) = frame.lookup(name) {
            value
        } else {
            Value::undefined()
        })
    }

    fn evaluate_unary_op(
        &self,
        node: &Ast,
        op: &UnaryOp,
        input: &'a Value<'a>,
        frame: &Frame<'a>,
    ) -> Result<&'a Value<'a>> {
        match *op {
            UnaryOp::Minus(ref value) => {
                let result = self.evaluate(value, input, frame)?;
                match result {
                    Value::Undefined => Ok(Value::undefined()),
                    Value::Number(n) if result.is_valid_number()? => {
                        Ok(Value::number(self.arena, -n))
                    }
                    _ => Err(Error::D1002NegatingNonNumeric(
                        node.char_index,
                        result.to_string(),
                    )),
                }
            }
            UnaryOp::ArrayConstructor(ref array) => {
                let mut values = BumpVec::new_in(self.arena);
                for item in array {
                    let value = self.evaluate(item, input, frame)?;
                    if let AstKind::Unary(UnaryOp::ArrayConstructor(..)) = item.kind {
                        values.push(value);
                    } else {
                        fn_append_internal(&mut values, value);
                    }
                }

                let flags = if node.cons_array {
                    ArrayFlags::CONS
                } else {
                    ArrayFlags::empty()
                };
                Ok(Value::array_from(self.arena, values, flags))
            }
            UnaryOp::ObjectConstructor(ref object) => {
                self.evaluate_group_expression(node.char_index, object, input, frame)
            }
        }
    }

    fn evaluate_group_expression(
        &self,
        char_index: usize,
        object: &[(Ast, Ast)],
        input: &'a Value<'a>,
        frame: &Frame<'a>,
    ) -> Result<&'a Value<'a>> {
        struct Group<'a> {
            pub data: &'a Value<'a>,
            pub index: usize,
        }

        let mut groups: HashMap<String, Group> = HashMap::new();
        let reduce = input.has_flags(ArrayFlags::TUPLE_STREAM);

        let input = if input.is_array() && input.is_empty() {
            let input = Value::array_with_capacity(self.arena, 1, input.get_flags());
            input.push(Value::undefined());
            input
        } else if !input.is_array() {
            let wrapped = Value::array_with_capacity(self.arena, 1, ArrayFlags::SEQUENCE);
            wrapped.push(input);
            wrapped
        } else {
            input
        };

        for item in input.members() {
            let tuple_frame = if reduce {
                Some(Frame::from_tuple(frame, item))
            } else {
                None
            };

            for (index, pair) in object.iter().enumerate() {
                let key = if reduce {
                    self.evaluate(&pair.0, &item["@"], tuple_frame.as_ref().unwrap())?
                } else {
                    self.evaluate(&pair.0, item, frame)?
                };
                if !key.is_string() {
                    return Err(Error::T1003NonStringKey(char_index, key.to_string()));
                }

                let key = key.as_str();

                match groups.entry(key.to_string()) {
                    hash_map::Entry::Occupied(mut entry) => {
                        let group = entry.get_mut();
                        if group.index != index {
                            return Err(Error::D1009MultipleKeys(char_index, key.to_string()));
                        }
                        group.data = fn_append(
                            self.fn_context("append", char_index, input, frame),
                            &[group.data, item],
                        )?;
                    }
                    hash_map::Entry::Vacant(entry) => {
                        entry.insert(Group { data: item, index });
                    }
                };
            }
        }

        let result = Value::object(self.arena);

        for key in groups.keys() {
            let group = groups.get(key).unwrap();
            let value = if reduce {
                let tuple = self.reduce_tuple_stream(char_index, group.data, input, frame)?;
                let context = tuple.get_entry("@");
                // TODO: Do we need this? JSONata does this, but it's difficult with the mutability
                // of our values.
                // tuple.remove_entry("@");
                let tuple_frame = Frame::from_tuple(frame, tuple);
                self.evaluate(&object[group.index].1, context, &tuple_frame)?
            } else {
                self.evaluate(&object[group.index].1, group.data, frame)?
            };
            if !value.is_undefined() {
                result.insert(key, value);
            }
        }

        Ok(result)
    }

    fn reduce_tuple_stream(
        &self,
        char_index: usize,
        tuple_stream: &'a Value<'a>,
        input: &'a Value<'a>,
        frame: &Frame<'a>,
    ) -> Result<&'a Value<'a>> {
        if !tuple_stream.is_array() {
            return Ok(tuple_stream);
        }

        let result = Value::object(self.arena);
        for (key, value) in tuple_stream[0].entries() {
            result.insert(key, value);
        }
        for i in 1..tuple_stream.len() {
            for (key, value) in tuple_stream[i].entries() {
                let new_value = fn_append(
                    self.fn_context("append", char_index, input, frame),
                    &[result.get_entry(&key[..]), value],
                )?;
                result.insert(key, new_value);
            }
        }

        Ok(result)
    }

    fn evaluate_binary_op(
        &self,
        node: &Ast,
        op: &BinaryOp,
        lhs_ast: &Ast,
        rhs_ast: &Ast,
        input: &'a Value<'a>,
        frame: &Frame<'a>,
    ) -> Result<&'a Value<'a>> {
        if *op == BinaryOp::Bind {
            if let AstKind::Var(ref name) = lhs_ast.kind {
                let rhs = self.evaluate(rhs_ast, input, frame)?;
                frame.bind(name, rhs);
                return Ok(rhs);
            }
            unreachable!()
        }

        // NOTE: rhs is not evaluated until absolutely necessary to support short circuiting
        // of boolean expressions.
        let lhs = self.evaluate(lhs_ast, input, frame)?;

        match op {
            BinaryOp::Add
            | BinaryOp::Subtract
            | BinaryOp::Multiply
            | BinaryOp::Divide
            | BinaryOp::Modulus => {
                let rhs = self.evaluate(rhs_ast, input, frame)?;

                let lhs = if lhs.is_undefined() {
                    return Ok(Value::undefined());
                } else if lhs.is_valid_number()? {
                    lhs.as_f64()
                } else {
                    return Err(Error::T2001LeftSideNotNumber(
                        node.char_index,
                        op.to_string(),
                    ));
                };

                let rhs = if rhs.is_undefined() {
                    return Ok(Value::undefined());
                } else if rhs.is_valid_number()? {
                    rhs.as_f64()
                } else {
                    return Err(Error::T2002RightSideNotNumber(
                        node.char_index,
                        op.to_string(),
                    ));
                };

                let result = match op {
                    BinaryOp::Add => lhs + rhs,
                    BinaryOp::Subtract => lhs - rhs,
                    BinaryOp::Multiply => lhs * rhs,
                    BinaryOp::Divide => lhs / rhs,
                    BinaryOp::Modulus => lhs % rhs,
                    _ => unreachable!(),
                };

                Ok(Value::number(self.arena, result))
            }

            BinaryOp::LessThan
            | BinaryOp::LessThanEqual
            | BinaryOp::GreaterThan
            | BinaryOp::GreaterThanEqual => {
                let rhs = self.evaluate(rhs_ast, input, frame)?;

                if lhs.is_undefined() || rhs.is_undefined() {
                    return Ok(Value::undefined());
                }

                if !((lhs.is_number() || lhs.is_string()) && (rhs.is_number() || rhs.is_string())) {
                    return Err(Error::T2010BinaryOpTypes(node.char_index, op.to_string()));
                }

                if lhs.is_number() && rhs.is_number() {
                    let lhs = lhs.as_f64();
                    let rhs = rhs.as_f64();
                    return Ok(Value::bool(match op {
                        BinaryOp::LessThan => lhs < rhs,
                        BinaryOp::LessThanEqual => lhs <= rhs,
                        BinaryOp::GreaterThan => lhs > rhs,
                        BinaryOp::GreaterThanEqual => lhs >= rhs,
                        _ => unreachable!(),
                    }));
                }

                if let (Value::String(ref lhs), Value::String(ref rhs)) = (lhs, rhs) {
                    return Ok(Value::bool(match op {
                        BinaryOp::LessThan => lhs < rhs,
                        BinaryOp::LessThanEqual => lhs <= rhs,
                        BinaryOp::GreaterThan => lhs > rhs,
                        BinaryOp::GreaterThanEqual => lhs >= rhs,
                        _ => unreachable!(),
                    }));
                }

                Err(Error::T2009BinaryOpMismatch(
                    node.char_index,
                    lhs.to_string(),
                    rhs.to_string(),
                    op.to_string(),
                ))
            }

            BinaryOp::Equal | BinaryOp::NotEqual => {
                let rhs = self.evaluate(rhs_ast, input, frame)?;

                if lhs.is_undefined() || rhs.is_undefined() {
                    return Ok(Value::bool(false));
                }

                Ok(Value::bool(match op {
                    BinaryOp::Equal => lhs == rhs,
                    BinaryOp::NotEqual => lhs != rhs,
                    _ => unreachable!(),
                }))
            }

            BinaryOp::Range => {
                let rhs = self.evaluate(rhs_ast, input, frame)?;

                if !lhs.is_undefined() && !lhs.is_integer() {
                    return Err(Error::T2003LeftSideNotInteger(node.char_index));
                };

                if !rhs.is_undefined() && !rhs.is_integer() {
                    return Err(Error::T2004RightSideNotInteger(node.char_index));
                }

                if lhs.is_undefined() || rhs.is_undefined() {
                    return Ok(Value::undefined());
                }

                let lhs = lhs.as_isize();
                let rhs = rhs.as_isize();

                if lhs > rhs {
                    return Ok(Value::undefined());
                }

                let size = rhs - lhs + 1;
                if size > 10_000_000 {
                    return Err(Error::D2014RangeOutOfBounds(node.char_index, size));
                }

                Ok(Value::range(self.arena, lhs, rhs))
            }

            BinaryOp::Concat => {
                let rhs = self.evaluate(rhs_ast, input, frame)?;
                let mut result = String::new();
                if !lhs.is_undefined() {
                    result.push_str(
                        &fn_string(
                            self.fn_context("string", node.char_index, input, frame),
                            &[lhs],
                        )?
                        .as_str(),
                    );
                }
                if !rhs.is_undefined() {
                    result.push_str(
                        &fn_string(
                            self.fn_context("string", node.char_index, input, frame),
                            &[rhs],
                        )?
                        .as_str(),
                    );
                }
                Ok(Value::string(self.arena, &result))
            }

            BinaryOp::And => Ok(Value::bool(
                lhs.is_truthy() && self.evaluate(rhs_ast, input, frame)?.is_truthy(),
            )),

            BinaryOp::Or => Ok(Value::bool(
                lhs.is_truthy() || self.evaluate(rhs_ast, input, frame)?.is_truthy(),
            )),

            BinaryOp::Apply => {
                if let AstKind::Function {
                    ref proc,
                    ref args,
                    is_partial,
                    ..
                } = rhs_ast.kind
                {
                    // Function invocation with lhs as the first argument
                    Ok(self.evaluate_function(input, proc, args, is_partial, frame, Some(lhs))?)
                } else {
                    let rhs = self.evaluate(rhs_ast, input, frame)?;

                    if !rhs.is_function() {
                        return Err(Error::T2006RightSideNotFunction(rhs_ast.char_index));
                    }

                    if lhs.is_function() {
                        // Apply function chaining
                        let chain = self.evaluate(
                            self.chain_ast.as_ref().unwrap(),
                            Value::undefined(),
                            frame,
                        )?;

                        Ok(self.apply_function(
                            lhs_ast.char_index,
                            Value::undefined(),
                            chain,
                            &[lhs, rhs],
                            frame,
                        )?)
                    } else {
                        Ok(self.apply_function(
                            rhs_ast.char_index,
                            Value::undefined(),
                            rhs,
                            &[lhs],
                            frame,
                        )?)
                    }
                }
            }

            BinaryOp::In => {
                let rhs = self.evaluate(rhs_ast, input, frame)?;

                if lhs.is_undefined() || rhs.is_undefined() {
                    return Ok(Value::bool(false));
                }

                let rhs = Value::wrap_in_array_if_needed(self.arena, rhs, ArrayFlags::empty());

                for item in rhs.members() {
                    if item == lhs {
                        return Ok(Value::bool(true));
                    }
                }

                Ok(Value::bool(false))
            }

            _ => unimplemented!("TODO: binary op not supported yet: {:#?}", *op),
        }
    }

    fn evaluate_ternary(
        &self,
        cond: &Ast,
        truthy: &Ast,
        falsy: Option<&Ast>,
        input: &'a Value<'a>,
        frame: &Frame<'a>,
    ) -> Result<&'a Value<'a>> {
        let cond = self.evaluate(cond, input, frame)?;
        if cond.is_truthy() {
            self.evaluate(truthy, input, frame)
        } else if let Some(falsy) = falsy {
            self.evaluate(falsy, input, frame)
        } else {
            Ok(Value::undefined())
        }
    }

    fn evaluate_path(
        &self,
        node: &Ast,
        steps: &[Ast],
        input: &'a Value<'a>,
        frame: &Frame<'a>,
    ) -> Result<&'a Value<'a>> {
        // Turn the input into an array if it's not already.
        //
        // If the first step is a variable reference, then the path is absolute rather than
        // relative.
        let mut input = if input.is_array() && !matches!(steps[0].kind, AstKind::Var(..)) {
            input
        } else {
            Value::wrap_in_array(self.arena, input, ArrayFlags::SEQUENCE)
        };

        let mut result = Value::undefined();
        let mut is_tuple_stream = false;
        let mut tuple_bindings = Value::undefined();

        for (step_index, step) in steps.iter().enumerate() {
            // If any step is marked as a tuple, then we have to deal with a tuple stream
            if step.tuple {
                is_tuple_stream = true;
            }

            // If the first step is an explicit array constructor, then just evaluate that
            // (i.e. don't iterate over a context array)
            if step_index == 0 && step.cons_array {
                result = self.evaluate(step, input, frame)?;
            } else if is_tuple_stream {
                tuple_bindings = self.evaluate_tuple_step(step, input, tuple_bindings, frame)?;
            } else {
                result = self.evaluate_step(step, input, frame, step_index == steps.len() - 1)?;
            }

            // If any step results in undefined or an empty array, we can break out as
            // no further steps will produce any results
            if !is_tuple_stream
                && (result.is_undefined() || (result.is_array() && result.is_empty()))
            {
                break;
            }

            input = result
        }

        if is_tuple_stream {
            if node.tuple {
                result = tuple_bindings;
            } else {
                let new_result = Value::array_with_capacity(
                    self.arena,
                    tuple_bindings.len(),
                    ArrayFlags::SEQUENCE,
                );
                for binding in tuple_bindings.members() {
                    new_result.push(binding.get_entry("@"));
                }
                result = new_result;
            }
        }

        if node.keep_singleton_array {
            let flags = result.get_flags();
            if flags.contains(ArrayFlags::CONS) && !flags.contains(ArrayFlags::SEQUENCE) {
                result = Value::wrap_in_array(
                    self.arena,
                    result,
                    flags | ArrayFlags::SEQUENCE | ArrayFlags::SINGLETON,
                );
            }
            result = result.clone_array_with_flags(self.arena, flags | ArrayFlags::SINGLETON);
        }

        if let Some((char_index, ref object)) = node.group_by {
            self.evaluate_group_expression(
                char_index,
                object,
                if is_tuple_stream {
                    tuple_bindings
                } else {
                    result
                },
                frame,
            )
        } else {
            Ok(result)
        }
    }

    fn evaluate_step(
        &self,
        step: &Ast,
        input: &'a Value<'a>,
        frame: &Frame<'a>,
        last_step: bool,
    ) -> Result<&'a Value<'a>> {
        if let AstKind::Sort(ref sort_terms) = step.kind {
            let mut result = self.evaluate_sort(step.char_index, sort_terms, input, frame)?;
            if let Some(ref stages) = step.stages {
                result = self.evaluate_stages(stages, result, frame)?;
            }
            return Ok(result);
        }

        let mut result: Vec<&'a Value<'a>> = Vec::new();

        // Evaluate the step on each member of the input
        for (item_index, item) in input.members().enumerate() {
            if let Some(ref index_var) = step.index {
                frame.bind(index_var, Value::number(self.arena, item_index as f64));
            }

            let mut item_result = self.evaluate(step, item, frame)?;

            if let Some(ref stages) = step.stages {
                for stage in stages {
                    if let AstKind::Filter(ref expr) = stage.kind {
                        item_result = self.evaluate_filter(expr, item_result, frame)?
                    }
                }
            }

            if !item_result.is_undefined() {
                result.push(item_result);
            }
        }

        Ok(
            if last_step
                && result.len() == 1
                && result[0].is_array()
                && !result[0].has_flags(ArrayFlags::SEQUENCE)
            {
                result.remove(0)
            } else {
                // Flatten the result sequence
                let result_sequence =
                    Value::array_with_capacity(self.arena, result.len(), ArrayFlags::SEQUENCE);

                for result_item in result {
                    if !result_item.is_array() || result_item.has_flags(ArrayFlags::CONS) {
                        result_sequence.push(result_item);
                    } else {
                        for item in result_item.members() {
                            result_sequence.push(item);
                        }
                    }
                }
                result_sequence
            },
        )
    }

    fn evaluate_tuple_step(
        &self,
        step: &Ast,
        input: &'a Value<'a>,
        tuple_bindings: &'a Value<'a>,
        frame: &Frame<'a>,
    ) -> Result<&'a Value<'a>> {
        if let AstKind::Sort(ref sort_terms) = step.kind {
            let mut result = if tuple_bindings.is_undefined() {
                let sorted = self.evaluate_sort(step.char_index, sort_terms, input, frame)?;
                let result =
                    Value::array(self.arena, ArrayFlags::SEQUENCE | ArrayFlags::TUPLE_STREAM);
                for (item_index, item) in sorted.members().enumerate() {
                    let tuple = Value::object(self.arena);
                    tuple.insert("@", item);
                    if let Some(ref index_var) = step.index {
                        tuple.insert(index_var, Value::number(self.arena, item_index as f64));
                    }
                    result.push(tuple);
                }
                result
            } else {
                self.evaluate_sort(step.char_index, sort_terms, tuple_bindings, frame)?
            };

            if let Some(ref stages) = step.stages {
                result = self.evaluate_stages(stages, result, frame)?;
            }

            return Ok(result);
        }

        let tuple_bindings = if tuple_bindings.is_undefined() {
            let tuple_bindings =
                Value::array_with_capacity(self.arena, input.len(), ArrayFlags::empty());
            for member in input.members() {
                let tuple = Value::object(self.arena);
                tuple.insert("@", member);
                tuple_bindings.push(tuple);
            }
            tuple_bindings
        } else {
            tuple_bindings
        };

        let result = Value::array(self.arena, ArrayFlags::SEQUENCE | ArrayFlags::TUPLE_STREAM);

        for tuple in tuple_bindings.members() {
            let step_frame = Frame::from_tuple(frame, tuple);
            let mut binding_sequence = self.evaluate(step, &tuple["@"], &step_frame)?;
            if !binding_sequence.is_undefined() {
                binding_sequence = Value::wrap_in_array_if_needed(
                    self.arena,
                    binding_sequence,
                    ArrayFlags::empty(),
                );
                for (binding_index, binding) in binding_sequence.members().enumerate() {
                    let output_tuple = Value::object(self.arena);
                    for (key, value) in tuple.entries() {
                        output_tuple.insert(key, value);
                    }
                    if binding_sequence.has_flags(ArrayFlags::TUPLE_STREAM) {
                        for (key, value) in binding.entries() {
                            output_tuple.insert(key, value);
                        }
                    } else {
                        if let Some(ref focus_var) = step.focus {
                            output_tuple.insert(focus_var, binding);
                            output_tuple.insert("@", &tuple["@"]);
                        } else {
                            output_tuple.insert("@", binding);
                        }
                        if let Some(ref index_var) = step.index {
                            output_tuple
                                .insert(index_var, Value::number(self.arena, binding_index as f64));
                        }
                    }
                    result.push(output_tuple);
                }
            }
        }

        let mut result = &*result;
        if let Some(ref stages) = step.stages {
            result = self.evaluate_stages(stages, result, frame)?;
        }

        Ok(result)
    }

    fn evaluate_sort(
        &self,
        char_index: usize,
        sort_terms: &[(Ast, bool)],
        input: &'a Value<'a>,
        frame: &Frame<'a>,
    ) -> Result<&'a Value<'a>> {
        if input.is_undefined() {
            return Ok(Value::undefined());
        }

        if !input.is_array() || input.len() <= 1 {
            return Ok(Value::wrap_in_array_if_needed(
                self.arena,
                input,
                ArrayFlags::empty(),
            ));
        }

        let unsorted = input.members().collect::<Vec<&'a Value<'a>>>();
        let is_tuple_sort = input.has_flags(ArrayFlags::TUPLE_STREAM);

        let comp = |a: &'a Value<'a>, b: &'a Value<'a>| {
            let mut result = 0;

            for (sort_term, descending) in sort_terms {
                let aa = if is_tuple_sort {
                    let tuple_frame = Frame::from_tuple(frame, a);
                    self.evaluate(sort_term, &a["@"], &tuple_frame)?
                } else {
                    self.evaluate(sort_term, a, frame)?
                };

                let bb = if is_tuple_sort {
                    let tuple_frame = Frame::from_tuple(frame, b);
                    self.evaluate(sort_term, &b["@"], &tuple_frame)?
                } else {
                    self.evaluate(sort_term, b, frame)?
                };

                if aa.is_undefined() {
                    result = if bb.is_undefined() { 0 } else { 1 };
                    continue;
                }

                if bb.is_undefined() {
                    result = -1;
                    continue;
                }

                if !(aa.is_string() || aa.is_number()) || !(bb.is_string() || bb.is_number()) {
                    return Err(Error::T2008InvalidOrderBy(char_index));
                }

                match (aa, bb) {
                    (Value::String(a), Value::String(b)) if *a == *b => {
                        continue;
                    }
                    (Value::String(a), Value::String(b)) if *a < *b => {
                        result = -1;
                    }
                    (Value::String(..), Value::String(..)) => {
                        result = 1;
                    }
                    (Value::Number(a), Value::Number(b)) if *a == *b => {
                        continue;
                    }
                    (Value::Number(a), Value::Number(b)) if *a < *b => {
                        result = -1;
                    }
                    (Value::Number(..), Value::Number(..)) => {
                        result = 1;
                    }
                    _ => {
                        return Err(Error::T2007CompareTypeMismatch(
                            char_index,
                            a.to_string(),
                            b.to_string(),
                        ));
                    }
                };

                if *descending {
                    result = -result;
                }
            }

            Ok(result == 1)
        };

        let sorted = merge_sort(unsorted, &comp)?;
        let result = Value::array_with_capacity(self.arena, sorted.len(), input.get_flags());
        sorted.iter().for_each(|member| result.push(member));

        Ok(result)
    }

    fn evaluate_stages(
        &self,
        stages: &[Ast],
        input: &'a Value<'a>,
        frame: &Frame<'a>,
    ) -> Result<&'a Value<'a>> {
        let mut result = input;
        for stage in stages.iter() {
            match stage.kind {
                AstKind::Filter(ref predicate) => {
                    result = self.evaluate_filter(predicate, result, frame)?;
                }
                AstKind::Index(ref index_var) => {
                    // TODO: This is really annoying. We can't reach into the internal HashMap and
                    // change the value of the index_var key because we have an &Value and in general
                    // there's no other place we need to mutate Values after they have been created.
                    //
                    // So this just recreates the whole thing, which could be very inefficient for large
                    // arrays.
                    let new_result =
                        Value::array_with_capacity(self.arena, result.len(), result.get_flags());
                    for (tuple_index, tuple) in result.members().enumerate() {
                        let new_tuple =
                            Value::object_with_capacity(self.arena, tuple.entries().len());
                        for (key, value) in tuple.entries() {
                            new_tuple.insert(key, value);
                        }
                        new_tuple.insert(index_var, Value::number(self.arena, tuple_index as f64));
                        new_result.push(new_tuple);
                    }
                    result = new_result;
                }
                _ => unreachable!(),
            }
        }
        Ok(result)
    }

    fn evaluate_filter(
        &self,
        predicate: &Ast,
        input: &'a Value<'a>,
        frame: &Frame<'a>,
    ) -> Result<&'a Value<'a>> {
        let flags = if input.has_flags(ArrayFlags::TUPLE_STREAM) {
            ArrayFlags::SEQUENCE | ArrayFlags::TUPLE_STREAM
        } else {
            ArrayFlags::SEQUENCE
        };
        let result = Value::array(self.arena, flags);
        let input = Value::wrap_in_array_if_needed(self.arena, input, ArrayFlags::empty());

        let get_index = |n: f64| {
            let mut index = n.floor() as isize;
            let length = if input.is_array() {
                input.len() as isize
            } else {
                1
            };
            if index < 0 {
                // Count from the end of the array
                index += length;
            }
            index as usize
        };

        match predicate.kind {
            AstKind::Number(n) => {
                let index = get_index(n);
                let item = input.get_member(index);
                if !item.is_undefined() {
                    if item.is_array() {
                        return Ok(item);
                    } else {
                        result.push(item);
                    }
                }
            }
            _ => {
                for (item_index, item) in input.members().enumerate() {
                    let mut index = if input.has_flags(ArrayFlags::TUPLE_STREAM) {
                        let tuple_frame = Frame::from_tuple(frame, item);
                        self.evaluate(predicate, &item["@"], &tuple_frame)?
                    } else {
                        self.evaluate(predicate, item, frame)?
                    };

                    if index.is_valid_number()? {
                        index = Value::wrap_in_array(self.arena, index, ArrayFlags::empty());
                    }

                    if index.is_array_of_valid_numbers()? {
                        index.members().for_each(|v| {
                            let index = get_index(v.as_f64());
                            if index == item_index {
                                result.push(item);
                            }
                        });
                    } else if index.is_truthy() {
                        result.push(item);
                    }
                }
            }
        }

        Ok(result)
    }

    fn evaluate_wildcard(&self, input: &'a Value<'a>) -> Result<&'a Value<'a>> {
        let mut values = BumpVec::new_in(self.arena);

        let input = if input.is_array() && input.has_flags(ArrayFlags::WRAPPED) && !input.is_empty()
        {
            input.get_member(0)
        } else {
            input
        };

        if input.is_object() {
            for (_key, value) in input.entries() {
                if value.is_array() {
                    let value = value.flatten(self.arena);
                    fn_append_internal(&mut values, value);
                } else {
                    values.push(value)
                }
            }
        }

        Ok(Value::array_from(self.arena, values, ArrayFlags::SEQUENCE))
    }

    fn evaluate_descendants(&self, input: &'a Value<'a>) -> Result<&'a Value<'a>> {
        Ok(if !input.is_undefined() {
            let result_sequence =
                self.recurse_descendants(input, Value::array(self.arena, ArrayFlags::SEQUENCE));

            if result_sequence.len() == 1 {
                result_sequence.get_member(0)
            } else {
                result_sequence
            }
        } else {
            input
        })
    }

    #[allow(clippy::only_used_in_recursion)]
    fn recurse_descendants(
        &self,
        input: &'a Value<'a>,
        result_sequence: &'a mut Value<'a>,
    ) -> &'a mut Value<'a> {
        if !input.is_array() {
            result_sequence.push(input);
        }

        let mut result_sequence = result_sequence;

        if input.is_array() {
            for member in input.members() {
                result_sequence = self.recurse_descendants(member, result_sequence);
            }
        } else if input.is_object() {
            for (_key, value) in input.entries() {
                result_sequence = self.recurse_descendants(value, result_sequence);
            }
        }

        result_sequence
    }

    fn evaluate_function(
        &self,
        input: &'a Value<'a>,
        proc: &Ast,
        args: &[Ast],
        _is_partial: bool,
        frame: &Frame<'a>,
        context: Option<&'a Value<'a>>,
    ) -> Result<&'a Value<'a>> {
        let evaluated_proc = self.evaluate(proc, input, frame)?;

        // Help the user out if they forgot a '$'
        if evaluated_proc.is_undefined() {
            if let AstKind::Path(ref steps) = proc.kind {
                if let AstKind::Name(ref name) = steps[0].kind {
                    if frame.lookup(name).is_some() {
                        return Err(Error::T1005InvokedNonFunctionSuggest(
                            proc.char_index,
                            name.clone(),
                        ));
                    }
                }
            }
        }

        let mut evaluated_args = Vec::with_capacity(args.len());

        if let Some(context) = context {
            evaluated_args.push(context);
        }

        for arg in args {
            let arg = self.evaluate(arg, input, frame)?;
            evaluated_args.push(arg);
        }

        let mut result = self.apply_function(
            proc.char_index,
            input,
            evaluated_proc,
            &evaluated_args,
            frame,
        )?;

        // Trampoline loop for tail-call optimization
        // TODO: This loop needs help
        while let Value::Lambda {
            ref ast,
            input: lambda_input,
            frame: ref lambda_frame,
            ..
        } = result
        {
            if let AstKind::Lambda {
                ref body,
                thunk: true,
                ..
            } = ast.kind
            {
                if let AstKind::Function {
                    ref proc, ref args, ..
                } = body.kind
                {
                    let next = self.evaluate(proc, lambda_input, lambda_frame)?;
                    let mut evaluated_args = Vec::with_capacity(args.len());

                    for arg in args {
                        let arg = self.evaluate(arg, lambda_input, lambda_frame)?;
                        evaluated_args.push(arg);
                    }

                    result =
                        self.apply_function(proc.char_index, input, next, &evaluated_args, frame)?;
                } else {
                    unreachable!()
                }
            } else {
                break;
            }
        }

        Ok(result)
    }

    pub fn apply_function(
        &self,
        char_index: usize,
        input: &'a Value<'a>,
        evaluated_proc: &'a Value<'a>,
        evaluated_args: &[&'a Value<'a>],
        frame: &Frame<'a>,
    ) -> Result<&'a Value<'a>> {
        match evaluated_proc {
            Value::Lambda {
                ref ast,
                ref frame,
                input,
                ..
            } => {
                if let AstKind::Lambda {
                    ref body, ref args, ..
                } = ast.kind
                {
                    // Create a new frame for use in the lambda, so it can have locals
                    let frame = Frame::new_with_parent(frame);

                    // Bind the arguments to their respective names
                    for (index, arg) in args.iter().enumerate() {
                        if let AstKind::Var(ref name) = arg.kind {
                            frame.bind(
                                name,
                                evaluated_args
                                    .get(index)
                                    .copied()
                                    .unwrap_or_else(Value::undefined),
                            );
                        } else {
                            unreachable!()
                        }
                    }

                    // Evaluate the lambda!
                    self.evaluate(body, input, &frame)
                } else {
                    unreachable!()
                }
            }
            Value::NativeFn {
                ref name, ref func, ..
            } => {
                let context = self.fn_context(name, char_index, input, frame);
                func(context, evaluated_args)
            }
            Value::Transformer {
                ref pattern,
                ref update,
                ref delete,
            } => {
                let input = &evaluated_args[0];
                self.apply_transformer(input, pattern, update, delete, frame)
            }
            _ => Err(Error::T1006InvokedNonFunction(char_index)),
        }
    }

    fn apply_transformer(
        &self,
        input: &'a Value<'a>,
        pattern_ast: &Ast,
        update_ast: &Ast,
        delete_ast: &Option<Box<Ast>>,
        frame: &Frame<'a>,
    ) -> Result<&'a Value<'a>> {
        if input.is_undefined() {
            return Ok(Value::undefined());
        }

        if !input.is_object() && !input.is_array() {
            return Err(Error::T0410ArgumentNotValid(
                pattern_ast.char_index,
                1,
                "undefined".to_string(),
            ));
        }

        let result = input.clone(self.arena);

        let matches = self.evaluate(
            pattern_ast,
            Value::wrap_in_array(self.arena, result, ArrayFlags::empty()),
            frame,
        )?;

        if !matches.is_undefined() {
            let matches = Value::wrap_in_array_if_needed(self.arena, matches, ArrayFlags::empty());
            for m in matches.members() {
                let update = self.evaluate(update_ast, m, frame)?;
                if !update.is_undefined() {
                    if !update.is_object() {
                        return Err(Error::T2011UpdateNotObject(
                            update_ast.char_index,
                            update.to_string(),
                        ));
                    } else {
                        for (key, value) in update.entries() {
                            m.__very_unsafe_make_mut().insert(key, value);
                        }
                    }
                }

                if let Some(delete_ast) = delete_ast {
                    let deletions = self.evaluate(delete_ast, m, frame)?;
                    if !deletions.is_undefined() {
                        let deletions = Value::wrap_in_array_if_needed(
                            self.arena,
                            deletions,
                            ArrayFlags::empty(),
                        );
                        for deletion in deletions.members() {
                            if !deletion.is_string() {
                                return Err(Error::T2012DeleteNotStrings(
                                    delete_ast.char_index,
                                    deletions.to_string(),
                                ));
                            }
                            if m.is_object() {
                                m.__very_unsafe_make_mut().remove(&deletion.as_str());
                            }
                        }
                    }
                }
            }
        }

        Ok(result)
    }
}
