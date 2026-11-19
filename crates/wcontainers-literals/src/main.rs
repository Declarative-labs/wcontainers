//! The command line; see the library's `usage()`.

use std::process::ExitCode;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match wcontainers_literals::main_with(&args) {
        Ok(out) => {
            out.lines
                .iter()
                .chain(&out.marks)
                .for_each(|l| println!("{l}"));
            out.summary.iter().for_each(|l| eprintln!("{l}"));
            if out.clean {
                ExitCode::SUCCESS
            } else {
                ExitCode::FAILURE
            }
        }
        Err(e) => {
            eprintln!("{}: {e}", env!("CARGO_PKG_NAME"));
            ExitCode::FAILURE
        }
    }
}
