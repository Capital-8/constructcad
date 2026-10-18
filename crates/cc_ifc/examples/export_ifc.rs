//! Export a `.ccad` project to IFC4.
//!
//! `cargo run -p cc_ifc --example export_ifc -- examples/casa_simples.ccad casa_simples.ifc`

use std::path::PathBuf;
use std::process::ExitCode;

fn main() -> ExitCode {
    let args: Vec<PathBuf> = std::env::args_os().skip(1).map(PathBuf::from).collect();
    let [input, output] = args.as_slice() else {
        eprintln!("usage: export_ifc <project.ccad> <output.ifc>");
        return ExitCode::FAILURE;
    };
    let project = match cc_core::load(input) {
        Ok(p) => p,
        Err(err) => {
            eprintln!("cannot load {}: {err}", input.display());
            return ExitCode::FAILURE;
        }
    };
    if let Err(err) = cc_ifc::export_to_file(&project, output) {
        eprintln!("cannot write {}: {err}", output.display());
        return ExitCode::FAILURE;
    }
    println!("wrote {}", output.display());
    ExitCode::SUCCESS
}
