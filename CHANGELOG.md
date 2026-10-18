# Changelog

All notable changes to this project are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- Modify tools: Move (MV), Copy (CO, multiple), Rotate (RO, with center and typed angle), Mirror by picking an axis (MM) or drawing it (DM), linear Array (AR), Trim/Extend to Corner (TR) or to a reference (TX), Split (SL), Offset (OF), Align (AL), allow/disallow joins at wall ends, and Pin/Unpin (PN/UP); previews, snapping and one undo step per edit. Walls take their doors and windows along; mirrored openings swap their hinge side.
- Clipboard (*Modify → Clipboard*): Cut (Ctrl+X), Copy (Ctrl+C) and Paste (Ctrl+V) of elements within the project, placing the copy with a click in a plan or aligned to the same place or to the current level; walls bring their doors and windows, and a cut can be pasted after its originals are gone. Match Type Properties (MA) picks an element and gives its type to others of the same category.
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
