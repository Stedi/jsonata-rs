use crate::{Error, Result};

use super::ast::*;
use super::tokenizer::{Token, TokenKind};
use super::Parser;

pub trait Symbol {
    fn left_binding_power(&self) -> u32;
    fn null_denotation(&self, parser: &mut Parser) -> Result<Ast>;
    fn left_denotation(&self, parser: &mut Parser, left: Ast) -> Result<Ast>;
}

impl Symbol for Token {
    fn left_binding_power(&self) -> u32 {
        use TokenKind::*;
        match &self.kind {
            Bind => 10,
            QuestionMark => 20,
            Or => 25,
            And => 30,
            NotEqual | GreaterEqual | LessEqual | Apply | In | Equal | RightAngleBracket
            | LeftAngleBracket | Caret => 40,
            Ampersand | Plus | Minus => 50,
            Asterisk | Descendent | ForwardSlash | PercentSign => 60,
            LeftBrace => 70,
            Period => 75,
            LeftBracket | LeftParen => 80,
            At | Hash => 80,
            _ => 0,
        }
    }

    fn null_denotation(&self, parser: &mut Parser) -> Result<Ast> {
        match self.kind {
            TokenKind::Null => Ok(Ast::new(AstKind::Null, self.char_index)),
            TokenKind::Bool(ref v) => Ok(Ast::new(AstKind::Bool(*v), self.char_index)),
            TokenKind::Str(ref v) => Ok(Ast::new(AstKind::String(v.clone()), self.char_index)),
            TokenKind::Number(v) => Ok(Ast::new(AstKind::Number(v), self.char_index)),
            TokenKind::Regex(ref v) => Ok(Ast::new(AstKind::Regex(v.clone()), self.char_index)),
            TokenKind::Name(ref v) => Ok(Ast::new(AstKind::Name(v.clone()), self.char_index)),
            TokenKind::Var(ref v) => Ok(Ast::new(AstKind::Var(v.clone()), self.char_index)),
            TokenKind::And => Ok(Ast::new(
                AstKind::Name(String::from("and")),
                self.char_index,
            )),
            TokenKind::Or => Ok(Ast::new(AstKind::Name(String::from("or")), self.char_index)),
            TokenKind::In => Ok(Ast::new(AstKind::Name(String::from("in")), self.char_index)),
            TokenKind::Minus => Ok(Ast::new(
                AstKind::Unary(UnaryOp::Minus(Box::new(parser.expression(70)?))),
                self.char_index,
            )),
            TokenKind::Asterisk => Ok(Ast::new(AstKind::Wildcard, self.char_index)),
            TokenKind::Descendent => Ok(Ast::new(AstKind::Descendent, self.char_index)),
            TokenKind::PercentSign => Ok(Ast::new(AstKind::Parent, self.char_index)),

            // Block of expressions
            TokenKind::LeftParen => {
                let mut expressions = Vec::new();

                while parser.token().kind != TokenKind::RightParen {
                    expressions.push(parser.expression(0)?);
                    if parser.token().kind != TokenKind::SemiColon {
                        break;
                    }
                    parser.expect(TokenKind::SemiColon)?;
                }
                parser.expect(TokenKind::RightParen)?;

                Ok(Ast::new(AstKind::Block(expressions), self.char_index))
            }

            // Array constructor
            TokenKind::LeftBracket => {
                let mut expressions = Vec::new();

                if parser.token().kind != TokenKind::RightBracket {
                    loop {
                        let mut item = parser.expression(0)?;

                        if parser.token().kind == TokenKind::Range {
                            parser.expect(TokenKind::Range)?;
                            item = Ast::new(
                                AstKind::Binary(
                                    BinaryOp::Range,
                                    Box::new(item),
                                    Box::new(parser.expression(0)?),
                                ),
                                self.char_index,
                            )
                        }

                        expressions.push(item);

                        if parser.token().kind != TokenKind::Comma {
                            break;
                        }

                        parser.expect(TokenKind::Comma)?;
                    }
                }
                parser.expect(TokenKind::RightBracket)?;

                Ok(Ast::new(
                    AstKind::Unary(UnaryOp::ArrayConstructor(expressions)),
                    self.char_index,
                ))
            }

            // Object constructor
            TokenKind::LeftBrace => Ok(Ast::new(
                AstKind::Unary(UnaryOp::ObjectConstructor(parse_object(parser)?)),
                self.char_index,
            )),

            // Object transformer
            TokenKind::Pipe => {
                let pattern = Box::new(parser.expression(0)?);

                parser.expect(TokenKind::Pipe)?;

                let update = Box::new(parser.expression(0)?);

                let delete = if parser.token().kind == TokenKind::Comma {
                    parser.expect(TokenKind::Comma)?;
                    Some(Box::new(parser.expression(0)?))
                } else {
                    None
                };

                parser.expect(TokenKind::Pipe)?;

                Ok(Ast::new(
                    AstKind::Transform {
                        pattern,
                        update,
                        delete,
                    },
                    self.char_index,
                ))
            }

            _ => Err(Error::S0211InvalidUnary(
                self.char_index,
                self.kind.to_string(),
            )),
        }
    }

