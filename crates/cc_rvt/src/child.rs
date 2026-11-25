//! Running the converter in a child process, stopped after a timeout.

use crate::{Converted, RvtError, Summary};
use std::collections::hash_map::RandomState;
use std::ffi::OsString;
use std::hash::{BuildHasher, Hasher};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitStatus, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc;
use std::time::{Duration, Instant};

/// How long a conversion may take before it is stopped.
pub const DEFAULT_TIMEOUT: Duration = Duration::from_secs(600);
/// Time between two checks of the running converter.
const TICK: Duration = Duration::from_millis(20);
/// Bytes of the converter's standard error kept: the end, which names a crash.
const MAX_STDERR: usize = 8 * 1024;
/// How long the reader of standard error gets after the converter ended.
const READER_GRACE: Duration = Duration::from_secs(2);

/// The program that converts, with the arguments that come before the
/// converter's own (`<file.rvt> <out.ifc> <report.json>`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Converter {
    pub program: PathBuf,
    pub args: Vec<OsString>,
}

impl Converter {
    /// This program started again with [`SUBCOMMAND`](crate::SUBCOMMAND), for
    /// programs that call [`child_main`](crate::child_main) when they get it.
    pub fn this_program() -> Result<Self, RvtError> {
        let program = std::env::current_exe().map_err(|e| RvtError::Process(e.to_string()))?;
        Ok(Self { program, args: vec![crate::SUBCOMMAND.into()] })
    }
}

/// Convert the Revit file `input` to IFC4 in a child process that runs
/// `converter`, stopped when it takes longer than `timeout` or when `stop`
/// returns true (checked every few milliseconds; the caller cancelled). The
/// IFC file and the report are written to a new folder under the system's
/// temporary folder, deleted afterwards.
pub fn convert_in_child(
    converter: &Converter,
    input: &Path,
    timeout: Duration,
    stop: &dyn Fn() -> bool,
) -> Result<Converted, RvtError> {
    let folder = RunFolder::create()?;
    let (ifc, report) = (folder.0.join("model.ifc"), folder.0.join("report.json"));
    let mut command = Command::new(&converter.program);
    command
        .args(&converter.args)
        .arg(input)
        .arg(&ifc)
        .arg(&report)
        .current_dir(&folder.0)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped());
    #[cfg(windows)]
    {
        // No console window over the app.
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        command.creation_flags(CREATE_NO_WINDOW);
    }
    let process = |e: std::io::Error| RvtError::Process(format!("{}: {e}", converter.program.display()));
    let mut child = command.spawn().map_err(process)?;
    let (tx, rx) = mpsc::channel();
    if let Some(stream) = child.stderr.take() {
        let _ = std::thread::Builder::new().name("rvt-stderr".into()).spawn(move || {
            let _ = tx.send(read_tail(stream));
        });
    }
    let started = Instant::now();
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) => {}
            Err(e) => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(process(e));
            }
        }
        if stop() {
            let _ = child.kill();
            let _ = child.wait();
            return Err(RvtError::Cancelled);
        }
        if started.elapsed() >= timeout {
            let _ = child.kill();
            let _ = child.wait();
            let seconds = timeout.as_secs() + u64::from(timeout.subsec_nanos() > 0);
            return Err(RvtError::Timeout { seconds });
        }
        std::thread::sleep(TICK);
    };
    let Ok(bytes) = std::fs::read(&report) else {
        let stderr = rx.recv_timeout(READER_GRACE).unwrap_or_default();
        return Err(RvtError::Crashed(crash_detail(status, &stderr)));
    };
    let result: Result<Summary, RvtError> =
        serde_json::from_slice(&bytes).map_err(|e| RvtError::Process(format!("report: {e}")))?;
    let summary = result?;
    let ifc = std::fs::read(&ifc).map_err(|e| RvtError::Process(format!("{}: {e}", ifc.display())))?;
    Ok(Converted { ifc: String::from_utf8_lossy(&ifc).into_owned(), summary })
}

/// The last [`MAX_STDERR`] bytes of `stream`, read to its end.
fn read_tail(mut stream: impl Read) -> Vec<u8> {
    let mut tail = Vec::new();
    let mut buf = [0u8; 4096];
    loop {
        let n = match stream.read(&mut buf) {
            Ok(0) | Err(_) => break,
            Ok(n) => n,
        };
        tail.extend_from_slice(buf.get(..n).unwrap_or_default());
        if tail.len() > MAX_STDERR {
            tail.drain(..tail.len() - MAX_STDERR);
        }
    }
    tail
}

/// The last line the converter wrote to standard error, or its exit status.
fn crash_detail(status: ExitStatus, stderr: &[u8]) -> String {
    let text = String::from_utf8_lossy(stderr);
    let last = text.lines().map(str::trim).rfind(|l| !l.is_empty());
    last.map_or_else(|| status.to_string(), |l| l.chars().take(300).collect())
}

/// The folder of a conversion: created here, deleted when dropped.
struct RunFolder(PathBuf);

impl RunFolder {
    fn create() -> Result<RunFolder, RvtError> {
        let temp = std::env::temp_dir();
        let mut last = String::new();
        for _ in 0..16 {
            let dir = temp.join(format!("constructcraft-rvt-{}", random_suffix()));
            // `create_dir`, not `create_dir_all`: a folder that exists is not taken.
            match std::fs::create_dir(&dir) {
                Ok(()) => return Ok(RunFolder(dir)),
                Err(e) => last = e.to_string(),
            }
        }
        Err(RvtError::Process(format!("could not create a folder in {}: {last}", temp.display())))
    }
}

impl Drop for RunFolder {
    fn drop(&mut self) {
        // A killed converter may hold its files for a moment (Windows).
        for _ in 0..20 {
            if std::fs::remove_dir_all(&self.0).is_ok() || !self.0.exists() {
                return;
            }
            std::thread::sleep(Duration::from_millis(50));
        }
    }
}

fn random_suffix() -> String {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let mut h = RandomState::new().build_hasher();
    h.write_u64(COUNTER.fetch_add(1, Ordering::Relaxed));
    h.write_u32(std::process::id());
    if let Ok(t) = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH) {
        h.write_u128(t.as_nanos());
    }
    format!("{:016x}", h.finish())
}
