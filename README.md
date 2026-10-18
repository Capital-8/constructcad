# ConstructCAD

Free and open source **Building Information Modeling (BIM)** software, written in Rust and independent of any commercial vendor.

[Português (Brasil)](README.pt-BR.md) · [Contributing](CONTRIBUTING.md) · [Code of Conduct](CODE_OF_CONDUCT.md) · [Security](SECURITY.md)

A single parametric model is the source of truth. Floor plans, ceiling plans, sections, elevations, 3D views, legends, schedules and sheets are *queries* over that model and update automatically after every change.

## Familiar by design

The menus, shortcuts, panels and tools your hands already know. Productive on day one, no retraining.

ConstructCAD follows the conventions shared by desktop CAD and BIM tools: the quick access toolbar at the top left, a ribbon of tabs, the properties palette and project browser docked at the side, and standard and two-letter keyboard shortcuts. These are general conventions; ConstructCAD does not copy any product's icons, artwork or wording (see [Independence](#independence)).

## Running

Requires a recent stable Rust toolchain (edition 2024).

```sh
cargo run --release -p cc_app                                    # new project
cargo run --release -p cc_app -- examples/casa_simples.ccad      # open a project
cargo run --release -p cc_app -- examples/casa_simples.ccad --view "Elevação: Sul"
cargo run --release -p cc_app -- --lang en                       # English interface
cargo run -p cc_core --example casa_simples                      # regenerate the example
```

The bundled example project was authored in Portuguese, so its view names are in Portuguese.

## Languages

The interface is available in **English** and **Portuguese (Brazil)**, switchable under *File → Language*. The choice is saved between sessions. On first run the system language is used, and `--lang pt-BR|en` forces a language.

New projects get level, material, type and view names in the active language. Existing projects keep their names, since they are part of the data. The decimal separator follows the language (comma or point).

Translations live in `assets/i18n/*.ftl`. A test ensures every language has the same keys and placeholders. To add a language, create the `.ftl` file and register it in `Language` (`crates/cc_app/src/i18n.rs`).

## Features (MVP)

- **Project browser**: Floor Plans, Ceiling Plans, 3D Views, Elevations (N/S/E/W), Sections, Legends, Schedules and Sheets. The context menu opens, renames, duplicates, deletes and places views on the active sheet.
- **Plan modeling**:
  - Chained walls with automatic L (mitered) and T joins.
  - Wall-hosted doors and windows, with openings cut automatically.
  - Floors, ceilings and roofs (flat or single-slope, with overhang) by sketch or "From walls".
- **Snapping**: endpoints, midpoints, on wall, 15° angles, Shift for orthogonal, typed length + Enter.
- **Editing**: click or window selection (inside or crossing), drag to move, Ctrl+drag to copy, delete. Undo and redo cover every edit.
- **Properties**: instance and view parameters, type switching and a type editor (layers, materials, dimensions, duplicate).
- **3D view** (wgpu, MSAA): orbit, pan, zoom, zoom to fit and click selection.
- **Elevations and sections**: hidden line removal, cut hatching and annotated levels.
- **Schedules**:
  - Doors and windows.
  - Walls, with length, area and volume.
  - Floors, ceilings and roofs, with area, perimeter and volume.
- **Sheets** A0–A4: title block with project data and draggable, scaled viewports.
- **Manage**: project information, levels (each new level creates its plans) and materials.
- **`.ccad` files**: versioned, human-readable RON; saving is atomic.
- **IFC4 export** (*File → Export IFC…*): storeys, walls with openings, doors, windows, floors, ceilings and roofs as swept solids, with types, material layers, colors and base quantities. Written by ConstructCAD's own STEP writer, with no third-party IFC library.

## Shortcuts

| Key | Action | Key | Action |
|---|---|---|---|
| WA | Wall | SB | Floor |
| DR | Door | FO | Ceiling |
| WN | Window | TE | Roof |
| CT | Section | MD | Select |
| ZE | Zoom to fit | Esc | Cancel / exit tool |
| Ctrl+Z / Ctrl+Y | Undo / Redo | Delete | Delete |
| Ctrl+S / Ctrl+Shift+S | Save / Save as | Space | Flip door |

2D navigation: the mouse wheel zooms and the middle button pans. 3D navigation: left drag orbits, right or middle drag pans and the wheel zooms.

## Architecture

```
crates/
  cc_core    BIM model (elements, types, levels, views), undo/redo transactions, .ccad persistence
  cc_geom    derived geometry: walls with joins and openings, slabs, roofs, cuts, picking
  cc_views   2D drawing generation (plan, ceiling, elevation, section, axonometric, legend) and schedules
  cc_render  offscreen wgpu 3D renderer
  cc_ifc     IFC4 export: STEP physical file writer and model mapping
  cc_app     eframe/egui application: ribbon, browser, properties, tools, sheets
assets/i18n  translations (pt-BR, en); interface text is never hard-coded
```

Every change goes through `Document::edit`, which records a reversible `Transaction`. Continuous edits (dragging values) are merged into a single undo step.

## Development

```sh
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all -- --check
```

IFC exports are validated in CI with [IfcOpenShell](https://ifcopenshell.org/), an independent open-source reference used only for testing:

```sh
pip install -r scripts/requirements-ifc.txt
cargo run -p cc_ifc --example export_ifc -- examples/casa_simples.ccad casa_simples.ifc
python scripts/validate_ifc.py casa_simples.ifc
```

See [CONTRIBUTING.md](CONTRIBUTING.md) for the workflow and project rules.

## Roadmap

- IFC import.
- PDF/SVG sheet export.
- Dimensions and annotations.
- User parametric families.
- Stairs and railings.
- Grids.
- Gable and hip roofs.
- Walls attached to the level above.
- Material textures.

## Independence

ConstructCAD is independent. Behaviour is designed from public documentation, general CAD conventions and user feedback. Reference tests use analytic values and open-source tools. ConstructCAD is not affiliated with, sponsored by or endorsed by Autodesk. Autodesk and Revit are trademarks of Autodesk, Inc.

## License

Copyright © Francis Rebouças <francisreboucas@gmail.com>.

ConstructCAD is licensed under the [GNU General Public License v3.0 or later](LICENSE) (GPL-3.0-or-later).

The application icon is by [Magnific – Flaticon](https://www.flaticon.com/) (see `assets/icons/FLATICON.txt`). The bundled [Inter](https://rsms.me/inter/) typeface is © The Inter Project Authors and licensed under the [SIL Open Font License 1.1](assets/fonts/OFL-Inter.txt).
