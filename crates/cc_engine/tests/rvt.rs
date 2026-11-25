//! Revit files: `rvt_import` converts a .rvt to IFC4 with the `rvt` crate in a child process and imports it.
#![cfg(feature = "rvt")]

use cc_engine::{ErrorCode, Session};
use serde_json::json;
use std::ffi::OsString;
use std::path::PathBuf;

/// A synthetic Revit 2024 file written by rvt-rs's `gen-fixture` (no Autodesk content).
fn fixture() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../cc_rvt/tests/fixtures/synthetic.rvt")
}

/// The converter of these tests, when this binary is started by
/// [`use_test_converter`]: an input named `*-hangs.rvt` never ends, one
/// named `*-crashes.rvt` aborts, others are converted.
#[test]
#[ignore = "the converter of the rvt tests, run as a child process"]
fn converter() {
    let args: Vec<OsString> = std::env::args_os().collect();
    let Some(at) = args.iter().position(|a| a == "--") else { return };
    let args = &args[at + 1..];
    let input = args.first().map(|a| a.to_string_lossy().into_owned()).unwrap_or_default();
    if input.ends_with("-hangs.rvt") {
        loop {
            std::thread::sleep(std::time::Duration::from_secs(1));
        }
    }
    if input.ends_with("-crashes.rvt") {
        std::process::abort();
    }
    std::process::exit(i32::from(cc_engine::rvt::child_main(args)));
}

/// `rvt_import` runs this test binary as its converter (the default, this
/// program with `rvt-to-ifc`, is `ccad` or the app).
fn use_test_converter() {
    let args = ["--exact", "rvt::converter", "--ignored", "--quiet", "--"];
    cc_engine::rvt::set_converter(Some(cc_engine::rvt::Converter {
        program: std::env::current_exe().unwrap(),
        args: args.iter().map(Into::into).collect(),
    }));
}

fn session_with_a_wall() -> Session {
    use_test_converter();
    let mut s = Session::default();
    s.execute("wall_create", json!({"start": [0.0, 0.0], "end": [5.0, 0.0]})).unwrap();
    s
}

fn temp_rvt(name: &str, data: &[u8]) -> PathBuf {
    let path = std::env::temp_dir().join(format!("ccad-{}-{name}", std::process::id()));
    std::fs::write(&path, data).unwrap();
    path
}

#[test]
fn a_revit_file_replaces_the_project() {
    let mut s = session_with_a_wall();
    let out = s.execute("rvt_import", json!({"path": fixture()})).unwrap();
    assert_eq!(out["schema"], "IFC4");
    assert_eq!(out["revit_version"], 2024);
    for key in ["elements", "with_geometry", "boxes"] {
        assert!(out["converted"][key].is_u64(), "{key}: {out}");
    }
    assert!(out["converted"]["notes"].is_array());
    assert_eq!(s.project().walls().count(), 0, "the import replaces the open project");
    assert!(s.doc.path.is_none());
}

#[test]
fn a_file_that_is_not_revit_is_an_error_and_keeps_the_project() {
    let path = temp_rvt("not-revit.rvt", b"ISO-10303-21;\nHEADER;\nENDSEC;\n");
    let mut s = session_with_a_wall();
    let err = s.execute("rvt_import", json!({"path": path})).unwrap_err();
    std::fs::remove_file(&path).ok();
    assert_eq!(err.code, ErrorCode::Io, "{err:?}");
    assert_eq!(s.project().walls().count(), 1);
    let missing = s.execute("rvt_import", json!({"path": "/no/such/file.rvt"})).unwrap_err();
    assert_eq!(missing.code, ErrorCode::Io);
}

/// A converter that crashes or never ends is an error; the session goes on with its project.
#[test]
fn a_converter_that_crashes_or_hangs_keeps_the_project() {
    let synthetic = std::fs::read(fixture()).unwrap();
    let mut s = session_with_a_wall();
    for (name, timeout) in [("crashes.rvt", 60.0), ("hangs.rvt", 0.5)] {
        let path = temp_rvt(name, &synthetic);
        let err = s.execute("rvt_import", json!({"path": path, "timeout": timeout})).unwrap_err();
        std::fs::remove_file(&path).ok();
        assert_eq!(err.code, ErrorCode::Io, "{name}: {err:?}");
        assert_eq!(s.project().walls().count(), 1, "{name}");
    }
}

/// A stop asked from another thread (the app's Cancel) kills the converter
/// long before the timeout; the command fails as cancelled and the project stays.
#[test]
fn a_stop_request_kills_the_converter_and_keeps_the_project() {
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, Ordering};
    let path = temp_rvt("stopped-hangs.rvt", &std::fs::read(fixture()).unwrap());
    let mut s = session_with_a_wall();
    let stop = Arc::new(AtomicBool::new(false));
    s.set_stop_flag(Arc::clone(&stop));
    let started = std::time::Instant::now();
    let canceller = std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_millis(300));
        stop.store(true, Ordering::Relaxed);
    });
    let err = s.execute("rvt_import", json!({"path": path})).unwrap_err();
    canceller.join().unwrap();
    std::fs::remove_file(&path).ok();
    assert_eq!(err.code, ErrorCode::Cancelled, "{err:?}");
    assert!(started.elapsed() < std::time::Duration::from_secs(30), "stopped, not timed out");
    assert_eq!(s.project().walls().count(), 1);
}

#[test]
fn the_timeout_is_a_positive_number_of_seconds() {
    let mut s = session_with_a_wall();
    for timeout in [0.0, -1.0, 1e300] {
        let err = s.execute("rvt_import", json!({"path": fixture(), "timeout": timeout})).unwrap_err();
        assert_eq!(err.code, ErrorCode::InvalidArguments, "{timeout}: {err:?}");
    }
}
