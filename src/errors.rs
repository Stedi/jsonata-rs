use std::{char, error, fmt};

/// User-facing error codes and messages. These codes are defined in the Javascript implementation here:
/// <https://github.com/jsonata-js/jsonata/blob/9e6b8e6d081e34fbd72fe24ccd703afa9248fec5/src/jsonata.js#L1941>
#[derive(Debug, PartialEq)]
#[non_exhaustive]
pub enum Error {
    // Compile time errors
    S0101UnterminatedStringLiteral(usize),
    S0102LexedNumberOutOfRange(usize, String),
    S0103UnsupportedEscape(usize, char),
    S0104InvalidUnicodeEscape(usize),
    S0105UnterminatedQuoteProp(usize),
    S0106UnterminatedComment(usize),
    S0201SyntaxError(usize, String),
    S0202UnexpectedToken(usize, String, String),
    S0204UnknownOperator(usize, String),
    S0203ExpectedTokenBeforeEnd(usize, String),
    S0208InvalidFunctionParam(usize, String),
    S0209InvalidPredicate(usize),
    S0210MultipleGroupBy(usize),
    S0211InvalidUnary(usize, String),
    S0212ExpectedVarLeft(usize),
    S0213InvalidStep(usize, String),
    S0214ExpectedVarRight(usize, String),
    S0215BindingAfterPredicates(usize),
    S0216BindingAfterSort(usize),
    S0301EmptyRegex(usize),
    S0302UnterminatedRegex(usize),
    // This variant is not present in the JS implementation
    S0303InvalidRegex(usize, String),

    // Runtime errors
    D1001NumberOfOutRange(f64),
    D1002NegatingNonNumeric(usize, String),
    D1009MultipleKeys(usize, String),
    D2014RangeOutOfBounds(usize, isize),
    D3001StringNotFinite(usize),
    D3010EmptyPattern(usize),
    D3011NegativeLimit(usize),
    D3020NegativeLimit(usize),
    D3030NonNumericCast(usize, String),
    D3050SecondArguement(String),
    D3060SqrtNegative(usize, String),
    D3061PowUnrepresentable(usize, String, String),
    D3070InvalidDefaultSort(usize),
    D3141Assert(String),
    D3137Error(String),
    D3138Error(String),
    D3139Error(String),
    D3133PictureStringNameModifierError(String),
    D3134TooManyTzDigits(String),
    D3135PictureStringNoClosingBracketError(String),

    // Type errors
    T0410ArgumentNotValid(usize, usize, String),
    T0412ArgumentMustBeArrayOfType(usize, usize, String, String),
    T1003NonStringKey(usize, String),
    T1005InvokedNonFunctionSuggest(usize, String),
    T1006InvokedNonFunction(usize),
    T2001LeftSideNotNumber(usize, String),
    T2002RightSideNotNumber(usize, String),
    T2003LeftSideNotInteger(usize),
    T2004RightSideNotInteger(usize),
    T2006RightSideNotFunction(usize),
    T2007CompareTypeMismatch(usize, String, String),
    T2008InvalidOrderBy(usize),
    T2009BinaryOpMismatch(usize, String, String, String),
    T2010BinaryOpTypes(usize, String),
    T2011UpdateNotObject(usize, String),
    T2012DeleteNotStrings(usize, String),
    T2013BadClone(usize),

    // Expression timebox/depth errors
    U1001StackOverflow,
    U1001Timeout,
}

impl error::Error for Error {}

