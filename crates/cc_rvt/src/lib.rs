//! Revit project files (.rvt), read by the crates.io `rvt` crate (rvt-rs,
//! Apache-2.0, pure Rust, `forbid(unsafe_code)`) and written as IFC4 text,
//! which `cc_ifc` imports as it imports any IFC file.
//!
//! `rvt` is not written to our robustness rules: a damaged file may make it
//! panic, overflow its stack or run without end. So the conversion runs in a
//! child process ([`convert_in_child`]), which is stopped after a timeout; a
//! crash or a stop is an error and the calling program goes on. The child is
//! a program that takes [`SUBCOMMAND`] and calls [`child_main`] (`ccad
//! rvt-to-ifc` and the app both do). Inside the child, the reader still runs
//! on its own thread with a large stack inside `catch_unwind`, so a panic
//! becomes an error in its report.
//!
//! The reader's default open and walker limits bound what it reads; a file
//! they keep from being read whole is refused, as `rvt-ifc` refuses it, since
//! the model would silently miss what those streams hold.

mod child;

pub use child::{Converter, DEFAULT_TIMEOUT, convert_in_child};

use rvt::RevitFile;
use rvt::ifc::step_writer::{StepOptions, write_step_to};
use rvt::ifc::{ExportOptions, run_export_with_limits};
use rvt::reader::OpenLimits;
use rvt::walker::WalkerLimits;
use serde::{Deserialize, Serialize};
use std::ffi::OsString;
use std::io::Write;
use std::path::{Path, PathBuf};

/// The argument that makes a program run as the converter: `<program>
/// rvt-to-ifc <file.rvt> <out.ifc> [<report.json>]`.
pub const SUBCOMMAND: &str = "rvt-to-ifc";

/// Stack of the reading thread (the reader recurses over nested records).
const STACK_BYTES: usize = 128 << 20;

/// The largest file read: the reader holds the whole file in memory.
pub const MAX_RVT_BYTES: u64 = rvt::reader::DEFAULT_MAX_FILE_BYTES;

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RvtError {
    /// The reader refused the file: not a Revit file, damaged, or a part it cannot read.
    #[error("{0}")]
    Read(String),
    /// A resource limit kept this many streams from being read whole.
    #[error("{streams} stream(s) of the file are too large to read whole")]
    Partial { streams: usize },
    /// The reader stopped on an internal error (a panic).
    #[error("{0}")]
    Internal(String),
    /// The converter did not finish within this many seconds and was stopped.
    #[error("the converter did not finish within {seconds} s")]
    Timeout { seconds: u64 },
    /// The caller cancelled the conversion and the converter was stopped.
    #[error("the conversion was cancelled")]
    Cancelled,
    /// The converter ended without a report: a crash, such as a stack
    /// overflow, or a kill. The text is its last error line or exit status.
    #[error("the converter stopped without a result ({0})")]
    Crashed(String),
    /// The converter could not be started, or its files written or read.
    #[error("{0}")]
    Process(String),
}

/// What a conversion recovered, besides the IFC text: the report the
/// converter writes.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Summary {
    /// The Revit release (year) that saved the file, when read.
    pub revit_version: Option<u32>,
    /// Building elements written.
    pub elements: usize,
    /// Of those, the elements with a body.
    pub with_geometry: usize,
    /// Bodies that are their element's bounding box: placed and sized, shape not decoded.
    pub boxes: usize,
    /// The converter's notes and warnings, in order (English, from the crate).
    pub notes: Vec<String>,
}

/// A Revit file written as IFC4.
#[derive(Debug, Clone)]
pub struct Converted {
    /// The IFC4 file, as text.
    pub ifc: String,
    pub summary: Summary,
}

/// The converter's side of [`convert_in_child`]: `args` are what follows
/// [`SUBCOMMAND`], `<file.rvt> <out.ifc> [<report.json>]`. It writes the IFC
/// file and the report, a JSON `{"Ok": Summary}` or `{"Err": RvtError}`, to
/// `<report.json>` or else to standard output. Returns the exit code: 0
/// converted, 1 not converted, 2 wrong arguments.
pub fn child_main(args: &[OsString]) -> u8 {
    let (input, output, report) = match args {
        [input, output] => (input, output, None),
        [input, output, report] => (input, output, Some(report)),
        _ => {
            eprintln!("usage: {SUBCOMMAND} <file.rvt> <out.ifc> [<report.json>]");
            return 2;
        }
    };
    let result = convert_file(Path::new(input), PathBuf::from(output));
    let code = if result.is_ok() { 0 } else { 1 };
    let json = match serde_json::to_string_pretty(&result) {
        Ok(json) => json,
        Err(e) => {
            eprintln!("{SUBCOMMAND}: {e}");
            return 1;
        }
    };
    match report {
        Some(path) => {
            if let Err(e) = std::fs::write(path, json) {
                eprintln!("{SUBCOMMAND}: {}: {e}", Path::new(path).display());
                return 1;
            }
        }
        None => println!("{json}"),
    }
    code
}

