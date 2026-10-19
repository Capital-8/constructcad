//! Export a `.ccad` project to IFC4, or import an `.ifc` file and export it again.
//!
//! `cargo run -p cc_ifc --example export_ifc -- examples/casa_simples.ccad casa_simples.ifc`

use std::path::PathBuf;
use std::process::ExitCode;

fn main() -> ExitCode {
    let args: Vec<PathBuf> = std::env::args_os().skip(1).map(PathBuf::from).collect();
    let [input, output] = args.as_slice() else {
        eprintln!("usage: export_ifc <project.ccad|input.ifc> <output.ifc>");
        return ExitCode::FAILURE;
    };
    let is_ifc = input.extension().is_some_and(|e| e.eq_ignore_ascii_case("ifc"));
    let loaded = if is_ifc {
        cc_ifc::import_file(input, cc_core::Project::from_template()).map(|(p, _)| p).map_err(|e| e.to_string())
    } else {
        cc_core::load(input).map_err(|e| e.to_string())
    };
    let project = match loaded {
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