impl Error {
    /**
     * Error codes
     *
     * Sxxxx    - Static errors (compile time)
     * Txxxx    - Type errors
     * Dxxxx    - Dynamic errors (evaluate time)
     *  01xx    - tokenizer
     *  02xx    - parser
     *  03xx    - regex parser
     *  04xx    - function signature parser/evaluator
     *  10xx    - evaluator
     *  20xx    - operators
     *  3xxx    - functions (blocks of 10 for each function)
     */
    pub fn code(&self) -> &str {
        match *self {
            // Compile time errors
            Error::S0101UnterminatedStringLiteral(..) => "S0101",
            Error::S0102LexedNumberOutOfRange(..) => "S0102",
            Error::S0103UnsupportedEscape(..) => "S0103",
            Error::S0104InvalidUnicodeEscape(..) => "S0104",
            Error::S0105UnterminatedQuoteProp(..) => "S0105",
            Error::S0106UnterminatedComment(..) => "S0106",
            Error::S0201SyntaxError(..) => "S0201",
            Error::S0202UnexpectedToken(..) => "S0202",
            Error::S0203ExpectedTokenBeforeEnd(..) => "S0203",
            Error::S0204UnknownOperator(..) => "S0204",
            Error::S0208InvalidFunctionParam(..) => "S0208",
            Error::S0209InvalidPredicate(..) => "S0209",
            Error::S0210MultipleGroupBy(..) => "S0210",
            Error::S0211InvalidUnary(..) => "S0211",
            Error::S0212ExpectedVarLeft(..) => "S0212",
            Error::S0213InvalidStep(..) => "S0213",
            Error::S0214ExpectedVarRight(..) => "S0214",
            Error::S0215BindingAfterPredicates(..) => "S0215",
            Error::S0216BindingAfterSort(..) => "S0216",
            Error::S0301EmptyRegex(..) => "S0301",
            Error::S0302UnterminatedRegex(..) => "S0302",
            Error::S0303InvalidRegex(..) => "S0303",

            // Runtime errors
            Error::D1001NumberOfOutRange(..) => "D1001",
            Error::D1002NegatingNonNumeric(..) => "D1002",
            Error::D1009MultipleKeys(..) => "D1009",
            Error::D2014RangeOutOfBounds(..) => "D2014",
            Error::D3001StringNotFinite(..) => "D3001",
            Error::D3010EmptyPattern(..) => "D3010",
            Error::D3011NegativeLimit(..) => "D3011",
            Error::D3020NegativeLimit(..) => "D3020",
            Error::D3030NonNumericCast(..) => "D3030",
            Error::D3050SecondArguement(..) => "D3050",
            Error::D3060SqrtNegative(..) => "D3060",
            Error::D3061PowUnrepresentable(..) => "D3061",
            Error::D3070InvalidDefaultSort(..) => "D3070",
            Error::D3133PictureStringNameModifierError(..) => "D3133",
            Error::D3134TooManyTzDigits(..) => "D3134",
            Error::D3135PictureStringNoClosingBracketError(..) => "D3135",
            Error::D3141Assert(..) => "D3141",
            Error::D3137Error(..) => "D3137",
            Error::D3138Error(..) => "D3138",
            Error::D3139Error(..) => "D3139",

            // Type errors
            Error::T0410ArgumentNotValid(..) => "T0410",
            Error::T0412ArgumentMustBeArrayOfType(..) => "T0412",
            Error::T1003NonStringKey(..) => "T1003",
            Error::T1005InvokedNonFunctionSuggest(..) => "T1005",
            Error::T1006InvokedNonFunction(..) => "T1006",
            Error::T2001LeftSideNotNumber(..) => "T2001",
            Error::T2002RightSideNotNumber(..) => "T2002",
            Error::T2003LeftSideNotInteger(..) => "T2003",
            Error::T2004RightSideNotInteger(..) => "T2004",
            Error::T2006RightSideNotFunction(..) => "T2006",
            Error::T2007CompareTypeMismatch(..) => "T2007",
            Error::T2008InvalidOrderBy(..) => "T2008",
            Error::T2009BinaryOpMismatch(..) => "T2009",
            Error::T2010BinaryOpTypes(..) => "T2010",
            Error::T2011UpdateNotObject(..) => "T2011",
            Error::T2012DeleteNotStrings(..) => "T2012",
            Error::T2013BadClone(..) => "T2013",

            // Expression timebox/depth errors
            Error::U1001StackOverflow => "U1001",
            Error::U1001Timeout => "U1001",
        }
    }
}

impl fmt::Display for Error {
    #[allow(clippy::many_single_char_names)]
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        use Error::*;

        write!(f, "{} @ ", self.code())?;

