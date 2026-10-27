//! `ccad`: ConstructCAD without a GUI. Runs engine commands from the
//! command line, from scripts, or for AI agents as an MCP server; all three
//! use the engine's one command catalog.

mod mcp;

use cc_engine::{EngineError, Session};
use serde_json::{Value, json};
use std::io::Read;
use std::path::PathBuf;
use std::process::ExitCode;

const USAGE: &str = "\
ccad - ConstructCAD without a GUI

Usage:
  ccad [options] commands             list the commands
  ccad [options] schema <command>     the JSON Schema of a command's parameters
  ccad reference                      the command reference in Markdown
  ccad [options] run <command> [args] run one command
  ccad [options] script <file|->      run a script of commands
  ccad [options] mcp                  serve the commands over MCP (stdio)

Arguments of `run`: a JSON object, @file.json, - (standard input), or
key=value pairs whose values are JSON or plain text, e.g.
  ccad -p house.ccad --save run wall_create start=[0,0] end=[5,0] height=2.8

A script is a JSON array of {\"command\": ..., \"args\": {...}} objects, or one
such object per line; it stops at the first error.

Options:
  -p, --project <file>   open a project (.ccad, or .ifc to import) first
  --save                 save the project afterwards (to its file)
  --save-as <file>       save the project afterwards to <file>
  --lang <code>          language of default names (default: the system's):
                         en, pt-BR, es, fr, de, it, lo, pl, uk-UA or zh-TW
  -V, --version          the version
  -h, --help             this help

Environment: CCAD_LANG sets the language like --lang.
";

struct Options {
    project: Option<PathBuf>,
    save: bool,
    save_as: Option<PathBuf>,
    rest: Vec<String>,
}

fn parse_options(args: Vec<String>) -> Result<Options, String> {
    let mut o = Options { project: None, save: false, save_as: None, rest: Vec::new() };
    let mut it = args.into_iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "-p" | "--project" => o.project = Some(it.next().ok_or("--project needs a file")?.into()),
            "--save" => o.save = true,
            "-V" | "--version" => o.rest.push("--version".into()),
            "--save-as" => o.save_as = Some(it.next().ok_or("--save-as needs a file")?.into()),
            "--lang" => {
                let code = it.next().ok_or("--lang needs a language")?;
                let lang = cc_engine::i18n::Language::from_code(&code).ok_or(format!("unsupported language {code}"))?;
                cc_engine::i18n::set_language(lang);
            }
            _ if o.rest.is_empty() && a.starts_with('-') && a != "-" => return Err(format!("unknown option {a}")),
            _ => {
                o.rest.push(a);
                o.rest.extend(it.by_ref());
            }
        }
    }
    Ok(o)
}

/// The session for the options: a new project, or the given file (`.ifc` is imported).
fn session(project: Option<&PathBuf>) -> Result<Session, EngineError> {
    let mut s = Session::default();
    if let Some(path) = project {
        let is_ifc = path.extension().is_some_and(|e| e.eq_ignore_ascii_case("ifc"));
        let command = if is_ifc { "ifc_import" } else { "project_open" };
        s.execute(command, json!({ "path": path }))?;
    }
    Ok(s)
}

fn read_input(spec: &str) -> Result<String, String> {
    if spec == "-" {
        let mut text = String::new();
        std::io::stdin().read_to_string(&mut text).map_err(|e| e.to_string())?;
        Ok(text)
    } else {
        std::fs::read_to_string(spec).map_err(|e| format!("{spec}: {e}"))
    }
}

/// Command arguments from the command line.
fn parse_args(args: &[String]) -> Result<Value, String> {
    match args {
        [] => Ok(json!({})),
        [one] if one.trim_start().starts_with('{') => serde_json::from_str(one).map_err(|e| e.to_string()),
        [one] if one == "-" || one.starts_with('@') => {
            let text = read_input(one.strip_prefix('@').unwrap_or(one))?;
            serde_json::from_str(&text).map_err(|e| e.to_string())
        }
        pairs => {
            let mut map = serde_json::Map::new();
            for pair in pairs {
                let (k, v) = pair.split_once('=').ok_or(format!("expected key=value, got '{pair}'"))?;
                let value = serde_json::from_str(v).unwrap_or_else(|_| Value::String(v.to_string()));
                map.insert(k.to_string(), value);
            }
            Ok(Value::Object(map))
        }
    }
}

fn print(v: &Value) {
    println!("{}", serde_json::to_string_pretty(v).unwrap_or_default());
}

