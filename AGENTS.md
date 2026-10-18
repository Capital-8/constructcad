# AGENTS.md

Guidance for AI coding agents working in this repository.

## Commands

```sh
cargo run -p cc_app                                   # app (opens the home screen)
cargo run -p cc_app -- examples/casa_simples.ccad     # open a project
cargo run -p cc_app -- examples/casa_simples.ccad --view "Elevação: Sul" --lang en
cargo run -p cc_core --example casa_simples           # regenerate examples/casa_simples.ccad

cargo test --workspace                                # all tests
cargo test -p cc_geom rooms                           # tests of one crate matching a name
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all -- --check                            # max_width = 120 (rustfmt.toml)

# IFC validation (CI only; IfcOpenShell is never a build/runtime dependency)
pip install -r scripts/requirements-ifc.txt
cargo run -p cc_ifc --example export_ifc -- examples/casa_simples.ccad out.ifc
python scripts/validate_ifc.py out.ifc
```

CI (`.github/workflows/ci.yml`) runs fmt, clippy + tests on Linux/Windows/macOS, the IFC validation job (exports `examples/casa_simples.ccad` and `crates/cc_ifc/tests/fixtures/features.ccad`), and a DCO check on PRs: commits must be signed off (`git commit -s`).

## Architecture

Rust workspace (edition 2024). Dependency direction: `cc_core` ← `cc_geom` ← `cc_views` / `cc_render` / `cc_ifc` ← `cc_app`. `cc_core` knows nothing of geometry, views or UI; no crate depends on `cc_app`.

- **cc_core** — the BIM model. `Project` (levels, materials, `types: BTreeMap<ElementTypeId, ElementType>`, `elements`, `views`) is the single source of truth. Every edit goes through `Document::edit` / `edit_merged`, which records a reversible `Transaction` (elements, views, types and project "settings" are tracked); continuous edits use `edit_merged` + `break_merge`. `Uid` is a persistent 128-bit id on project/types/elements (used as IFC GlobalId); `Document::new`, `from_ron` and `add_element/add_type` assign missing or duplicated ones. Files are versioned RON (`persist.rs`, `FORMAT_VERSION`): new fields must have serde defaults so older files load; bump the version for semantic changes. Old projects get missing ids and default annotation types on load.
- **cc_geom** — derived geometry, rebuilt from scratch by `ModelGeometry::build` whenever the document revision changes. Walls: joins are resolved per level on the body centerline (`Project::wall_axis`, which applies the wall's `LocationLine` and the type's core layers); base/top come from `Project::wall_base_z/wall_top_z` (top may be attached to a level). `WallGeom` holds columns between openings; per-wall volume is analytic (the body solid omits caps at joins). `hosted_parts` defines door/window boxes shared by the 3D view and IFC. `rooms.rs` finds a room boundary by tracing the face of the wall-footprint arrangement around the room point.
- **cc_views** — views are queries over the model: `generate(project, geom, view)` returns a resolution-independent `Drawing2D` (lines with `LineStyle`, fills, anchored/rotated text in paper mm). Plans (`plan.rs`), projected views (`projected.rs`: elevations, sections, axonometric, painter's algorithm ordered by pairwise depth comparison of overlapping faces), legends, schedules (`schedule.rs`, headers are i18n keys) and annotations (`annotate.rs`: room/door/window tags, grids, dimensions). Dimensions store points in the view's own drawing coordinates, so they work in any 2D view. A fill with alpha 0 is not hit-testable and only shows when its element is highlighted (room areas).
- **cc_render** — offscreen wgpu renderer for 3D views (`Renderer3D`); `render_to_rgba` reads a frame back (home screen thumbnails).
- **cc_ifc** — IFC4 export with an in-house STEP writer (`step.rs`); no third-party IFC libraries by design. GlobalIds come from `Uid`s; derived objects (relationships, openings) hash the project uid with a tag.
- **cc_app** — eframe/egui app. `AppState` (state.rs) holds the `Document`, cached geometry/drawings, selection, active `Tool`, dialog flags. `app.rs` lays out the quick access toolbar + ribbon, options bar, left panel (project browser over properties), the egui_dock view tabs, and the home screen (`home.rs`) when `show_home`. Tools (`tools.rs`) run inside 2D canvases; two-letter shortcuts are matched in `app.rs::shortcuts`.

## UI conventions

- **Familiar by design**: menus, shortcuts, panels and tools go where desktop CAD/BIM users expect them (quick access toolbar top-left, ribbon tabs, properties/browser docked left, standard Ctrl shortcuts and two-letter tool keys). Use general conventions only — never a specific product's icons, colors or wording; prefer neutral terms (Category, Type, Component — not "Family").
- Colors and fonts come from `theme.rs` (`PALETTE`, Inter). Icons are painter-drawn line icons in `icons.rs`.
- Every modal uses `dialog::Dialog` (header with close button, footer buttons right-aligned: main action first, Cancel last; auxiliary buttons left; `danger` for destructive). Dialogs that edit data offer OK/Cancel and wrap edits in `dialog::session` so Cancel undoes them.
- Property grids use the shared `table.rs` primitives (collapsible `section`, two-column `row`).
- No hard-coded interface text: keys live in `assets/i18n/{en,pt-BR}.ftl`, read with `t!` / `tr`. A test scans the sources and fails if any key used in code is missing from a language, so add every new key to both files.

## Robustness rules

- **Non-test code never panics.** No `unwrap()`, `expect()`, `panic!`, `unreachable!`, `todo!`, `unimplemented!` — the workspace denies them in clippy (`[workspace.lints]`), and `clippy.toml` allows them only in tests. Return errors and propagate with `?`, or use `let … else`, `if let`, `ok_or(…)?`, `unwrap_or*` where the fallback is truly correct (never one that silently corrupts a document).
- **No `unsafe`** (`unsafe_code = "forbid"`).
- **Files are hostile input.** `.ccad` files pass through `Project::repair` on load (`persist::from_ron_with_repairs`): invalid numbers and dangling references are removed or reset and the app reports how many. Use `get()` for indices that come from files, selections or arithmetic on them; guard divisions by zero and NaN/inf.
- **Bound recursion and loops** over model references with visited sets or limits (hosting can be cyclic in a crafted file; see `EditCtx::remove_element`).
- **Last-resort guard:** opening, saving and exporting run inside `actions::guard` (`catch_unwind`); a panic becomes an error message and the open document is kept. Keep `panic = "unwind"` (the default profile).
- **Prove it:** every crash fix comes with a small synthetic regression test that failed before the fix.

## Project policies

- Clean-room: behaviour comes from public documentation, general CAD conventions, open standards and user feedback; reference tests use analytic values or open-source tools (see CONTRIBUTING.md and the README "Independence" section).
- Code, comments, identifiers and commit messages are in English; the user communicates in Portuguese.
