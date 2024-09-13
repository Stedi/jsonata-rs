pub mod ast;
pub mod expressions;
mod process;
mod symbol;
mod tokenizer;

use crate::{Error, Result};

use ast::*;
use symbol::Symbol;
use tokenizer::*;

#[derive(Debug)]
pub struct Parser<'a> {
    pub tokenizer: Tokenizer<'a>,
    pub token: Token,
}

impl<'a> Parser<'a> {
    fn new(source: &'a str) -> Result<Self> {
        let mut tokenizer = Tokenizer::new(source);
        Ok(Self {
            token: tokenizer.next_token()?,
            tokenizer,
        })
    }

    pub fn token(&self) -> &Token {
        &self.token
    }

    pub fn next_token(&mut self) -> Result<()> {
        self.token = self.tokenizer.next_token()?;
        Ok(())
    }

    pub fn expect(&mut self, expected: TokenKind) -> Result<()> {
        if self.token.kind == TokenKind::End {
            return Err(Error::S0203ExpectedTokenBeforeEnd(
                self.token.byte_index,
                expected.to_string(),
            ));
        }

        if self.token.kind != expected {
            return Err(Error::S0202UnexpectedToken(
                self.token.char_index,
                expected.to_string(),
                self.token.kind.to_string(),
            ));
        }

        self.next_token()?;

        Ok(())
    }

    pub fn expression(&mut self, bp: u32) -> Result<Ast> {
        let mut last = self.token.clone();
        self.next_token()?;

        let mut left = last.null_denotation(self)?;

        while bp < self.token.left_binding_power() {
            last = self.token.clone();
            self.next_token()?;
            left = last.left_denotation(self, left)?;
        }

        Ok(left)
    }
}

pub fn parse(source: &str) -> Result<Ast> {
    let mut parser = Parser::new(source)?;
    let ast = parser.expression(0)?;
    if !matches!(parser.token().kind, TokenKind::End) {
        return Err(Error::S0201SyntaxError(
            parser.token().byte_index,
            parser.tokenizer.string_from_token(parser.token()),
        ));
    }
    ast.process()
}

#[cfg(test)]
mod tests {
    //! Parsing tests, mostly just to ensure that the parser doesn't fail on valid JSONata. Most
    //! of these examples are taken from the JSONata docs. These are not meant to be tests of the
    //! produced AST, which is proved correct by the integration tests.
    use super::*;
    use test_case::test_case;

