# Automation: engine, command line and MCP

[Português](automacao.md)

Everything ConstructCAD does to a model can be done without the interface. The **engine** (`cc_engine`) holds the operations as a catalog of named **commands**; three clients use that one catalog:

```
                 ┌──────────────────────────────┐
  desktop app ──▶│                              │
  ccad (CLI)  ──▶│  cc_engine: Session +        │──▶ the model (.ccad)
  MCP agents  ──▶│  catalog of commands         │
                 └──────────────────────────────┘
```

A wall drawn with the mouse, a `ccad run wall_create …` and an AI agent calling the `wall_create` tool run the same code, with the same defaults, checks and undo. The full list of commands with their parameters is in the [command reference](commands.md).

## Concepts

**Session.** One open project: the document (with its undo history) plus geometry and drawings derived from it. The CLI opens one per invocation (or script), the MCP server one per connection, the app one per window.

**Commands.** Each command has a `snake_case` name, a JSON object of parameters described by a JSON Schema, and a JSON result. Kinds:

| Kind | Does | Examples |
|---|---|---|
| query | reads the model | `elements_list`, `element_get`, `schedule_table` |
| edit | changes the model: one undo step | `wall_create`, `elements_move`, `element_set` |
| file | works with files or replaces the project | `project_open`, `ifc_import`, `sheet_export` |

**Units and coordinates.** Lengths are meters, angles degrees, points `[x, y]` in plan. Dimensions use the coordinates of their view's drawing (plan X/Y in plans; horizontal position and height in elevations and sections). Areas and volumes in results are m² and m³.

**References.** Levels, types, views and materials are given by **id or name** (`"level": 3` or `"level": "Ground"`; names are matched without regard to case; an ambiguous name is an error asking for the id). Sheets can also be named by their number (`"A101"`). Elements are given by id; `elements_list` and the results of creating commands give them. Omitted levels default to the lowest level and omitted types to the category's default type.

**Reading and changing parameters.** `element_get`, `type_get` and `view_get` show an object's `params`; `element_set`, `type_set` and `view_set` take the fields to change in the same shape and merge them in:

```sh
ccad -p house.ccad run element_get id=42
ccad -p house.ccad --save run element_set id=42 'params={"height": 2.7}'
ccad -p house.ccad --save run element_set id=57 'params={"shape": {"Gable": {"slope": 0.5, "ridge_deg": 0}}}'
```

Objects merge field by field; arrays and plain values replace; `null` clears an optional value. `material_set`, `units_set` (shaped like `material_get`, `units_get`) work alike.

**Validation and undo.** Every edit is one undo step (`undo`, `redo`). An edit that would leave the model invalid — a door pointing at a missing wall, a negative size, a reference to nothing — is refused and leaves no trace. Pinned elements are not moved, trimmed or deleted.

**Errors.** A failed command returns `{"error": {"code": …, "message": …}}`. Messages are in the session's language (`--lang`, `language_set`).

| Code | Meaning |
|---|---|
| `unknown_command` | no command has that name |
| `invalid_arguments` | the parameters do not match the schema or make no sense (zero length, unknown kind…) |
| `not_found` | a referenced element, type, level, view or material does not exist |
| `rejected` | the edit would leave the model invalid, or is not allowed (pinned, in use…) |
| `io` | reading or writing a file failed |
| `internal` | a bug; the project is kept as it was |

## Command line: `ccad`

`ccad` is a separate program without graphics dependencies (it runs on servers and in CI). Release packages include it; from the sources: `cargo build --release -p cc_cli` (the binary is `target/release/ccad`).

```text
ccad [options] commands             list the commands
ccad [options] schema <command>     the JSON Schema of a command's parameters
ccad reference                      the command reference in Markdown
ccad [options] run <command> [args] run one command
ccad [options] script <file|->      run a script of commands
ccad [options] mcp                  serve the commands over MCP (stdio)

-p, --project <file>   open a project (.ccad, or .ifc to import) first
--save                 save the project afterwards (to its file)
--save-as <file>       save the project afterwards to <file>
--lang <en|pt-BR>      language of default names and messages (also CCAD_LANG)
```

Arguments of `run` can be a JSON object, `@file.json`, `-` (standard input) or `key=value` pairs, whose values are read as JSON when they parse and as text otherwise:

```sh
ccad -p house.ccad --save run wall_create start=[0,0] end=[5,0] height=2.8
ccad -p house.ccad run elements_list category=door
ccad -p house.ccad --save run view_set '{"view": "Section 1", "scale": 100}'
ccad -p model.ifc --save-as model.ccad run project_info              # import an IFC file
ccad -p house.ccad run sheet_export sheet=A101 path=a101.pdf
```

