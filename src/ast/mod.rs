mod process;

use super::json::Number;
use super::position::Position;
use super::signature::Arg;

// Object constructor, represented by tuples of (key, value)
pub type Object = Vec<(Ast, Ast)>;

// Sort terms, representend by expresions and a bool indicating descending/ascending
pub type SortTerms = Vec<(Ast, bool)>;

#[derive(Debug, Clone)]
pub enum UnaryOp {
    Minus(Box<Ast>),
    ArrayConstructor(Vec<Ast>),
    ObjectConstructor(Object),
}

#[derive(Debug, PartialEq, Clone)]
pub enum BinaryOp {
    Add,
    Subtract,
    Multiply,
    Divide,
    Modulus,
    Equal,
    NotEqual,
    LessThan,
    GreaterThan,
    LessThanEqual,
    GreaterThanEqual,
    Concat,
    And,
    Or,
    In,
    Map,
    Range,
    ContextBind,
    PositionalBind,
    Predicate,
    Apply,
    Bind,
}

impl std::fmt::Display for BinaryOp {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match *self {
            BinaryOp::Add => "+",
            BinaryOp::Subtract => "-",
            BinaryOp::Multiply => "*",
            BinaryOp::Divide => "/",
            BinaryOp::Modulus => "%",
            BinaryOp::Equal => "=",
            BinaryOp::NotEqual => "!=",
            BinaryOp::LessThan => "<",
            BinaryOp::GreaterThan => ">",
            BinaryOp::LessThanEqual => "<=",
            BinaryOp::GreaterThanEqual => ">=",
            BinaryOp::Concat => "&",
            BinaryOp::And => "and",
            BinaryOp::Or => "or",
            BinaryOp::In => "in",
            BinaryOp::Map => ".",
            BinaryOp::Range => "..",
            BinaryOp::ContextBind => "@",
            BinaryOp::PositionalBind => "#",
            BinaryOp::Predicate => "[]",
            BinaryOp::Apply => "~>",
            BinaryOp::Bind => ":=",
        })
    }
}

#[derive(Debug, Clone)]
pub enum AstKind {
    Empty,
    Null,
    Bool(bool),
    String(String),
    Number(Number),
    Name(String),
    Var(String),
    Unary(UnaryOp),
    Binary(BinaryOp, Box<Ast>, Box<Ast>),
    GroupBy(Box<Ast>, Object),
    OrderBy(Box<Ast>, Vec<(Ast, bool)>),
    Block(Vec<Ast>),
    Wildcard,
    Descendent,
    Parent,
    Function {
        name: String,
        proc: Box<Ast>,
        args: Vec<Ast>,
        is_partial: bool,
    },
    PartialArg,
    Lambda {
        name: String,
        args: Vec<Ast>,
        body: Box<Ast>,
        signature: Option<Vec<Arg>>,
    },
    Ternary {
        cond: Box<Ast>,
        truthy: Box<Ast>,
        falsy: Option<Box<Ast>>,
    },
    Transform {
        pattern: Box<Ast>,
        update: Box<Ast>,
        delete: Option<Box<Ast>>,
    },

    // Generated by AST post-processing
    Path(Vec<Ast>),
    Filter(Box<Ast>),
    Sort(SortTerms),
}

#[derive(Debug, Clone)]
pub struct Ast {
    pub kind: AstKind,
    pub position: Position,

    pub keep_array: bool,
    pub cons_array: bool,
    pub keep_singleton_array: bool,

    /// An optional group by expression, represented as an object.
    pub group_by: Option<(Position, Object)>,

    /// An optional list of predicates.
    pub predicates: Option<Vec<Ast>>,

    /// An optional list of evaluation stages, for example this specifies the filtering and
    /// indexing for various expressions.
    pub stages: Option<Vec<Ast>>,
}

impl Default for Ast {
    fn default() -> Ast {
        Ast::new(AstKind::Empty, Default::default())
    }
}

impl Ast {
    pub fn new(kind: AstKind, position: Position) -> Self {
        Self {
            kind,
            position,
            keep_array: false,
            cons_array: false,
            keep_singleton_array: false,
            group_by: None,
            predicates: None,
            stages: None,
        }
    }
}
