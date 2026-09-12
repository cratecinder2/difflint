mod diff;
mod print;

use std::env;
use std::fs;
use std::io::{self, Read};
use std::process::ExitCode;

fn main() -> ExitCode {
    let mut json_mode = false;
    let mut path: Option<String> = None;

    for arg in env::args().skip(1) {
        match arg.as_str() {
            "--json" => json_mode = true,
            "-h" | "--help" => {
                print_usage();
                return ExitCode::SUCCESS;
            }
            other => {
                if path.is_some() {
                    eprintln!("unexpected extra argument: {}", other);
                    return ExitCode::FAILURE;
                }
                path = Some(other.to_string());
            }
        }
    }

    let input = match read_input(path.as_deref()) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("failed to read input: {}", e);
            return ExitCode::FAILURE;
        }
    };

    match diff::parse(&input) {
        Ok(files) => {
            if json_mode {
                println!("{}", print::print_json(&files));
            } else {
                print!("{}", print::print_human(&files));
            }
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("parse error: {}", e);
            ExitCode::FAILURE
        }
    }
}

fn read_input(path: Option<&str>) -> io::Result<String> {
    match path {
        Some(p) if p != "-" => fs::read_to_string(p),
        _ => {
            let mut buf = String::new();
            io::stdin().read_to_string(&mut buf)?;
            Ok(buf)
        }
    }
}

fn print_usage() {
    eprintln!("usage: difflint [--json] [FILE]");
    eprintln!();
    eprintln!("Reads a unified diff from FILE (or stdin if omitted or '-'),");
    eprintln!("validates the hunk headers against the actual line counts,");
    eprintln!("and pretty-prints the result.");
    eprintln!();
    eprintln!("  --json   emit machine-readable JSON instead of the default text form");
}
