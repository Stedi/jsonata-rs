/// From the reference JavaScript JSONAta code:
///   This parser implements the 'Top down operator precedence' algorithm developed by Vaughan R Pratt; http://dl.acm.org/citation.cfm?id=512931.
///   and builds on the Javascript framework described by Douglas Crockford at http://javascript.crockford.com/tdop/tdop.html
///   and in 'Beautiful Code', edited by Andy Oram and Greg Wilson, Copyright 2007 O'Reilly Media, Inc. 798-0-596-51004-6
///
/// The formulation of a Top Down Operator Precendence parser (Pratt's Parser) is little more
/// complicated (and a lot more verbose) in a non-dynamic language.
///
/// More resources:
///  - http://effbot.org/zone/simple-top-down-parsing.htm
///  - http://journal.stuffwithstuff.com/2011/03/19/pratt-parsers-expression-parsing-made-easy/
///
/// Some definitions for some of the obscure abbreviations used in this parsing method:
///  rbp & lbp: Left/right binding power, this is how the algorithm evaluates operator precedence
///  nud: Null denotation, a nud symbol DOES NOT care about tokens to the left of it
///  led: Left denotation, a led symbol DOES cares about tokens to the left of it
///
/// Basic algorithm:
///  1. Lexer generates tokens
///  2. If the token appears at the beginning of an expression, call the nud method. If it appears
///     infix, call the led method with the current left hand side as an argument.
///  3. Expression parsing ends when the token's precendence is less than the expression's
///     precendence.
///  4. Productions are returned, which point to other productions forming the AST.
use crate::ast::Node;
use crate::error::Error;
use crate::symbol::Symbol;
use crate::tokenizer::{Token, TokenKind, Tokenizer};

pub struct Parser<'a> {
    tokenizer: Tokenizer<'a>,
    token: Token,
    finished: bool,
}

impl<'a> Parser<'a> {
    pub fn new(source: &'a str) -> Self {
        let mut tokenizer = Tokenizer::new(source);
        let token = match tokenizer.next(false) {
            Some(token) => token,
            None => panic!("No token stream"),
        };
        Self {
            tokenizer,
            token,
            finished: false,
        }
    }

    pub fn token(&self) -> &Token {
        &self.token
    }

    pub fn next(&mut self, infix: bool) {
        match self.tokenizer.next(infix) {
            Some(token) => self.token = token,
            None => self.finished = true,
        };
    }

    pub fn expect(&mut self, expected: TokenKind, infix: bool) {
        self.next(infix);

        if self.finished {
            panic!(format!(
                "{:#?}",
                Error {
                    code: "S0203",
                    position: self.token.position,
                    message: format!("Expected {} before end of expression", expected)
                }
            ))
        }

        if self.token.kind != expected {
            panic!(format!(
                "{:#?}",
                Error {
                    code: "S0202",
                    position: self.token.position,
                    message: format!("Expected {}, got {}", expected, self.token)
                }
            ))
        }
    }

    pub fn expression(&mut self, rbp: u32) -> Box<dyn Node> {
        let mut last = self.token.clone();
        self.next(true);
        let mut left = last.nud(self);

        while !self.finished && rbp < self.token.lbp() {
            last = self.token.clone();
            self.next(false);
            left = last.led(self, left)
        }

        left
    }
}

pub fn parse(source: &str) -> Box<dyn Node> {
    let mut parser = Parser::new(source);
    parser.expression(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use json::stringify_pretty;

    #[test]
    fn basic() {
        let ast = parse("1 + 2 * 3");
        let json = ast.to_json();
        let expected = r#"{
    "type": "binary",
    "value": "+",
    "position": 3,
    "lhs": {
        "type": "number",
        "position": 1,
        "value": 1
    },
    "rhs": {
        "type": "binary",
        "value": "*",
        "position": 7,
        "lhs": {
            "type": "number",
            "position": 5,
            "value": 2
        },
        "rhs": {
            "type": "number",
            "position": 9,
            "value": 3
        }
    }
}"#;
        assert_eq!(expected, stringify_pretty(json, 4));
    }
}