Results are printed as JSON on standard output. Errors go to standard error as JSON with exit code 1; usage errors exit with 2. Without `--save` or `--save-as` the project file is not changed.

Results combine with other tools; for example, with `jq`:

```sh
wall=$(ccad -p house.ccad --save run wall_create start=[0,0] end=[6,0] | jq .id)
ccad -p house.ccad --save run door_create wall=$wall offset=1.5
```

### Scripts

A script is a list of steps, either a JSON array of `{"command": …, "args": {…}}` objects or one such object per line (lines starting with `//` are comments). `ccad script` runs the steps in order in one session and stops at the first error; each step is one undo step.

[`examples/small_house.ccmacro`](../examples/small_house.ccmacro) builds a small house:

```text
// A small house built with engine commands only.
{"command": "wall_create", "args": {"start": [0, 0], "end": [8, 0], "height": 2.8}}
…
{"command": "door_create", "args": {"point": [2, -0.5]}}
{"command": "floor_create", "args": {"from_walls": true}}
{"command": "room_create", "args": {"point": [2, 3], "name": "Living room"}}
```

```sh
ccad --save-as house.ccad script examples/small_house.ccmacro
```

Scripts have no variables: a step cannot use an id created by an earlier one. Commands help by accepting positions where it matters (`door_create` and `window_create` find the wall nearest `point`; floors, ceilings and roofs follow the walls with `from_walls`). For logic between steps, call `ccad run` from a shell script or use an agent over MCP.

## Macros in the app

*Manage → Macros → Record Macro* records the changes you make until you press *Stop Recording*; the macro is saved as a script (`.ccmacro`, the format above). *Run Macro* plays a macro or any script on the open project; `ccad script` runs the same files.

- Only edits are recorded: opening, saving and exporting are left out, so replaying never overwrites files.
- A value dragged in the properties is recorded once, with its final value; changes made in a dialog that you cancel are not recorded.
- Recorded steps refer to elements by id. Played on the project it was recorded on (or on a copy of it as it was when recording started) a macro reproduces the same changes; on another project, steps that name elements by id may fail or reach other elements.

## MCP server

`ccad mcp` is a [Model Context Protocol](https://modelcontextprotocol.io/) server on standard input/output. Every command is a tool with the same name, description and input schema; queries are marked read-only. The server keeps one session, so an agent opens or creates a project, works on it over many calls and saves it.

**Claude Code:**

```sh
claude mcp add constructcad -- ccad mcp
# or with a project open from the start
claude mcp add constructcad -- ccad mcp -p /path/to/house.ccad
```

**Claude Desktop and other clients** (`claude_desktop_config.json` or the client's MCP settings; use the full path of `ccad`):

```json
{
  "mcpServers": {
    "constructcad": { "command": "/path/to/ccad", "args": ["mcp", "--lang", "en"] }
  }
}
```

Requests an agent can handle with these tools, for example: *"open house.ccad, list the doors without a mark and number them"*, *"import model.ifc and tell me the volume of the concrete walls"*, *"add a second floor 3 m above the ground floor with the same exterior walls"*, *"export sheet A101 as PDF"*.

Notes:

- Paths are on the machine where `ccad` runs.
- Errors come back as tool results with `isError` and the error object, so the model can correct its call.
- The server speaks protocol revisions 2025-06-18, 2025-03-26 and 2024-11-05 and is implemented in `crates/cc_cli/src/mcp.rs` (JSON-RPC 2.0, one message per line).

## For developers: adding a command

Every user-facing feature starts as an engine command; the app calls it rather than editing the document itself.

1. Write the parameter struct in `crates/cc_engine/src/commands/<area>.rs`: `#[derive(Debug, Deserialize, Serialize, JsonSchema)]`, `#[serde(deny_unknown_fields)]`, a doc comment (the command's description) and a doc comment on every field (its parameter description; a test fails without them).
2. Implement `Command` with `NAME` (`snake_case`), `KIND` and `run`. Resolve references with `refs`, change the model inside `commands::edit` (one undo step, refused if the result needs repair), and return JSON (`created(s, id)` for elements). Use `t!` messages for errors people will read.
3. Register it in its group in `registry.rs`.
4. Test it in `crates/cc_engine/tests/commands.rs` through `Session::execute`.
5. Regenerate the reference: `cargo run -p cc_cli -- reference > docs/commands.md` (a test fails while it is stale).
6. In the app, call it with `Session::run(Command { … })`, `actions::run_command` or the `engine_edit` helpers (use `Session::run_merged` for dragged values). That also makes it recordable in macros.

Architecture details are in [AGENTS.md](../AGENTS.md).