    fn left_denotation(&self, parser: &mut Parser, mut left: Ast) -> Result<Ast> {
        macro_rules! binary {
            ($n:tt) => {
                Ok(Ast::new(
                    AstKind::Binary(
                        BinaryOp::$n,
                        Box::new(left),
                        Box::new(parser.expression(self.left_binding_power())?),
                    ),
                    self.char_index,
                ))
            };
        }

        match self.kind {
            TokenKind::Period => binary!(Map),
            TokenKind::Plus => binary!(Add),
            TokenKind::Minus => binary!(Subtract),
            TokenKind::Asterisk => binary!(Multiply),
            TokenKind::ForwardSlash => binary!(Divide),
            TokenKind::PercentSign => binary!(Modulus),
            TokenKind::Equal => binary!(Equal),
            TokenKind::LeftAngleBracket => binary!(LessThan),
            TokenKind::RightAngleBracket => binary!(GreaterThan),
            TokenKind::NotEqual => binary!(NotEqual),
            TokenKind::LessEqual => binary!(LessThanEqual),
            TokenKind::GreaterEqual => binary!(GreaterThanEqual),
            TokenKind::Ampersand => binary!(Concat),
            TokenKind::And => binary!(And),
            TokenKind::Or => binary!(Or),
            TokenKind::In => binary!(In),
            TokenKind::Apply => binary!(Apply),

            // Function calls or lambda definitions
            TokenKind::LeftParen => {
                let mut args = Vec::new();
                let mut is_partial = false;
                let mut is_lambda = false;

                if parser.token().kind != TokenKind::RightParen {
                    loop {
                        match parser.token().kind {
                            TokenKind::QuestionMark => {
                                is_partial = true;
                                args.push(Ast::new(AstKind::PartialArg, parser.token().char_index));
                                parser.expect(TokenKind::QuestionMark)?;
                            }
                            _ => {
                                args.push(parser.expression(0)?);
                            }
                        }
                        if parser.token().kind != TokenKind::Comma {
                            break;
                        }
                        parser.expect(TokenKind::Comma)?;
                    }
                }
                parser.expect(TokenKind::RightParen)?;

                let name = match left.kind {
                    AstKind::Name(ref name) => {
                        // If the name of the function is 'function' or λ, then this is a function definition (lambda function)
                        if name == "function" || name == "λ" {
                            is_lambda = true;

                            // All of the args must be Variable nodes
                            for arg in &args {
                                if !matches!(arg.kind, AstKind::Var(..)) {
                                    return Err(Error::S0208InvalidFunctionParam(
                                        arg.char_index,
                                        self.kind.to_string(),
                                    ));
                                }
                            }
                        }
                        name.clone()
                    }
                    AstKind::Var(ref name) => name.clone(),
                    _ => unreachable!(),
                };

                let func: Ast;

                if is_lambda {
                    parser.expect(TokenKind::LeftBrace)?;
                    let body = Box::new(parser.expression(0)?);
                    func = Ast::new(
                        AstKind::Lambda {
                            name,
                            args,
                            body,
                            thunk: false,
                        },
                        self.char_index,
                    );
                    parser.expect(TokenKind::RightBrace)?;
                } else {
                    func = Ast::new(
                        AstKind::Function {
                            name,
                            proc: Box::new(left),
                            args,
                            is_partial,
                        },
                        self.char_index,
                    );
                }

                Ok(func)
            }

            // Variable assignment
            TokenKind::Bind => {
                if !matches!(left.kind, AstKind::Var(..)) {
                    return Err(Error::S0212ExpectedVarLeft(left.char_index));
                }

                Ok(Ast::new(
                    AstKind::Binary(
                        BinaryOp::Bind,
                        Box::new(left),
                        Box::new(parser.expression(self.left_binding_power() - 1)?),
                    ),
                    self.char_index,
                ))
            }

            // Order by expression
            TokenKind::Caret => {
                let mut terms = Vec::new();

                parser.expect(TokenKind::LeftParen)?;
                loop {
                    let mut descending = false;
                    if parser.token().kind == TokenKind::LeftAngleBracket {
                        parser.expect(TokenKind::LeftAngleBracket)?;
                    } else if parser.token().kind == TokenKind::RightAngleBracket {
                        parser.expect(TokenKind::RightAngleBracket)?;
                        descending = true;
                    }

                    terms.push((parser.expression(0)?, descending));

                    if parser.token().kind != TokenKind::Comma {
                        break;
                    }
                    parser.expect(TokenKind::Comma)?;
                }
                parser.expect(TokenKind::RightParen)?;

                Ok(Ast::new(
                    AstKind::OrderBy(Box::new(left), terms),
                    self.char_index,
                ))
            }

            // Context variable bind
            TokenKind::At => {
                let rhs = parser.expression(self.left_binding_power())?;

                if !matches!(rhs.kind, AstKind::Var(..)) {
                    return Err(Error::S0214ExpectedVarRight(
                        rhs.char_index,
                        "@".to_string(),
                    ));
                }

                Ok(Ast::new(
                    AstKind::Binary(BinaryOp::FocusBind, Box::new(left), Box::new(rhs)),
                    self.char_index,
                ))
            }

            // Positional variable bind
            TokenKind::Hash => {
                let rhs = parser.expression(self.left_binding_power())?;

                if !matches!(rhs.kind, AstKind::Var(..)) {
                    return Err(Error::S0214ExpectedVarRight(
                        rhs.char_index,
                        "#".to_string(),
                    ));
                }

                Ok(Ast::new(
                    AstKind::Binary(BinaryOp::IndexBind, Box::new(left), Box::new(rhs)),
                    self.char_index,
                ))
            }

            // Ternary conditional
            TokenKind::QuestionMark => {
                let truthy = Box::new(parser.expression(0)?);

                let falsy = if parser.token().kind == TokenKind::Colon {
                    parser.expect(TokenKind::Colon)?;
                    Some(Box::new(parser.expression(0)?))
                } else {
                    None
                };

                Ok(Ast::new(
                    AstKind::Ternary {
                        cond: Box::new(left),
                        truthy,
                        falsy,
                    },
                    self.char_index,
                ))
            }

            // Object group by
            TokenKind::LeftBrace => Ok(Ast::new(
                AstKind::GroupBy(Box::new(left), parse_object(parser)?),
                self.char_index,
            )),

            // Array predicate or index
            TokenKind::LeftBracket => {
                if parser.token().kind == TokenKind::RightBracket {
                    // Empty predicate means maintain singleton arrays in the output

                    let mut step = &mut left;

                    // Walk back through left hand sides to find something that's not an array
                    // predicate
                    while let AstKind::Binary(BinaryOp::Predicate, ref mut left, ..) = step.kind {
                        step = left
                    }

                    step.keep_array = true;

                    parser.expect(TokenKind::RightBracket)?;

                    Ok(left)
                } else {
                    let rhs = parser.expression(0)?;
                    parser.expect(TokenKind::RightBracket)?;
                    Ok(Ast::new(
                        AstKind::Binary(BinaryOp::Predicate, Box::new(left), Box::new(rhs)),
                        self.char_index,
                    ))
                }
            }

            _ => Err(Error::S0201SyntaxError(
                self.byte_index,
                parser.tokenizer.string_from_token(self),
            )),
        }
    }
}

/// Parses an object definition.
fn parse_object(parser: &mut Parser) -> Result<Object> {
    let mut object: Object = Vec::new();
    if parser.token().kind != TokenKind::RightBrace {
        loop {
            let key = parser.expression(0)?;
            parser.expect(TokenKind::Colon)?;
            let value = parser.expression(0)?;
            object.push((key, value));
            if parser.token().kind != TokenKind::Comma {
                break;
            }
            parser.expect(TokenKind::Comma)?;
        }
    }
    parser.expect(TokenKind::RightBrace)?;
    Ok(object)
}