/// Convert the Revit file `input` to the IFC4 file `output`, in this process.
fn convert_file(input: &Path, output: PathBuf) -> Result<Summary, RvtError> {
    let process = |e: std::io::Error| RvtError::Process(e.to_string());
    let len = std::fs::metadata(input).map_err(process)?.len();
    if len > MAX_RVT_BYTES {
        return Err(RvtError::Read(format!("the file is {len} bytes, over the limit of {MAX_RVT_BYTES}")));
    }
    let data = std::fs::read(input).map_err(process)?;
    on_reader_thread(move || {
        let mut out = std::io::BufWriter::new(std::fs::File::create(&output).map_err(process)?);
        let summary = convert(data, &mut out)?;
        out.flush().map_err(process)?;
        Ok(summary)
    })
}

/// Run `f` on a thread with a large stack, inside `catch_unwind`.
fn on_reader_thread<T: Send + 'static>(
    f: impl FnOnce() -> Result<T, RvtError> + Send + 'static,
) -> Result<T, RvtError> {
    let worker = std::thread::Builder::new()
        .name("rvt-reader".into())
        .stack_size(STACK_BYTES)
        .spawn(move || std::panic::catch_unwind(std::panic::AssertUnwindSafe(f)))
        .map_err(|e| RvtError::Internal(e.to_string()))?;
    match worker.join() {
        Ok(Ok(result)) => result,
        Ok(Err(payload)) | Err(payload) => Err(RvtError::Internal(panic_message(payload.as_ref()))),
    }
}

/// Read the Revit file `data` and write it to `out` as IFC4.
fn convert(data: Vec<u8>, out: &mut dyn Write) -> Result<Summary, RvtError> {
    let read = |e: rvt::Error| RvtError::Read(e.to_string());
    let mut file = RevitFile::open_bytes_with_limits(data, OpenLimits::default()).map_err(read)?;
    let run = run_export_with_limits(&mut file, &ExportOptions::default(), WalkerLimits::default()).map_err(read)?;
    if run.diagnostics.is_partial() {
        return Err(RvtError::Partial { streams: run.unread.len().max(1) });
    }
    // The model is complete: release the file and the partitions it inflated before writing.
    drop(file);
    write_step_to(&run.model, &StepOptions::default(), out).map_err(|e| RvtError::Process(e.to_string()))?;
    let d = &run.diagnostics;
    Ok(Summary {
        revit_version: d.input.revit_version,
        elements: d.exported.building_elements,
        with_geometry: d.exported.building_elements_with_geometry,
        boxes: d.exported.bounding_box_bodies,
        notes: run.notes.iter().map(ToString::to_string).chain(d.warnings.iter().cloned()).collect(),
    })
}

