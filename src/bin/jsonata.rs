use bumpalo::Bump;
use clap::Parser;
use std::path::PathBuf;

use jsonata_rs::JsonAta;

/// A command line JSON processor using JSONata
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Opt {
    /// Parse the given expression, print the AST and exit
    #[arg(short, long)]
    ast: bool,

    /// File containing the JSONata expression to evaluate (overrides expr on command line)
    #[arg(short, long)]
    expr_file: Option<PathBuf>,

    /// Input JSON file (if not specified, STDIN)
    #[arg(short, long)]
    input_file: Option<PathBuf>,

    /// JSONata expression to evaluate
    expr: Option<String>,

    /// JSON input
    input: Option<String>,
}

fn main() {
    let opt = Opt::parse();

    let expr = match opt.expr_file {
        Some(expr_file) => {
            let expr = std::fs::read(expr_file).expect("Could not read expression input file");
            String::from_utf8_lossy(&expr).to_string()
        }
        None => opt.expr.expect("No JSONata expression provided"),
    };

    let arena = Bump::new();
    let jsonata = JsonAta::new(&expr, &arena);

    match jsonata {
        Ok(jsonata) => {
            if opt.ast {
                println!("{:#?}", jsonata.ast());
                return;
            }

            let input = match opt.input_file {
                Some(input_file) => {
                    std::fs::read_to_string(input_file).expect("Could not read the JSON input file")
                }
                None => opt.input.unwrap_or_else(|| "{}".to_string()),
            };

            match jsonata.evaluate(Some(&input), None) {
                Ok(result) => println!("{}", result.serialize(true)),
                Err(error) => println!("{}", error),
            }
        }
        Err(error) => println!("{}", error),
    }
}
