# jsonata-rs

[<img alt="crates.io" src="https://img.shields.io/crates/v/jsonata-rs?logo=rust&style=for-the-badge" height=22>](https://crates.io/crates/jsonata-rs)
[<img alt="docs.rs" src="https://img.shields.io/docsrs/jsonata-rs?label=docs.rs&logo=docs.rs&style=for-the-badge" height=22>](https://docs.rs/jsonata-rs)

An (incomplete) implementation of [JSONata](https://jsonata.org) in Rust. Stedi's fork of [jsonata-rust](https://github.com/johanventer/jsonata-rust).

**Alpha version. All internal and external interfaces are considered unstable and subject to change without notice.**

## What is JSONata?

From the JSONata website:

- Lightweight query and transformation language for JSON data
- Inspired by the location path semantics of XPath 3.1
- Sophisticated query expressions with minimal syntax
- Built in operators and functions for manipulating and combining data
- Create user-defined functions
- Format query results into any JSON output structure

Read the [full documentation](https://docs.jsonata.org/overview.html), and try it out in Stedi's [JSONata Playground](https://www.stedi.com/jsonata/playground).

## Getting started

The API is currently not very ergonomic, as you need to provide a [`bumpalo`](https://github.com/fitzgen/bumpalo) arena for allocating values in.

First, add the following to your `Cargo.toml`:

```toml
[dependencies]
jsonata-rs = "0"
bumpalo = "3.9.1"
```

Then you can evaluate an expression with JSON input like this:

```rust
use bumpalo::Bump;
use jsonata_rs::JsonAta;

// Create an arena for allocating values, this will go away in future except for advanced use cases
let arena = Bump::new();

// Provide some JSON input, this could be read from a file or come from the network
let input = "{ \"name\": \"world\" }";

// The JSONata expression to evaluate
let expr = "\"Hello, \" & name & \"!\"";

// Parse the expression - this could fail
let jsonata = JsonAta::new(expr, &arena).unwrap();

// Evaluate the expression against the input - this could fail
let result = jsonata.evaluate(Some(input)).unwrap();

// Serialize the result into JSON
println!("{}", result.serialize(false));
```

There's also a basic CLI tool:

```
# cargo install jsonata-rs

# jsonata "1 + 1"
2

# jsonata '"Hello, " & name & "!"' '{ "name": "world" }'
"Hello, world!"
```

The expression and input can be specified on the command line, but that requires manual escaping. Alternatively, they can be provided from files. Here's the `--help` output:

```
# jsonata --help
jsonata-rs
A command line JSON processor using JSONata

USAGE:
    jsonata [FLAGS] [OPTIONS] [ARGS]

FLAGS:
    -a, --ast        Parse the given expression, print the AST and exit
    -h, --help       Prints help information
    -V, --version    Prints version information

OPTIONS:
    -e, --expr-file <expr-file>      File containing the JSONata expression to evaluate (overrides expr on command line)
    -i, --input-file <input-file>    Input JSON file (if not specified, STDIN)

ARGS:
    <expr>     JSONata expression to evaluate
    <input>    JSON input
```

## Missing (but planned) features

There are a number of JSONata features which are not yet implemented:

- Many built-in functions are missing
- Parent operator
- Regular expressions
- Partial function application
- JSON AST output to match the reference implementation

## Differences from reference JSONata

### Function signatures are not supported

Function signatures have their problems as described [here](docs/function-signatures.md), and are not supported by this implementation.

Most of the JSONata functions, however, support being passed the context as the first argument as dictated by their signature, e.g:

```
["Hello", "world"].$substring(1, 2)

/* Output: ["el", "or"] */
```

This is implemented in each built-in function itself. For example, if `$string` sees that it is called with no arguments, it will use the current context.

In addition, for all the built-in functions, type checking of arguments is also implemented directly in the functions themselves so that you get equivalent runtime errors for passing the wrong things to these functions as you would in reference JSONata.

## Status

There's a [status document](docs/status.md) which describes the current status and long-term goals for this implementation.

## Tests

Reference JSONata contains an extensive test suite with over 1000 tests. Currently, this implementation passes over 600 of these, you can run them like this:

```bash
cargo test testsuite
```

In `tests/testsuite/groups` are the tests groups that are passing, while `tests/testsuite/skip` contains the groups that still require feature implementation. There may be tests in the remaining groups that do pass, but I don't want to split them up - only when a test group fully passes is it moved.

## Contribution

We welcome community contributions and pull requests.

## License

This project is licensed under the Apache-2.0 License. Any code you submit will be released under that license.