    #[test_case("Address1.City" ; "basic path")]
    #[test_case("Other.`Over 18 ?`" ; "backtick path")]
    #[test_case("Phone1[0]" ; "array index")]
    #[test_case("Phone2[-1]" ; "negative array index")]
    #[test_case("Phone3[0].Number" ; "index and path")]
    #[test_case("Phone4[[0..1]]" ; "range")]
    #[test_case("$[0]" ; "context index")]
    #[test_case("$[0].ref" ; "context index and path")]
    #[test_case("$[0].ref[0]" ; "context index and path index")]
    #[test_case("$.ref" ; "context path")]
    #[test_case("Phone5[type='mobile']" ; "predicate")]
    #[test_case("Phone6[type='mobile'].number" ; "predicate and path")]
    #[test_case("Address2.*" ; "suffix wildcard")]
    #[test_case("*.Postcode1" ; "prefix wildcard")]
    #[test_case("**.Postcode2" ; "prefix descendent wildcard")]
    #[test_case("FirstName & ' ' & Surname" ; "string concatenation")]
    #[test_case("Address3.(Street & ', ' & City)" ; "grouped string concatenation")]
    #[test_case("5&0&true" ; "mixed type string concatenation")]
    #[test_case("Numbers1[0] + Numbers[1]" ; "addition")]
    #[test_case("Numbers2[0] - Numbers[1]" ; "subtraction")]
    #[test_case("Numbers3[0] * Numbers[1]" ; "multiplication")]
    #[test_case("Numbers4[0] / Numbers[1]" ; "division")]
    #[test_case("Numbers5[0] % Numbers[1]" ; "modulus")]
    #[test_case("Numbers6[0] = Numbers[5]" ; "equal")]
    #[test_case("Numbers7[0] != Numbers[5]" ; "not equqal")]
    #[test_case("Numbers8[0] < Numbers[5]" ; "less than")]
    #[test_case("Numbers9[0] <= Numbers[5]" ; "less than equal")]
    #[test_case("Numbers10[0] > Numbers[5]" ; "greater than")]
    #[test_case("Numbers11[0] >= Numbers[5]" ; "greater than equal")]
    #[test_case("\"01962 001234\" in Phone.number" ; "string constant, in operator")]
    #[test_case("(Numbers12[2] != 0) and (Numbers[5] != Numbers[1])" ; "boolean and")]
    #[test_case("(Numbers13[2] != 0) or (Numbers[5] = Numbers[1])" ; "boolean or")]
    #[test_case("Email1.[address]" ; "array constructor")]
    #[test_case("[Address4, Other.`Alternative.Address`].City" ; "array constructor 2")]
    #[test_case("Phone7.{type: number}" ; "object constructor")]
    #[test_case("Phone8{type: number}" ; "object constructor 2")]
    #[test_case("Phone9{type: number[]}"; "object constructor 3")]
    #[test_case("(5 + 3) * 4" ; "math")]
    #[test_case("(expr1; expr2; expr3)" ; "block expression")]
    #[test_case("Account2.Order.Product^(Price)" ; "sort 1")]
    #[test_case("Account3.Order.Product^(>Price)" ; "sort 2")]
    #[test_case("Account4.Order.Product^(>Price, <Quantity)" ; "sort 3")]
    #[test_case("Account5.Order.Product^(Price * Quantity)" ; "sort 4")]
    #[test_case("student[type='fulltime']^(DoB).name" ; "sort 5")]
    #[test_case(
        r#"
        Invoice.(
          $p := Product.Price;
          $q := Product.Quantity;
          $p * $q
        )
    "# ; "variable assignment"
    )]
    #[test_case(
        r#"
        (
          $volume := function($l, $w, $h){ $l * $w * $h };
          $volume(10, 10, 5);
        )
    "# ; "function definition"
    )]
    #[test_case(
        r#"
        (
          $factorial:= function($x){ $x <= 1 ? 1 : $x * $factorial($x-1) };
          $factorial(4)
        )
    "# ; "function definition 1"
    )]
    #[test_case(
        r#"
        (
          $factorial := function($x){(
            $iter := function($x, $acc) {
              $x <= 1 ? $acc : $iter($x - 1, $x * $acc)
            };
            $iter($x, 1)
          )};
          $factorial(170)
        )
    "# ; "function definition 2"
    )]
    #[test_case(
        r#"
        (
          $twice := function($f) { function($x){ $f($f($x)) } };
          $add3 := function($y){ $y + 3 };
          $add6 := $twice($add3);
          $add6(7)
        )
    "# ; "function definition 3"
    )]
    #[test_case(
        r#"
        Account.(
          $AccName := function() { $.'Account Name' };

          Order[OrderID = 'order104'].Product.{
            'Account': $AccName(),
            'SKU-' & $string(ProductID): $.'Product Name'
          }
        )
    "# ; "function definition 4"
    )]
    #[test_case(
        r#"
        (
          $firstN := $substring(?, 0, ?);
          $first5 := $firstN(?, 5);
          $first5("Hello, World")
        )
    "# ; "functions"
    )]
    #[test_case(
        "Customer.Email ~> $substringAfter(\"@\") ~> $substringBefore(\".\") ~> $uppercase()" ; "function application"
    )]
    // #[test_case(
    //     r#"
    //     Account.Order.Product.{
    //       'Product': `Product Name`,
    //       'Order': %.OrderID,
    //       'Account': %.%.`Account Name`
    //     }
    // "# ; "parent operator"
    // )]
    // #[test_case(
    //     r#"
    //     library.books#$i['Kernighan' in authors].{
    //       'title': title,
    //       'index': $i
    //     }
    // "# ; "context variables 1"
    // )]
    // #[test_case(
    //     r#"
    //     library.loans@$l.books@$b[$l.isbn=$b.isbn].{
    //       'title': $b.title,
    //       'customer': $l.customer
    //     }
    // "# ; "context variables 2"
    // )]
    // #[test_case(
    //     r#"
    //     (library.loans)@$l.(catalog.books)@$b[$l.isbn=$b.isbn].{
    //       'title': $b.title,
    //       'customer': $l.customer
    //     }
    // "# ; "context variables 3"
    // )]
    #[test_case("payload ~> |Account.Order.Product|{'Price': Price * 1.2}|" ; "object transform 1")]
    #[test_case("$ ~> |Account.Order.Product|{'Total': Price * Quantity}, ['Price', 'Quantity']|" ; "object transform 2")]
    #[test_case(
        r#"
        /* Long-winded expressions might need some explanation */
        (
          $pi := 3.1415926535897932384626;
          /* JSONata is not known for its graphics support! */
          $plot := function($x) {(
            $floor := $string ~> $substringBefore(?, '.') ~> $number;
            $index := $floor(($x + 1) * 20 + 0.5);
            $join([0..$index].('.')) & 'O' & $join([$index..40].('.'))
          )};

          /* Factorial is the product of the integers 1..n */
          $product := function($a, $b) { $a * $b };
          $factorial := function($n) { $n = 0 ? 1 : $reduce([1..$n], $product) };

          $sin := function($x){ /* define sine in terms of cosine */
            $cos($x - $pi/2)
          };
          $cos := function($x){ /* Derive cosine by expanding Maclaurin series */
            $x > $pi ? $cos($x - 2 * $pi) : $x < -$pi ? $cos($x + 2 * $pi) :
              $sum([0..12].($power(-1, $) * $power($x, 2*$) / $factorial(2*$)))
          };

          [0..24].$sin($*$pi/12).$plot($)
        )
    "# ; "complex expression"
    )]
    fn parser_tests(source: &str) {
        let _ = parse(source);
    }
}
