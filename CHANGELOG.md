# Changelog

All notable changes to this project are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- Engine (`cc_engine`): the operations of ConstructCAD as named commands that run without the GUI, each with typed JSON parameters and a generated JSON Schema — project files, undo/redo, project information, units and language; IFC import/export, sheet PDF/SVG, schedule CSV, component files; queries of categories, levels, types, elements (with derived length, height, thickness, volume, room area), views, materials and schedule tables; creation of walls, doors, windows, floors, ceilings, roofs, rooms, grids, columns, beams, stairs, railings and components with the drawing tools' defaults; move, copy/array, rotate, mirror, pin, flip, delete, wall joins, trim/extend, split, offset, align and end points; element, type, view and material parameters; types (duplicate, delete), components, materials, levels, views and sheets. Levels, types, views and materials are named by id or name; edits that would leave the model invalid are refused without an undo step.
- `ccad` command line (`cc_cli`, no GUI dependencies): list commands and schemas, run one command (JSON or `key=value` arguments) or a script, open or import a project and save it afterwards. Included in release packages.
- MCP server (`ccad mcp`): an in-house Model Context Protocol server over stdio exposing every engine command as a tool, so AI agents can build and edit models.
- Deleting a type that no element uses (`type_delete`); creating elevations and levels at a given elevation and name from commands.
- Dimensions as engine commands: `dimension_create` (associated with the elements or level lines its ends lie on) and `dimension_set_value` (drives what it measures); `elements_turn` (the Space key), `elements_export` and `elements_paste` (element sets between sessions).
- Documentation of the engine, the command line, scripts, macros and MCP (`docs/automation.md`, `docs/automacao.md`), a command reference generated from the catalog (`docs/commands.md`, `ccad reference`; a test keeps it current) and an example script (`examples/small_house.ccmacro`).
- Doors and windows can be placed by a point near a wall, without its id; sheets can be referred to by number.
- Macros (*Manage → Macros*): record the changes you make as a script of engine commands (`.ccmacro`, dragged values recorded once, cancelled dialogs left out) and play macros back; `ccad script` runs the same files.

### Changed

- The desktop app runs on the engine: its drawing tools, modify tools, properties, dialogs, clipboard, dimensions, document actions and undo/redo change the model through the same commands as the command line and MCP. Edits that would leave the model invalid are refused with a message; dragged values remain one undo step.
- Deleting a level also unconnects columns and stairs whose top was attached to it.