fn fail(e: &EngineError) -> ExitCode {
    eprintln!("{}", serde_json::to_string_pretty(&json!({ "error": e })).unwrap_or_default());
    ExitCode::from(1)
}

fn usage_error(message: &str) -> ExitCode {
    eprintln!("ccad: {message}\n\n{USAGE}");
    ExitCode::from(2)
}

/// Save when asked (after the commands ran).
fn save(s: &mut Session, o: &Options) -> Result<(), EngineError> {
    if let Some(path) = &o.save_as {
        s.execute("project_save", json!({ "path": path }))?;
    } else if o.save {
        s.execute("project_save", json!({}))?;
    }
    Ok(())
}

fn main() -> ExitCode {
    if let Some(lang) = std::env::var("CCAD_LANG").ok().and_then(|c| cc_engine::i18n::Language::from_code(&c)) {
        cc_engine::i18n::set_language(lang);
    } else {
        cc_engine::i18n::set_language(cc_engine::i18n::Language::from_system());
    }
    let o = match parse_options(std::env::args().skip(1).collect()) {
        Ok(o) => o,
        Err(e) => return usage_error(&e),
    };
    let Some((sub, rest)) = o.rest.split_first() else {
        print!("{USAGE}");
        return ExitCode::SUCCESS;
    };
    match sub.as_str() {
        "--version" => {
            println!("ccad {}", env!("CARGO_PKG_VERSION"));
            ExitCode::SUCCESS
        }
        "-h" | "--help" | "help" => {
            print!("{USAGE}");
            ExitCode::SUCCESS
        }
        "commands" => {
            let list: Vec<Value> = cc_engine::catalog()
                .iter()
                .map(|c| json!({"name": c.name, "kind": c.kind, "description": c.description}))
                .collect();
            print(&json!(list));
            ExitCode::SUCCESS
        }
        "reference" => {
            print!("{}", cc_engine::reference::markdown());
            ExitCode::SUCCESS
        }
        "schema" => match rest.first().and_then(|n| cc_engine::describe(n)) {
            Some(info) => {
                print(&serde_json::to_value(&info).unwrap_or_default());
                ExitCode::SUCCESS
            }
            None => usage_error("schema needs a known command name"),
        },
        "run" => {
            let Some((name, args)) = rest.split_first() else { return usage_error("run needs a command") };
            let args = match parse_args(args) {
                Ok(a) => a,
                Err(e) => return usage_error(&format!("invalid arguments: {e}")),
            };
            let mut s = match session(o.project.as_ref()) {
                Ok(s) => s,
                Err(e) => return fail(&e),
            };
            match s.execute(name, args).and_then(|out| save(&mut s, &o).map(|()| out)) {
                Ok(out) => {
                    print(&out);
                    ExitCode::SUCCESS
                }
                Err(e) => fail(&e),
            }
        }
        "script" => {
            let Some(file) = rest.first() else { return usage_error("script needs a file (or -)") };
            let steps = match read_input(file).and_then(|t| cc_engine::script::parse(&t).map_err(|e| e.message)) {
                Ok(s) => s,
                Err(e) => return usage_error(&format!("invalid script: {e}")),
            };
            let mut s = match session(o.project.as_ref()) {
                Ok(s) => s,
                Err(e) => return fail(&e),
            };
            let results = match cc_engine::script::run(&mut s, &steps) {
                Ok(results) => results,
                Err((done, failed)) => {
                    print(&json!(
                        done.iter()
                            .zip(&steps)
                            .map(|(r, st)| json!({"command": st.command, "result": r}))
                            .collect::<Vec<_>>()
                    ));
                    eprintln!("step {} ({}) failed:", failed.index + 1, failed.command);
                    return fail(&failed.error);
                }
            };
            let results: Vec<Value> =
                results.into_iter().zip(&steps).map(|(r, st)| json!({"command": st.command, "result": r})).collect();
            if let Err(e) = save(&mut s, &o) {
                print(&json!(results));
                return fail(&e);
            }
            print(&json!(results));
            ExitCode::SUCCESS
        }
        "mcp" => {
            let s = match session(o.project.as_ref()) {
                Ok(s) => s,
                Err(e) => return fail(&e),
            };
            match mcp::serve(s, std::io::stdin().lock(), std::io::stdout().lock()) {
                Ok(()) => ExitCode::SUCCESS,
                Err(e) => {
                    eprintln!("ccad mcp: {e}");
                    ExitCode::from(1)
                }
            }
        }
        other => usage_error(&format!("unknown command {other}")),
    }
}
