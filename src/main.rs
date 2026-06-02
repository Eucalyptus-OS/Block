use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use clap::Parser;
use block::system::{build::build, display::pretty_print_statements, lexer::lex, parser::parse};

pub static DEBUG: AtomicBool = AtomicBool::new(false);

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    #[arg(short, long)]
    file: PathBuf,

    #[arg(short, long)]
    debug: bool,

    #[arg(short, long)]
    target: Option<String>,

    #[arg(long)]
    dry_run: bool,

    /// Override or set variables: --var KEY=VALUE
    #[arg(long = "var", value_name = "KEY=VALUE", num_args = 1)]
    vars: Vec<String>,
}

fn main() {
    let args = Args::parse();
    DEBUG.store(args.debug, Ordering::Relaxed);

    let mut vars: HashMap<String, Vec<String>> = HashMap::new();
    for kv in &args.vars {
        if let Some((k, v)) = kv.split_once('=') {
            vars.insert(k.to_string(), vec![v.to_string()]);
        } else {
            eprintln!("warning: --var {kv:?} ignored (expected KEY=VALUE)");
        }
    }

    if let Some((tokens, file)) = lex(args.file) {
        let statements = parse(&file, tokens);
        if DEBUG.load(Ordering::Relaxed) {
            pretty_print_statements(&statements);
        }
        build(statements, args.dry_run, args.target.as_deref(), vars);
    }
}