- IFC import without dropping classes: every physical object of an IFC2X3/IFC4 file comes in. Products become native elements whenever a native element holds their geometry exactly; everything else (stairs, railings, members, plates, curtain walls, MEP segments and fittings, terrain, curved walls, classes from other schemas) becomes a generic component that keeps its IFC class, predefined type, object type and exact geometry, and is exported back as the same class (with its IFC4 type object, or as a named `IfcBuildingElementProxy` for classes IFC4 lacks). `IfcGrid` axes become grid lines.
- In-house IFC geometry engine: placements (local and grid), polylines, indexed curves with arcs, circles, ellipses, trimmed and composite curves, B-splines (rational too); every parametric profile, arbitrary profiles with voids, center-line, composite, derived and mirrored profiles; extrusions, revolutions, swept disks and directrix sweeps; CSG primitives; boolean union, difference and intersection; half-space clipping; faceted and advanced B-reps; face sets and mapped items. Plane angle, area and volume units are read.
- Openings (`IfcRelVoidsElement`) are cut from imported generic objects with in-house mesh booleans; slabs, coverings, roofs, columns and beams with openings come in as generic objects so their holes are kept.
- Property and quantity sets of imported generic objects (their own and their type's) are kept with their IFC value types, converted to SI units, listed in Properties and written back on IFC export.
- Components can hold fixed mesh pieces; plans draw them cut at the cut plane with the edges seen from above.
- Properties show the IFC class, predefined type and object type of imported objects, and the import message counts the objects brought in as generic objects.
- `export_ifc` example accepts an `.ifc` file (import and export again); CI validates a hand-written IFC4 fixture covering many classes with IfcOpenShell.

## [0.1.0] - 2026-10-19

### Added

- Modify tools: Move (MV), Copy (CO, multiple), Rotate (RO, with center and typed angle), Mirror by picking an axis (MM) or drawing it (DM), linear Array (AR), Trim/Extend to Corner (TR) or to a reference (TX), Split (SL), Offset (OF), Align (AL), allow/disallow joins at wall ends, and Pin/Unpin (PN/UP); previews, snapping and one undo step per edit. Walls take their doors and windows along; mirrored openings swap their hinge side.
- Clipboard (*Modify → Clipboard*): Cut (Ctrl+X), Copy (Ctrl+C) and Paste (Ctrl+V) of elements within the project, placing the copy with a click in a plan or aligned to the same place or to the current level; walls bring their doors and windows, and a cut can be pasted after its originals are gone. Match Type Properties (MA) picks an element and gives its type to others of the same category.
- Typed length and angle while drawing walls and moving, copying or arraying: digits go to the length, Tab switches to the angle (from the previous wall or the X axis, on the cursor's side), Enter applies; lengths are read in the project's length unit.
- Temporary dimensions on selection: a single selected wall, beam or grid shows its distance to the nearest parallel wall, grid or column on each side, columns and components along X and Y, doors and windows along their wall; click a value and type to move the element.
- Associative dimensions: in plans each end of a new dimension follows the element it was placed on (walls, grids, columns, beams, doors, windows, components), through moves, rotations, stretches and undo; angular dimensions also keep their vertex at the intersection of the measured lines; in elevations and sections ends follow elements (with their height from the element's base or wall top) and level lines, and editing a dimension between levels raises or lowers the upper level; the dimension line stays where it was placed. Copies follow the copied elements; deleting an element frees the end. The sample project's dimensions follow its grids.
- Dimension values are editable: click the text of a selected dimension, double-click a dimension or type in Properties; in plans the new value moves what the dimension touches at its moving end (crossing elements translate, walls/beams/grids along it stretch), in one undo step. Angular values rotate; radial and diameter values are read-only.
- Dimension kinds: linear (horizontal or vertical by where the line is placed), angular (between two picked lines), radial and diameter (picked on any arc or circle in the view), besides aligned; one shared layout draws them in views, sheets and tool previews. Project files move to format version 4 (older files load with aligned dimensions).
- Selecting objects with the Select tool (in any view) opens the Modify tab.
- Zoom menu on the view, sheet and 3D zoom button: zoom in region (drag a window), zoom out (2x), zoom to fit, zoom all to fit, zoom sheet size (paper millimeters at screen size) and previous/next pan/zoom; the last command chosen becomes the button's action. In 3D views the window zoom frames the dragged region and the history keeps camera positions.
- Interface icons redrawn as SVG line drawings (`assets/icons/ui`), rasterized with resvg at the exact pixel size and aligned to the pixel grid, so they stay crisp at any scale.
- Ribbon: long labels wrap onto two lines, and panels with many tools stack small icon buttons with tooltips.
- Material browser (*Manage → Materials*): searchable list with swatches and four tabs — identity (description, class, keywords, manufacturer, model, cost, URL, keynote, mark), graphics (shading color, transparency, surface and cut patterns), appearance (render color, finish) and physical properties (thermal and mechanical); new, duplicate, rename (updating every type) and delete (refused while in use).
- Material cut patterns in plans (per wall layer, columns) and sections, at a constant paper size; transparent materials blended in 3D, elevations and sections, and exported to IFC surface styles.
- Project units (*Manage → Project Units*): unit, decimals and symbol for lengths, areas, volumes, angles and slopes, a currency symbol and the decimal symbol / digit grouping, applied to dimensions, tags, schedules, properties and length fields.
- Navigation cube in 3D views (faces, edges and corners, compass ring, home view), temporary dimensions with angles while drawing, and alignment guides from acquired points.
- Space rotates selected components and columns by 90° and flips selected doors and windows; the quick access toolbar gains Export PDF, Aligned Dimension, Default 3D View and Section.
- Parametric components: definitions made of boxes, cylinders and extruded profiles sized by expressions over named type or instance parameters; a component editor with live expression checking and plan/front previews; a starter library (table, chair, bed, sofa, toilet, washbasin, wardrobe); the *Component* tool (CM) with rotation; instance parameter overrides in Properties; components in 3D, plans, elevations, sections, legends and schedules; `.ccomp` files to save and load components; IFC export and import as `IfcFurniture`, `IfcSanitaryTerminal`, `IfcLightFixture` and `IfcBuildingElementProxy`, keeping parametric types and instance values on a round trip.
- IFC import (*File → Import IFC…*): an in-house STEP parser and a mapping of IFC2X3/IFC4 storeys, walls, doors, windows, slabs, ceilings, roofs, spaces, columns and beams (extruded solids, B-reps and face sets; millimeter and imperial units) to native elements and types; export → import round trips are tested.
- Sheet export to vector PDF and SVG (*File → Export PDF/SVG…*); the sheet is composed once in `cc_views`, so the screen and the files match.
- Configurable schedules: choose and order fields (instance, type and computed values), sort ascending or descending, filter by text, show or hide totals, and export to CSV (semicolon-separated when the decimal comma is used).
- Gable and hip roofs computed as the lowest of the planes rising from each sloped eave, with ridge and hip lines in plan and faceted B-rep IFC export.
- Columns (rectangular/round sections, rotation, top constraint), beams, straight stairs and railings, with types, plan symbols (cut columns, dashed beams, stair treads with break line and arrow), legends, schedules and IFC export; new *Structure* tab and CL/BM/ST/RA shortcuts.
- Rooms: boundary detected from room-bounding walls around a point (openings do not break it), with number, name, height, area, perimeter and volume; room tags in plans, room schedule and `IfcSpace` export with base quantities.
- Grids: named grid lines with bubbles shown in every plan; elevation markers stay outside them.
- Aligned dimensions in any 2D view, on a new *Annotate* tab, snapping to the view's linework.
- Automatic door and window tags (marks) in floor plans.
- Room, grid and dimension types; projects created earlier gain default types when opened.
- Wall constraints: top constrained to a level with offset (follows the level), location line (centerline, core or finish faces), room bounding and structural parameters; the wall tool sets top constraint and location line in the options bar.
- Type parameters: wall function, layer functions (core and finishes), type mark, description, manufacturer, model, URL, keynote, classification code, fire rating, comments and cost.
- New Type Properties dialog with type switching, Duplicate, Rename, sectioned parameters and a layer editor.
- Persistent unique ids for the project, types and elements, used as IFC GlobalIds; walls export `Pset_WallCommon` and types `Pset_ManufacturerTypeInformation`.
- Close Project (*File* tab, Ctrl+W) returns to the home screen.
- Home screen with recent projects (with 3D thumbnails) and bundled samples, shown at startup when no project is given and from the Home button.
- Elevation markers in floor plans.
- IFC4 export (`cc_ifc` crate and *File → Export IFC…*), validated in CI with IfcOpenShell.
- Initial MVP: parametric walls, doors, windows, floors, ceilings and roofs; plans, ceiling plans, elevations, sections, 3D views, legends, schedules and sheets; English and Portuguese (Brazil) interface; `.ccad` project files.

### Changed

- `.ccad` format version 3 (components); older files still open and receive the component library.
- The sample house is furnished with library components.
- Robustness rules: clippy denies `unwrap`, `expect` and `panic!` outside tests and the workspace forbids `unsafe`; opening, saving and exporting are guarded so an internal error never loses the open project.
- Projects are repaired when opened: invalid numbers, missing levels or types, orphan doors and windows, broken view references are removed or reset, and the number of repairs is reported.
- Standard dialog layout for every window (header with close button, footer with right-aligned actions, highlighted main and red destructive buttons, alert icons); data dialogs have OK / Cancel and Cancel undoes their edits; deleting a level asks for confirmation.
- `.ccad` format version 2 (adds the fields above; version 1 files still open).
- Redesigned About dialog with version, author, license, repository and credits.
- Polished interface: light neutral theme with the Inter typeface, ribbon with icons grouped into captioned panels and a quick access toolbar, project browser with category icons and counts as an indented tree, property palette with headers, floating scale and zoom-to-fit control in 2D views and a clearer 3D orientation gizmo.

### Fixed

- A crafted file with cyclic hosting (e.g. a door hosting itself) crashed the application when the element was deleted.
- Deleting a level no longer leaves walls attached to it with a near-zero height; they become unconnected at their current height.
- Stray lines on roofs and other large faces in 3D sheet views, elevations and sections: faces are now ordered by comparing depths where they overlap.
- Closing the last open view tab crashed the application.
- Per-wall volumes in schedules were wrong for walls joined at corners; they are now computed exactly.

[Unreleased]: https://github.com/francisreboucas/constructcad/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/francisreboucas/constructcad/releases/tag/v0.1.0