fn panic_message(payload: &(dyn std::any::Any + Send)) -> String {
    payload
        .downcast_ref::<&str>()
        .map(|s| s.to_string())
        .or_else(|| payload.downcast_ref::<String>().cloned())
        .unwrap_or_else(|| "internal error".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    /// A synthetic Revit file written by rvt-rs's `gen-fixture` (no Autodesk content):
    /// `gen-fixture synthetic -o synthetic.rvt` with rvt 0.25.0.
    const SYNTHETIC: &[u8] = include_bytes!("../tests/fixtures/synthetic.rvt");

    /// Convert in this process, as the child does.
    fn to_ifc(data: Vec<u8>) -> Result<Converted, RvtError> {
        on_reader_thread(move || {
            let mut bytes = Vec::new();
            let summary = convert(data, &mut bytes)?;
            Ok(Converted { ifc: String::from_utf8_lossy(&bytes).into_owned(), summary })
        })
    }

    /// This test binary run again as the converter: the test `name`, which
    /// calls [`as_child`], with the converter's arguments after `--`.
    fn this_binary_as(name: &str) -> Converter {
        let args = ["--exact", name, "--ignored", "--quiet", "--"];
        Converter { program: std::env::current_exe().unwrap(), args: args.iter().map(Into::into).collect() }
    }

    /// The converter's arguments when this process was started by [`this_binary_as`].
    fn child_args() -> Option<Vec<OsString>> {
        let args: Vec<OsString> = std::env::args_os().collect();
        let at = args.iter().position(|a| a == "--")?;
        Some(args[at + 1..].to_vec())
    }

    /// In a child, run `f` on the converter's arguments and exit with its code.
    fn as_child(f: impl FnOnce(&[OsString]) -> u8) {
        if let Some(args) = child_args() {
            std::process::exit(i32::from(f(&args)));
        }
    }

    #[test]
    #[ignore = "the converter of the child process tests"]
    fn child_converts() {
        as_child(child_main);
    }

    #[test]
    #[ignore = "a converter that never ends, for the child process tests"]
    fn child_hangs() {
        as_child(|_| {
            loop {
                std::thread::sleep(Duration::from_secs(1));
            }
        });
    }

    /// Deep enough to overflow any stack.
    fn recurse(depth: u64) -> u64 {
        if depth == u64::MAX {
            return 0;
        }
        let frame = std::hint::black_box([depth; 512]);
        recurse(frame[0] + 1) + frame[511]
    }

    #[test]
    #[ignore = "a converter that overflows its stack, for the child process tests"]
    fn child_overflows_its_stack() {
        as_child(|_| u8::try_from(recurse(0) % 2).unwrap_or(0));
    }

    fn temp_file(name: &str, data: &[u8]) -> PathBuf {
        let path = std::env::temp_dir().join(format!("ccad-rvt-{}-{name}", std::process::id()));
        std::fs::write(&path, data).unwrap();
        path
    }

    #[test]
    fn a_revit_file_becomes_an_ifc4_project() {
        let out = to_ifc(SYNTHETIC.to_vec()).unwrap();
        assert!(out.ifc.contains("FILE_SCHEMA(('IFC4'))"), "{}", &out.ifc[..out.ifc.len().min(400)]);
        assert!(out.ifc.contains("IFCPROJECT("));
        assert_eq!(out.summary.revit_version, Some(2024));
    }

    #[test]
    fn other_files_are_errors() {
        for data in [Vec::new(), b"ISO-10303-21;".to_vec(), vec![0xd0, 0xcf, 0x11, 0xe0, 0xa1, 0xb1, 0x1a, 0xe1]] {
            assert!(matches!(to_ifc(data), Err(RvtError::Read(_))));
        }
    }

    /// Damaged files never panic: every prefix and a few flipped bytes of a real file.
    #[test]
    fn damaged_files_are_errors_or_conversions() {
        for len in (0..SYNTHETIC.len()).step_by(997) {
            let _ = to_ifc(SYNTHETIC[..len].to_vec());
        }
        for at in (512..SYNTHETIC.len()).step_by(1499) {
            let mut data = SYNTHETIC.to_vec();
            data[at] ^= 0xff;
            let _ = to_ifc(data);
        }
    }

    #[test]
    fn the_child_converts_a_revit_file() {
        let input = temp_file("child.rvt", SYNTHETIC);
        let out = convert_in_child(&this_binary_as("tests::child_converts"), &input, DEFAULT_TIMEOUT, &|| false);
        std::fs::remove_file(&input).ok();
        let out = out.unwrap();
        assert!(out.ifc.contains("IFCPROJECT("));
        assert_eq!(out.summary, to_ifc(SYNTHETIC.to_vec()).unwrap().summary);
    }

    #[test]
    fn the_child_reports_what_it_could_not_read() {
        let input = temp_file("not-revit.rvt", b"ISO-10303-21;");
        let out = convert_in_child(&this_binary_as("tests::child_converts"), &input, DEFAULT_TIMEOUT, &|| false);
        std::fs::remove_file(&input).ok();
        assert!(matches!(out, Err(RvtError::Read(_))), "{out:?}");
    }

    #[test]
    fn a_converter_that_never_ends_is_stopped() {
        let input = temp_file("hangs.rvt", SYNTHETIC);
        let started = std::time::Instant::now();
        let out =
            convert_in_child(&this_binary_as("tests::child_hangs"), &input, Duration::from_millis(500), &|| false);
        std::fs::remove_file(&input).ok();
        assert_eq!(out.unwrap_err(), RvtError::Timeout { seconds: 1 });
        assert!(started.elapsed() < Duration::from_secs(30));
    }

    #[test]
    fn a_cancelled_conversion_stops_the_converter() {
        let input = temp_file("cancelled.rvt", SYNTHETIC);
        let started = std::time::Instant::now();
        let stop = || started.elapsed() > Duration::from_millis(300);
        let out = convert_in_child(&this_binary_as("tests::child_hangs"), &input, DEFAULT_TIMEOUT, &stop);
        std::fs::remove_file(&input).ok();
        assert_eq!(out.unwrap_err(), RvtError::Cancelled);
        assert!(started.elapsed() < Duration::from_secs(30));
    }

    /// A stack overflow aborts the child only.
    #[test]
    fn a_converter_that_overflows_its_stack_is_an_error() {
        let input = temp_file("overflows.rvt", SYNTHETIC);
        let out =
            convert_in_child(&this_binary_as("tests::child_overflows_its_stack"), &input, DEFAULT_TIMEOUT, &|| false);
        std::fs::remove_file(&input).ok();
        assert!(matches!(out, Err(RvtError::Crashed(_))), "{out:?}");
    }

    #[test]
    fn a_converter_that_cannot_start_is_an_error() {
        let converter = Converter { program: PathBuf::from("/no/such/converter"), args: Vec::new() };
        let out = convert_in_child(&converter, Path::new("in.rvt"), DEFAULT_TIMEOUT, &|| false);
        assert!(matches!(out, Err(RvtError::Process(_))), "{out:?}");
    }

    #[test]
    fn wrong_arguments_are_a_usage_error() {
        assert_eq!(child_main(&[]), 2);
        assert_eq!(child_main(&["a".into(), "b".into(), "c".into(), "d".into()]), 2);
    }
}