        // These messages come from the Javascript implementation:
        // <https://github.com/jsonata-js/jsonata/blob/9e6b8e6d081e34fbd72fe24ccd703afa9248fec5/src/jsonata.js#L1941>
        match *self {
            // Compile time errors
            S0101UnterminatedStringLiteral(ref p) =>
                write!(f, "{}: String literal must be terminated by a matching quote", p),
            S0102LexedNumberOutOfRange(ref p, ref n) =>
                write!(f, "{}: Number out of range: {}", p, n),
            S0103UnsupportedEscape(ref p, ref c) =>
                write!(f, "{}: Unsupported escape sequence: \\{}", p, c),
            S0104InvalidUnicodeEscape(ref p) =>
                write!(f, "{}: The escape sequence \\u must be followed by 4 hex digits", p),
            S0105UnterminatedQuoteProp(ref p) =>
                write!(f, "{}: Quoted property name must be terminated with a backquote (`)", p),
            S0106UnterminatedComment(ref p) =>
                write!(f, "{}: Comment has no closing tag", p),
            S0201SyntaxError(ref p, ref t) =>
                write!(f, "{}: Syntax error `{}`", p, t),
            S0202UnexpectedToken(ref p, ref e, ref a) =>
                write!(f, "{}: Expected `{}`, got `{}`", p, e, a),
            S0203ExpectedTokenBeforeEnd(ref p, ref t) =>
                write!(f, "{}: Expected `{}` before end of expression", p, t),
            S0204UnknownOperator(ref p, ref t) =>
                write!(f, "{}: Unknown operator: `{}`", p, t),
            S0208InvalidFunctionParam(ref p, ref k) =>
                write!(f, "{}: Parameter `{}` of function definition must be a variable name (start with $)", p, k),
            S0209InvalidPredicate(ref p) =>
                write!(f, "{}: A predicate cannot follow a grouping expression in a step", p),
            S0210MultipleGroupBy(ref p) =>
                write!(f, "{}: Each step can only have one grouping expression", p),
            S0211InvalidUnary(ref p, ref k) =>
                write!(f, "{}: The symbol `{}` cannot be used as a unary operator", p, k),
            S0212ExpectedVarLeft(ref p) =>
                write!(f, "{}: The left side of `:=` must be a variable name (start with $)", p),
            S0213InvalidStep(ref p, ref k) =>
                write!(f, "{}: The literal value `{}` cannot be used as a step within a path expression", p, k),
            S0214ExpectedVarRight(ref p, ref k) =>
                write!(f, "{}: The right side of `{}` must be a variable name (start with $)", p, k),
            S0215BindingAfterPredicates(ref p) =>
                write!(f, "{}: A context variable binding must precede any predicates on a step", p),
            S0216BindingAfterSort(ref p) =>
                write!(f, "{}: A context variable binding must precede the 'order-by' clause on a step", p),
            S0301EmptyRegex(ref p) =>
                write!(f, "{}: Empty regular expressions are not allowed", p),
            S0302UnterminatedRegex(ref p) =>
                write!(f, "{}: No terminating / in regular expression", p),
            S0303InvalidRegex(ref p, ref message) =>
                // The error message from `regress::Regex` a "regex parse error: " prefix, so don't be redundant here.
                write!(f, "{}: {}", p, message),

            // Runtime errors
            D1001NumberOfOutRange(ref n) => write!(f, "Number out of range: {}", n),
            D1002NegatingNonNumeric(ref p, ref v) =>
                write!(f, "{}: Cannot negate a non-numeric value `{}`", p, v),
            D1009MultipleKeys(ref p, ref k) =>
                write!(f, "{}: Multiple key definitions evaluate to same key: {}", p, k),
            D2014RangeOutOfBounds(ref p, ref s) =>
                write!(f, "{}: The size of the sequence allocated by the range operator (..) must not exceed 1e7.  Attempted to allocate {}", p, s),
            D3001StringNotFinite(ref p) =>
                write!(f, "{}: Attempting to invoke string function on Infinity or NaN", p),
            D3010EmptyPattern(ref p) =>
                write!(f, "{}: Second argument of replace function cannot be an empty string", p),
            D3011NegativeLimit(ref p) =>
                write!(f, "{}: Fourth argument of replace function must evaluate to a positive number", p),
            D3020NegativeLimit(ref p) =>
                write!(f, "{}: Third argument of split function must evaluate to a positive number", p),
            D3030NonNumericCast(ref p, ref n) =>
                write!(f, "{}: Unable to cast value to a number: {}", p, n),
            D3050SecondArguement(ref p) =>
                write!(f, "{}: The second argument of reduce function must be a function with at least two arguments", p),
            D3060SqrtNegative(ref p, ref n) =>
                write!(f, "{}: The sqrt function cannot be applied to a negative number: {}", p, n),
            D3061PowUnrepresentable(ref p, ref b, ref e) =>
                write!(f, "{}: The power function has resulted in a value that cannot be represented as a JSON number: base={}, exponent={}", p, b, e),
            D3070InvalidDefaultSort(ref p) =>
                write!(f, "{}: The single argument form of the sort function can only be applied to an array of strings or an array of numbers.  Use the second argument to specify a comparison function", p),
            D3133PictureStringNameModifierError(ref m) =>
                write!(f, "{}: The 'name' modifier can only be applied to months and days in the date/time picture string, not Y", m),
            D3134TooManyTzDigits(ref m) =>
                write!(f, "{}: The timezone integer format specifier cannot have more than four digits", m),
            D3135PictureStringNoClosingBracketError(ref m) =>
                write!(f, "{}: No matching closing bracket ']' in date/time picture string", m),
            D3141Assert(ref m) =>
                write!(f, "{}", m),
            D3137Error(ref m) =>
                write!(f, "{}", m),
            D3138Error(ref m) =>
                write!(f, "{}: The $single() function expected exactly 1 matching result.  Instead it matched more.", m),
            D3139Error(ref m) =>
                write!(f, "{}: The $single() function expected exactly 1 matching result.  Instead it matched 0.", m),
            // Type errors
            T0410ArgumentNotValid(ref p, ref i, ref t) =>
                write!(f, "{}: Argument {} of function {} does not match function signature", p, i, t),
            T0412ArgumentMustBeArrayOfType(ref p, ref i, ref t, ref ty) =>
                write!(f, "{}: Argument {} of function {} must be an array of {}", p, i, t, ty),
            T1003NonStringKey(ref p, ref v) =>
                write!( f, "{}: Key in object structure must evaluate to a string; got: {}", p, v),
            T1005InvokedNonFunctionSuggest(ref p, ref t) =>
                write!(f, "{}: Attempted to invoke a non-function. Did you mean ${}?", p, t),
            T1006InvokedNonFunction(ref p) =>
                write!(f, "{}: Attempted to invoke a non-function", p),
            T2001LeftSideNotNumber(ref p, ref o) =>
                write!( f, "{}: The left side of the `{}` operator must evaluate to a number", p, o),
            T2002RightSideNotNumber(ref p, ref o) =>
                write!( f, "{}: The right side of the `{}` operator must evaluate to a number", p, o),
            T2003LeftSideNotInteger(ref p) =>
                write!(f, "{}: The left side of the range operator (..) must evaluate to an integer", p),
            T2004RightSideNotInteger(ref p) =>
                write!(f, "{}: The right side of the range operator (..) must evaluate to an integer", p),
            T2006RightSideNotFunction(ref p) =>
                write!(f, "{p} The right side of the function application operator ~> must be a function"),
            T2007CompareTypeMismatch(ref p, ref a, ref b) =>
                write!(f, "{p}: Type mismatch when comparing values {a} and {b} in order-by clause"),
            T2008InvalidOrderBy(ref p) =>
                write!(f, "{}: The expressions within an order-by clause must evaluate to numeric or string values", p),
            T2009BinaryOpMismatch(ref p,ref l ,ref r ,ref o ) =>
                write!(f, "{}: The values {} and {} either side of operator {} must be of the same data type", p, l, r, o),
            T2010BinaryOpTypes(ref p, ref o) =>
                write!(f, "{}: The expressions either side of operator `{}` must evaluate to numeric or string values", p, o),
            T2011UpdateNotObject(ref p, ref v) =>
                write!(f, "{p}: The insert/update clause of the transform expression must evaluate to an object: {v}"),
            T2012DeleteNotStrings(ref p, ref v) =>
                write!(f, "{p}: The delete clause of the transform expression must evaluate to a string or array of strings: {v}"),
            T2013BadClone(ref p) =>
                write!(f, "{p}: The transform expression clones the input object using the $clone() function.  This has been overridden in the current scope by a non-function."),
            // Expression timebox/depth errors
            U1001StackOverflow =>
                write!(f, "Stack overflow error: Check for non-terminating recursive function.  Consider rewriting as tail-recursive."),
            U1001Timeout =>
                write!(f, "Expression evaluation timeout: Check for infinite loop")
        }
    }
}

