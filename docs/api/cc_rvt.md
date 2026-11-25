# `cc_rvt` API

{% raw %}
[← API reference](index.md) · Generated from the source by `cargo run -p api_doc`; edit the doc comments, not this page.

Revit project files (.rvt), read by the crates.io `rvt` crate (rvt-rs,
Apache-2.0, pure Rust, `forbid(unsafe_code)`) and written as IFC4 text,
which `cc_ifc` imports as it imports any IFC file.
`rvt` is not written to our robustness rules: a damaged file may make it
panic, overflow its stack or run without end. So the conversion runs in a
child process (`convert_in_child`), which is stopped after a timeout; a
crash or a stop is an error and the calling program goes on. The child is
a program that takes `SUBCOMMAND` and calls `child_main` (`ccad
rvt-to-ifc` and the app both do). Inside the child, the reader still runs
on its own thread with a large stack inside `catch_unwind`, so a panic
becomes an error in its report.
The reader's default open and walker limits bound what it reads; a file
they keep from being read whole is refused, as `rvt-ifc` refuses it, since
the model would silently miss what those streams hold.

## Modules

| Module | Items | Summary |
|---|---|---|
| [`cc_rvt`](#mod-cc_rvt) | 6 | Revit project files (.rvt), read by the crates.io `rvt` crate (rvt-rs, Apache-2.0, pure Rust, `forbid(unsafe_code)`) and written as IFC4 text, which `cc_ifc` imports as it imports any IFC file. |
| [`cc_rvt::child`](#mod-cc_rvt-child) | 3 | Running the converter in a child process, stopped after a timeout. |

<a id="mod-cc_rvt"></a>

## `cc_rvt`

<sub>`crates/cc_rvt/src/lib.rs`</sub>

**Re-exports**

- `pub use child::{Converter, DEFAULT_TIMEOUT, convert_in_child}`

**Items:** [`SUBCOMMAND`](#cc_rvt-subcommand) · [`MAX_RVT_BYTES`](#cc_rvt-max_rvt_bytes) · [`RvtError`](#cc_rvt-rvterror) · [`Summary`](#cc_rvt-summary) · [`Converted`](#cc_rvt-converted) · [`child_main`](#cc_rvt-child_main)

<a id="cc_rvt-subcommand"></a>

### const `SUBCOMMAND`

```rust
pub const SUBCOMMAND: &str
```

The argument that makes a program run as the converter: `<program>
rvt-to-ifc <file.rvt> <out.ifc> [<report.json>]`.

<a id="cc_rvt-max_rvt_bytes"></a>

### const `MAX_RVT_BYTES`

```rust
pub const MAX_RVT_BYTES: u64
```

The largest file read: the reader holds the whole file in memory.

<a id="cc_rvt-rvterror"></a>

### enum `RvtError`

```rust
pub enum RvtError
```

**Traits:** `Debug`, `Clone`, `PartialEq`, `Eq`, `Error`, `Serialize`, `Deserialize`

**Variants**

- `Read(String)`

  The reader refused the file: not a Revit file, damaged, or a part it cannot read.

- `Partial { streams: usize }`

  A resource limit kept this many streams from being read whole.

- `Internal(String)`

  The reader stopped on an internal error (a panic).

- `Timeout { seconds: u64 }`

  The converter did not finish within this many seconds and was stopped.

- `Cancelled`

  The caller cancelled the conversion and the converter was stopped.

- `Crashed(String)`

  The converter ended without a report: a crash, such as a stack
  overflow, or a kill. The text is its last error line or exit status.

- `Process(String)`

  The converter could not be started, or its files written or read.


<a id="cc_rvt-summary"></a>

### struct `Summary`

```rust
pub struct Summary
```

What a conversion recovered, besides the IFC text: the report the
converter writes.

**Traits:** `Debug`, `Clone`, `Default`, `PartialEq`, `Eq`, `Serialize`, `Deserialize`

**Fields**

- `revit_version: Option<u32>`

  The Revit release (year) that saved the file, when read.

- `elements: usize`

  Building elements written.

- `with_geometry: usize`

  Of those, the elements with a body.

- `boxes: usize`

  Bodies that are their element's bounding box: placed and sized, shape not decoded.

- `notes: Vec<String>`

  The converter's notes and warnings, in order (English, from the crate).


<a id="cc_rvt-converted"></a>

### struct `Converted`

```rust
pub struct Converted
```

A Revit file written as IFC4.

**Traits:** `Debug`, `Clone`

**Fields**

- `ifc: String`

  The IFC4 file, as text.

- `summary: Summary`

<a id="cc_rvt-child_main"></a>

### fn `child_main`

```rust
pub fn child_main(args: &[OsString]) -> u8
```

The converter's side of `convert_in_child`: `args` are what follows
`SUBCOMMAND`, `<file.rvt> <out.ifc> [<report.json>]`. It writes the IFC
file and the report, a JSON `{"Ok": Summary}` or `{"Err": RvtError}`, to
`<report.json>` or else to standard output. Returns the exit code: 0
converted, 1 not converted, 2 wrong arguments.

<a id="mod-cc_rvt-child"></a>

## `cc_rvt::child`

<sub>`crates/cc_rvt/src/child.rs`</sub>

Running the converter in a child process, stopped after a timeout.

**Items:** [`DEFAULT_TIMEOUT`](#cc_rvt-child-default_timeout) · [`Converter`](#cc_rvt-child-converter) · [`convert_in_child`](#cc_rvt-child-convert_in_child)

<a id="cc_rvt-child-default_timeout"></a>

### const `DEFAULT_TIMEOUT`

```rust
pub const DEFAULT_TIMEOUT: Duration
```

How long a conversion may take before it is stopped.

<a id="cc_rvt-child-converter"></a>

### struct `Converter`

```rust
pub struct Converter
```

The program that converts, with the arguments that come before the
converter's own (`<file.rvt> <out.ifc> <report.json>`).

**Traits:** `Debug`, `Clone`, `PartialEq`, `Eq`

**Fields**

- `program: PathBuf`
- `args: Vec<OsString>`

**Methods**

- `pub fn this_program() -> Result<Self, RvtError>`

  This program started again with [`SUBCOMMAND`](crate::SUBCOMMAND), for
  programs that call [`child_main`](crate::child_main) when they get it.


<a id="cc_rvt-child-convert_in_child"></a>

### fn `convert_in_child`

```rust
pub fn convert_in_child(converter: &Converter, input: &Path, timeout: Duration, stop: &dyn Fn() -> bool) -> Result<Converted, RvtError>
```

Convert the Revit file `input` to IFC4 in a child process that runs
`converter`, stopped when it takes longer than `timeout` or when `stop`
returns true (checked every few milliseconds; the caller cancelled). The
IFC file and the report are written to a new folder under the system's
temporary folder, deleted afterwards.

{% endraw %}
