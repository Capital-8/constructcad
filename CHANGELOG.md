# Changelog

All notable changes to this project are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- Structural framing joins: a beam end that lies in a structural column stops at the column's face (along the face when the column is rotated), and beam ends at a common depth meet like walls — mitered at corners, butted against the side of a beam they end on, with a collinear pair running through junctions of three or more. Beams resting on a column, or at another depth, are left whole. Plans, sections, 3D, volumes and IFC export use the framed body; IFC beams carry their location line as an axis, so they come back with their own length and frame again on import.
- *Modify → Allow/Disallow Join* works on beam ends too. Beams gain *Join at Start*/*Join at End* properties and a read-only *Cut Length*; `wall_join` accepts beams, and `element_get` reports a beam's `cut_length` and what each end is framed into (`start_join`, `end_join`).
- Structural layout from grids (*Structure → Framing*): *Columns at Grids* places a column at every intersection of the selected grids (or all grids) on the active level, *Beams on Grids* places a beam between neighboring columns along each grid, and *Beams Between Columns* chains beams through the selected columns. Places and spans already modeled are skipped, and each run is one undo step. Engine commands `columns_at_grids`, `beams_on_grids` and `beams_between_columns`.
- Beam justification: the location line can follow the center or either face of the section (*Lateral Justification*) and sit at its top, center or bottom (*Vertical Justification*); joins work on the body, mirrors keep the body on its side, and `beam_create` takes `lateral` and `vertical`. The beam offset is now labeled *Elevation Offset*.
- Structural quantities: column and beam schedules gain *Section*, *Cut Length* (with a total) and *Material* fields (columns also *Structural*), shown by default in new schedules; *Structure → Structural Schedules* creates both.
- Foundations, a new category (*Structure → Foundation*): pad footings (FT, click their centers; *Footings Under Columns* centers and turns one under each column at its base), strip footings / ground beams (*Ground Beams Under Walls*, along each wall's centerline at its base, meeting at corners like beams) and foundation slabs (rafts, sketched or under the loop of walls). Types set the kind, size and depth; plans draw foundations dashed under the floor and cut where the plane passes through them; schedules list section, length, area, thickness, volume and material. IFC export writes `IfcFooting` (pad or strip) and `IfcSlab` `BASESLAB` with their type objects and base quantities. Engine commands `footing_create`, `strip_footing_create`, `foundation_slab_create`, `footings_under_columns` and `strip_footings_under_walls`.
- Sloped beams: a beam's end can sit at another elevation (*End Elevation Offset*, `beam_create` `end_offset`); the section stays square to the axis, the true length and volume follow the slope, splitting keeps the slope, and IFC export writes the body as a B-rep.
- Steel and hollow sections: column types can be I/H, channel (U), angle (L), rectangular tube or round tube, and beam types I/H, U, L or tube, edited in the type dialog (section kind, size, web, flange or wall thickness, kept within the section). 3D, plans, legends and schedules draw and measure the real profile; IFC export uses the parametric IFC profiles (`IfcIShapeProfileDef`, `IfcUShapeProfileDef`, `IfcLShapeProfileDef`, `IfcRectangleHollowProfileDef`, `IfcCircleHollowProfileDef`). The template gains steel column and beam types and a steel material.
- *Join Slabs to Supports* (*Structure → Analysis*, `slab_join_supports`): joins floors with the beams, structural columns and structural walls whose bodies overlap them, so the shared volume counts once and sections show one piece.
- Analytical model: structural columns and beams as center lines between shared nodes — beams framed into columns end on the column axis, beams ending on beams end on their line, members split where others connect, column bases are supports. *Analytical Model* opens a 3D view of it over the model drawn faint (any 3D view can switch with its *Analytical Model* property; `view_create` kind `analytical`); `analytical_model` returns it as JSON and *Export Analytical Model* (`analytical_export`) saves it for structural analysis programs.
- Reinforcement, a new category: sets of bars in beams (top and bottom bars, stirrups) and columns (longitudinal bars around the section, ties), with their cover and bar type (5 to 16 mm in the template). Bars follow their host and go with it; *Structure → Reinforcement*: *Typical Reinforcement* for the selected (or all) beams and columns, a *Reinforcement View* (3D, bars over the faint model) and a *Steel Schedule* (host, diameter, bar count, bar length, total length and weight, with totals). Beam and column properties list their sets, editable and removable. IFC export writes `IfcReinforcingBar`s (swept disks along each bar) with their type objects. Engine commands `rebar_create` and `rebar_typical`; `element_get` reports count, lengths and weight.
- Bracing and trusses (*Structure → Framing*): *X Bracing* between two selected columns (`braces_between_columns`, or `brace_create` between any two points in space) and the *Truss* tool, which lays out Pratt, Howe or Warren trusses from two clicks with the height and panels of the options bar (`truss_create`): chords and diagonals are sloped beams, posts are columns, grouped. The analytical model connects their members at the panel points.
- Structural plans (*Structure → Documentation → Structural Plan*, `view_create` kind `structural_plan`): floor plans showing only the structure — slabs, structural walls and columns, beams (seen from below) and foundations — tagged with mark and section (`V1 15×40`, `P1 20×20`). *Annotate → Structural Tags* shows the same tags in any floor plan, and plans gain *Structural Plan* and *Structural Tags* properties. *Number Structure* (`structure_number`) numbers beams, columns and pad footings in reading order.

### Changed

- `.ccad` format version 7 (foundations, steel sections, sloped beams, reinforcement, structural plans); older files still open.
- Elements cut by joined geometry (other than walls) are exported to IFC with their cut body as a B-rep, matching their exported volume.
- `*_set` commands replace an enum value given as another variant (a column section switching from rectangular to a profile) instead of merging the two.
- IFC validation tessellates curved profiles finely enough to compare their volumes, and CI validates a structural fixture.

## [0.2.1] - 2026-10-20

### Fixed

- The command palette button after the ribbon tabs did nothing: the click that opened the palette closed it again in the same frame. Ctrl+Shift+P was not affected.

### Changed

- Contributing: the DCO check lists the commits without a sign-off and explains how to add it; commits made in the GitHub web editor are signed off automatically.

## [0.2.0] - 2026-10-20

### Added

- Text notes (*Annotate → Text*, TT) in any 2D view: single-line text, or multiline text word-wrapped to a width in paper millimeters; rotation; text types with their size and default leader symbol. Leaders are drawn from the text with one segment or two (an elbow), ending in an arrow, open arrow, triangle, dot, circle, square, filled square, tick or nothing; *Add Leader* adds more to a selected text. Double-click a text to edit it; the properties edit its text, width, rotation and each leader's symbol. Engine commands `text_create`, `text_add_leader` and `text_remove_leader`; texts move, copy and rotate with the other elements and stay readable when mirrored.
- Engine (`cc_engine`): the operations of ConstructCAD as named commands that run without the GUI, each with typed JSON parameters and a generated JSON Schema — project files, undo/redo, project information, units and language; IFC import/export, sheet PDF/SVG, schedule CSV, component files; queries of categories, levels, types, elements (with derived length, height, thickness, volume, room area), views, materials and schedule tables; creation of walls, doors, windows, floors, ceilings, roofs, rooms, grids, columns, beams, stairs, railings and components with the drawing tools' defaults; move, copy/array, rotate, mirror, pin, flip, delete, wall joins, trim/extend, split, offset, align and end points; element, type, view and material parameters; types (duplicate, delete), components, materials, levels, views and sheets. Levels, types, views and materials are named by id or name; edits that would leave the model invalid are refused without an undo step.
- `ccad` command line (`cc_cli`, no GUI dependencies): list commands and schemas, run one command (JSON or `key=value` arguments) or a script, open or import a project and save it afterwards. Included in release packages.
- MCP server (`ccad mcp`): an in-house Model Context Protocol server over stdio exposing every engine command as a tool, so AI agents can build and edit models.
- Deleting a type that no element uses (`type_delete`); creating elevations and levels at a given elevation and name from commands.
- Dimensions as engine commands: `dimension_create` (associated with the elements or level lines its ends lie on) and `dimension_set_value` (drives what it measures); `elements_turn` (the Space key), `elements_export` and `elements_paste` (element sets between sessions).
- Documentation of the engine, the command line, scripts, macros and MCP (`docs/automation.md`, `docs/automacao.md`), a command reference generated from the catalog (`docs/commands.md`, `ccad reference`; a test keeps it current) and an example script (`examples/small_house.ccmacro`).
- Doors and windows can be placed by a point near a wall, without its id; sheets can be referred to by number.
- Macros (*Manage → Macros*): record the changes you make as a script of engine commands (`.ccmacro`, dragged values recorded once, cancelled dialogs left out) and play macros back; `ccad script` runs the same files.
- Join and unjoin geometry (*Modify → Geometry*, in plans and sections; engine commands `geometry_join` and `geometry_unjoin`): where two joined elements overlap, one cuts the other — structural columns and beams cut walls, floors, ceilings and roofs; floors, ceilings and roofs cut walls; walls cut architectural columns. The shared volume is counted once (quantities, schedules, IFC), the cut body loses it, and plans and sections draw two elements cut in the same material as one piece, without the line between them. Joins undo, survive saving and go with deleted elements; `element_get` lists `joined_with`. Columns gain a *Structural* property (on by default).
- *Select All Instances* (*Modify → Selection*, SA): selects every element of the selected elements' types shown in the active view; `elements_list` filters by `type` and by `view`.
- Groups (*Modify → Group*: Group GP, Ungroup UG; engine commands `group_create`, `group_ungroup`, `groups_list`): picking one member selects the whole group, so its elements move, copy and rotate together; an element belongs to one group at most, groups undo and lose deleted members, and `element_get` shows `group`.
- *Measure* (*Modify → Measure*, ME): measure between two references in any 2D view — the clicks snap to the view's linework; the status bar shows the distance, ΔX, ΔY and angle, and the distance between two parallel lines picked; nothing is created (engine query `measure`).
- Ribbon and quick access buttons are interface commands: each knows its label, shortcut, description, the engine command it runs and when it is available, so buttons enable themselves and their tooltips show the shortcut, what the command does, why it is unavailable (no selection, nothing to undo, no sheet open…) and the engine command scripts use for the same change. Two-letter shortcuts come from the same table and say why when they cannot run.
- Two-tone interface icons: strokes in the ink tone and soft fills in the theme's tint, with badges for commands that create (new views, components), import, export or remove. Theme colors are tokens with a dark set alongside the light one, ready for a dark theme.
- The ribbon collapses to its tabs (Ctrl+F1, a double click on a tab or the arrow at the end of the tabs) and the choice is remembered; while collapsed, a click on a tab shows its panel over the views until a command runs. When the window is narrower than a tab's groups, they scroll sideways with arrows at the ends.
- Command palette (Ctrl+Shift+P, or the magnifier after the ribbon tabs): finds interface commands, views to open, plans and schedules to create, and engine commands by name, description or shortcut (accents and case aside); engine commands open a form with their JSON arguments and run in the open project.
- Accessibility: the custom ribbon, palette, browser, home screen, dialog and property widgets report their role, name and state to screen readers. Interface tests drive the ribbon and the palette headlessly (egui_kittest), and screenshot tests of every ribbon tab, the narrow and collapsed ribbon, a rich tooltip, the palette and an icon gallery in both themes run on request.
- Interface guide (`docs/interface.md`, `docs/interface.pt-BR.md`): ribbon, tooltips, badges, collapsing, the command palette and its engine forms, keyboard, accessibility, and for developers how to add interface commands and icons and run the interface tests.

### Changed

- Cut hatching belongs to its element, so it highlights with it and joined geometry can trim it; the 3D boolean engine moved from `cc_ifc` to `cc_geom` (`cc_geom::csg`).
- Ribbon: *Load component* and *New component* use the component icon with an import or new badge, the sheet exports carry an export badge, and new views a new badge; a two-letter shortcut that cannot run says why in the status bar instead of doing nothing.
- `.ccad` format version 6 (joined geometry, structural columns, groups; version 5 added text notes); older files still open.
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

[Unreleased]: https://github.com/francisreboucas/constructcad/compare/v0.2.1...HEAD
[0.2.1]: https://github.com/francisreboucas/constructcad/compare/v0.2.0...v0.2.1
[0.2.0]: https://github.com/francisreboucas/constructcad/compare/v0.1.0...v0.2.0
[0.1.0]: https://github.com/francisreboucas/constructcad/releases/tag/v0.1.0