// "S0205": "Unexpected token: {{token}}",
// "S0206": "Unknown expression type: {{token}}",
// "S0207": "Unexpected end of expression",
// "S0217": "The object representing the 'parent' cannot be derived from this expression",

// "S0301": "Empty regular expressions are not allowed",
// "S0302": "No terminating / in regular expression",
// "S0402": "Choice groups containing parameterized types are not supported",
// "S0401": "Type parameters can only be applied to functions and arrays",
// "S0500": "Attempted to evaluate an expression containing syntax error(s)",
// "T0411": "Context value is not a compatible type with argument {{index}} of function {{token}}",
// "D1004": "Regular expression matches zero length string",
// "T1007": "Attempted to partially apply a non-function. Did you mean ${{{token}}}?",
// "T1008": "Attempted to partially apply a non-function",
// // "T1010": "The matcher function argument passed to function {{token}} does not return the correct object structure",
// "D2005": "The left side of := must be a variable name (start with $)",  // defunct - replaced by S0212 parser error
// define_error!(
//     D2014,
//     "The size of the sequence allocated by the range operator (..) must not exceed 1e7.  Attempted to allocate {}",
//     value
// );
// "D3010": "Second argument of replace function cannot be an empty string",
// "D3011": "Fourth argument of replace function must evaluate to a positive number",
// "D3012": "Attempted to replace a matched string with a non-string value",
// "D3020": "Third argument of split function must evaluate to a positive number",
// "D3040": "Third argument of match function must evaluate to a positive number",
// "D3050": "The second argument of reduce function must be a function with at least two arguments",
// "D3080": "The picture string must only contain a maximum of two sub-pictures",
// "D3081": "The sub-picture must not contain more than one instance of the 'decimal-separator' character",
// "D3082": "The sub-picture must not contain more than one instance of the 'percent' character",
// "D3083": "The sub-picture must not contain more than one instance of the 'per-mille' character",
// "D3084": "The sub-picture must not contain both a 'percent' and a 'per-mille' character",
// "D3085": "The mantissa part of a sub-picture must contain at least one character that is either an 'optional digit character' or a member of the 'decimal digit family'",
// "D3086": "The sub-picture must not contain a passive character that is preceded by an active character and that is followed by another active character",
// "D3087": "The sub-picture must not contain a 'grouping-separator' character that appears adjacent to a 'decimal-separator' character",
// "D3088": "The sub-picture must not contain a 'grouping-separator' at the end of the integer part",
// "D3089": "The sub-picture must not contain two adjacent instances of the 'grouping-separator' character",
// "D3090": "The integer part of the sub-picture must not contain a member of the 'decimal digit family' that is followed by an instance of the 'optional digit character'",
// "D3091": "The fractional part of the sub-picture must not contain an instance of the 'optional digit character' that is followed by a member of the 'decimal digit family'",
// "D3092": "A sub-picture that contains a 'percent' or 'per-mille' character must not contain a character treated as an 'exponent-separator'",
// "D3093": "The exponent part of the sub-picture must comprise only of one or more characters that are members of the 'decimal digit family'",
// "D3100": "The radix of the formatBase function must be between 2 and 36.  It was given {{value}}",
// "D3110": "The argument of the toMillis function must be an ISO 8601 formatted timestamp. Given {{value}}",
// "D3120": "Syntax error in expression passed to function eval: {{value}}",
// "D3121": "Dynamic error evaluating the expression passed to function eval: {{value}}",
// "D3130": "Formatting or parsing an integer as a sequence starting with {{value}} is not supported by this implementation",
// "D3131": "In a decimal digit pattern, all digits must be from the same decimal group",
// "D3132": "Unknown component specifier {{value}} in date/time picture string",
// "D3133": "The 'name' modifier can only be applied to months and days in the date/time picture string, not {{value}}",
// "D3134": "The timezone integer format specifier cannot have more than four digits",
// "D3135": "No matching closing bracket ']' in date/time picture string",
// "D3136": "The date/time picture string is missing specifiers required to parse the timestamp",
// "D3138": "The $single() function expected exactly 1 matching result.  Instead it matched more.",
// "D3139": "The $single() function expected exactly 1 matching result.  Instead it matched 0.",
// "D3140": "Malformed URL passed to ${{{functionName}}}(): {{value}}",
