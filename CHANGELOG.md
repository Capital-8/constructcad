# Changelog

All notable changes to this project are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- Stair shapes (#29): straight, straight with a landing, L-shaped (with a landing or 2 to 6 winders), U-shaped (with a gap between the flights) and spiral/helical (around an inner radius), turning left or right, with the risers of the first flight and the landing depth adjustable. The stair tool's options bar picks the shape; the properties palette edits it; `stair_create` takes `shape`, `turn`, `split`, `landing`, `gap`, `winders` and `inner_radius`. Plans draw every tread, landing and winder (dashed above the cut) with the break line and the arrow along the walking line; IFC export writes the matching `IfcStair` type (`TWO_STRAIGHT_RUN_STAIR`, `QUARTER_TURN_STAIR`, `QUARTER_WINDING_STAIR`, `HALF_TURN_STAIR`, `SPIRAL_STAIR`) with one closed B-rep per piece.
- Stair construction on the type: solid (filled down to the floor, as before), cast in place (steps on an inclined waist slab of a given thickness, measured square to the pitch, and landing slabs; `WaistThickness` in IFC) or assembled (precast, steel or timber treads of a given thickness on side stringers or a central one, a central column on spirals, optional closed risers).
- Step rules on the type and Blondel's formula: minimum riser and tread and the range of `2 × riser + tread` (defaults 16 cm, 28 cm and 63–65 cm); the tread can be fixed or follow Blondel's formula from the riser. The properties palette shows risers, riser height, tread, run and Blondel's step with the broken rules, and the new `stair_check` command returns them; `element_get` lists them under `derived.check`.
- Railings that follow a stair (#29): *Add railings* in a stair's properties, or `stair_railings_add`, puts railings along its sides that climb at the pitch of the nosings, go round landings and winders and follow the stair when it changes, is copied, mirrored or deleted. Railing lengths are measured along their slope.
- Ramps (#29), a new category (*Ramp* in the Circulation group, `RP`): straight (flights in line with landings between them), L-shaped (around a corner landing), U-shaped (switchback flights joined by landings) or helical, turning left or right. The type sets the width, the slab thickness and the accessibility rules (steepest slope, highest rise per flight, shortest landing; by default 8.33 %, 0.80 m and 1.20 m): flights are laid out to meet them, and the properties palette and the new `ramp_check` command report the slope, the flights and the rules broken (too steep, a flight rising too high, a short landing). The instance may take a gentler or steeper slope and a longer landing. `ramp_create` and `ramp_type_create` are new; a project without a ramp type gets one with its first ramp. Plans draw the flights (dashed above the cut, with a break line), the arrow and the slope; schedules list width, slope, run and volume; IFC export writes `IfcRamp` (`STRAIGHT_RUN_RAMP`, `TWO_STRAIGHT_RUN_RAMP`, `QUARTER_TURN_RAMP`, `HALF_TURN_RAMP`, `SPIRAL_RAMP`). `stair_railings_add` (*Add railings*) also puts railings along ramps.
- Autosave and crash recovery (#51). A minute after a project gets unsaved changes (new, never-saved projects too), a copy of it is written in the background to the `recovery` folder of the app's data folder, only when the model changed since the last copy, through a temporary file and a rename. Saving, closing or quitting cleanly removes it. When ConstructCAD starts and finds copies left by a session that crashed or was killed, the *Recover Unsaved Work* dialog offers them: *Restore* opens the project with its unsaved changes (saving writes to its original file), *Discard* deletes the copy, *Cancel* keeps it for the next start.
- A panic in the interface writes a last recovery copy before the app closes, and every panic is appended to `crash.log` in the app's data folder (message, place and backtrace), since release builds on Windows have no console.
- Grids are named in series (#52). A new grid takes the name after the most recently placed grid parallel to it (within 10°): `1` → `2`, `09` → `10`, `A` → `B`, `Z` → `AA`, `A1` → `A2`, `1.1` → `1.2`. A grid across the existing ones starts a new series: letters across numbered grids, numbers otherwise. The name is unique among the grids (a taken one is skipped). The grid tool shows the name next to the cursor before the second click. `grid_create` uses these names when `name` is left out.
- Grid ends line up with the bubbles of the other grids (#52). When the grid tool places a point or an end handle of a selected grid is dragged, the end snaps (within the usual 10-pixel snap distance) to the line through the matching end of a parallel grid, perpendicular to it. The second point also keeps the grid parallel to that grid. A dashed guide from that grid's end shows the alignment. Model points and guide crossings still take priority.
- *Align Grid Ends* (*Architecture* and *Structure* tabs, *Datum* group; `grids_align_ends`): with one grid selected, every grid parallel to it (within 1°) takes its extents. Each end moves along its own line to the reference end on its side. Pinned grids are left alone. `ends` limits the move to the `start` (bubble) or `end` ends, and `grids` picks the grids to align.
- Modify tools in 3D views (#53): Move, Copy, Array, Rotate, Mirror (pick or draw the axis), Align and Join/Unjoin Geometry work in 3D views. Points are picked on a horizontal working plane at the elements' level (then at the height of the first point) or snap to model points near the pointer: element corners, column and component centers at their base and top, wall ends and grid crossings. The preview shows the elements' outline where they will go. Changes are plan transforms, so elements keep their levels. Trim/Extend, Split, Offset and Allow/Disallow Join need plan linework and say so in 3D views.
- Align takes columns and components, by the edge of their outline nearest to the point picked, in plans and in 3D. Alignment is in plan, so a column of an upper floor aligns onto the face of the column below. `element_align` takes `pick` and `reference_pick` for them (without `pick`, a column or component moves its insertion point onto the reference line, such as a grid).

### Changed

- Solid and cast-in-place stairs build each straight flight as one block from its side profile: the steps on top and, when cast in place, a continuous waist slab with a flat foot on the floor and a haunch under the landing it rises to, down to the landing's underside; the flight above starts from that underside. Sides show the steps and the soffit only, with no seams between treads.
- Spiral stairs turn no faster than leaves 2.20 m between a tread and the one a turn below (the tread on the walking line grows when the type's would), and a solid spiral is built as a helical slab instead of treads filled down to the floor; its curved sides and soffit read as smooth surfaces.

### Fixed

- The 3D view and picking in it could crash on a degenerate face (a footprint with no area, or points that are not finite numbers): its triangulation panicked inside earcutr. Such faces now triangulate in the plane of their points or give no triangles.
- A GPU error (a validation error or a failed allocation) no longer ends the app: it is logged. A 3D mesh larger than the GPU's buffer limit is not uploaded instead of failing validation.

## [0.6.1] - 2026-10-28

### Added

- Lao interface (ພາສາລາວ, `lo`), contributed by @bounkirdni-2025. It is picked from the system language for `lo`.
- Lao text shows in the interface and in PNG export through a system font that has Lao letters (Leelawadee UI on Windows 10 and later, Lao Sangam MN on macOS, Noto Sans Lao or Phetsarath on Linux), after the CJK ones. No font is bundled.

### Changed

- Faster 3D views. Ending an orbit, pan or zoom no longer rebuilds the model's geometry and every view's drawing (saving the camera redraws only that view and the sheets showing it); selecting, hiding or isolating elements repaints the 3D mesh instead of rebuilding it; an unchanged 3D view is not rendered again on every repaint; clicking to select tests the nearest elements first (about 10 times faster). Outline edges of dense meshes, such as imported IFC elements, are found in near-linear time (an element of 8,000 triangles: 184 ms → 13 ms).

### Fixed

- Component editor: text fields shrank to a few letters, the dialog stretched to the window's width, the piece buttons (move down, remove) and the connectors were cut off, and the dialog could run off short windows. The fields have fixed widths, the previews sit in a column beside the parameters and pieces, the connectors span the width below, and the body scrolls when the window is short.
- A 3D view of only transparent solids (with no edges) showed nothing.

## [0.6.0] - 2026-10-27

### Added

- Doors and windows are components that open walls. A component of kind *Door* or *Window* with a wall opening (width, height and default sill as expressions of its parameters) makes door or window types: they cut that opening in their wall and their pieces, resolved with `WallThickness` (the wall's thickness) and centered on the opening, mirror with the door's flips. Plans draw the category's swing or sash lines, or the component's own symbol (*Own plan symbol*). Tags, schedules, legends, dimensions, phases and IFC (`IfcDoor`/`IfcWindow` with the pieces as body) work as before. The template's door and window types, and those imported from IFC, are built-in components; the component editor edits the opening; component files carry door and window components. `component_create` takes `kind: "Door"`/`"Window"` and `opening`.
- Plan-only component pieces: 2D symbols such as a door's swing (a plan profile with an arc), drawn in plans and legends with no solid in 3D, in the component's cuts or in IFC. The component editor marks them (*Plan only*).
- Component connectors: a component lists where pipes and conduits connect, each with a name, a system kind (cold or hot water, sanitary, vent, rainwater, power, lighting), a position and direction (expressions) and a diameter. The networks attach each connector to a run of its kind, and `mep_connect` starts each branch at its connector, of its diameter, leaving vertically or across as it points. The template toilet has a sewer outlet and a cold water inlet, the washbasin waste, cold and hot water. The component editor lists the connectors and marks them in its previews.
- Component library window (*Architecture → Component Library…*): browse the component files (`.ccomp`) of the folders you add (kept with the preferences), such as a copy of [ConstructCAD Components](https://github.com/francisreboucas/constructcad-components); search by name, type or description, filter by kind, see a component's types, parameters, connectors and plan, load it into the project (doors and windows start the wall tool) and save a project component into a library folder. The engine command `component_library_list` lists the folders for the command line and MCP.
- Arc walls trim, extend and align: *Trim/Extend to Corner* and *to a reference* work along an arc wall's circle and to it (lines and circles meet where they cross; of two crossings, the one on the elements, else the one that changes them least), and an arc wall aligns onto a concentric one (its radius changes, its openings keep their angles). An arc wall picked as a mirror axis mirrors across its axis of symmetry.

### Changed

- `.ccad` format version 12: door and window types are components. Older files open with their door and window types turned into the built-in door and window components of the same sizes. Files saved by 0.6.0 do not open in 0.5.0.

## [0.5.0] - 2026-10-27

### Added

- Arc walls: a selected arc wall shows a round handle at the middle of its arc; drag it to change the curvature (the arc passes through the cursor, its ends stay). While drawing an arc wall, after the start and the end, type a radius and press Enter: the arc takes the cursor's side, the shorter or the longer one, whichever middle is nearer the cursor. `wall_create` takes `radius` (signed by the turn) and `major_arc`.
- Traditional Chinese interface (繁體中文, `zh-TW`), contributed by @yoyomaimai. It is picked from the system language for `zh-TW`, `zh-HK`, `zh-MO` and `zh-Hant`.
- Chinese, Japanese and Korean text shows in the interface and in PNG export through the system's CJK fonts (Microsoft JhengHei, PingFang, Noto Sans CJK…). They used to show as boxes. No font is bundled. PDF export still writes such text as `?`.
- Doors and windows can be placed in a 3D view: with the Door or Window tool active, click a wall. The opening is centered where you clicked, at the type's sill, facing that side.
- Roofs: a slope per outline edge for gable and hip roofs (`Roof::edge_slopes`): an edge follows the roof's slope, becomes a gable end, or takes its own slope (*Edge Slopes* in the roof properties, `roof_edge_set`). Roof openings are listed in the properties and removed one at a time (`roof_opening_remove`).
- *Demolish* acts on the selection at once, takes a window (left to right: inside it; right to left: touching it) and works in 3D views.
- Any view as a PNG image: *File → Export Image…* and `view_export` (PNG, SVG or PDF of plans, elevations, sections, 3D views, legends, diagrams and sheets, at a chosen resolution); text is drawn with the embedded Inter font.
- Light, dark or system theme in *File → Preferences*; 2D drawings keep a white paper in both.
- Floor openings (shafts, stairwells): `floor_create` takes `openings`, `floor_opening_add` and `floor_opening_remove` edit them, and the floor properties list them; plans draw them, areas and volumes leave them out, and IFC export writes them as voids of the slab's profile.
- IFC import understands `IfcSlab` better: slabs with openings that pass through them (profile voids, opening elements, or shafts cut out of their body) become floors with openings, and base slabs shaped like slabs become foundation slabs; piles and pile caps written as base slabs stay generic objects keeping their class.

### Fixed

- *Duplicate* in the Type Properties dialog kept the default "(copy)" name unless Enter was pressed: clicking *OK* or another field dropped the typed name. The name is now kept either way.
- Snapping only knew wall axes, so clicks at the corner of a wall, as drawn, missed it (sketching a ceiling inside a room, moving an element). Wall faces and corners, column and stair outlines, sketch edges, railings, and grid lines and their crossings are now snap points in every tool.
- A door or window copied without its wall and pasted on another level stayed on the wall it was copied from. It now goes into the wall where it lands on the target level, and is skipped when there is no wall there.
- Elevations and sections of a project with nothing modelled yet showed no level lines. They now span the grids, or a few meters from the origin.
- The *Filters* and *View Templates* dialogs grew a little every frame until their header, list and buttons left the window, leaving a blank panel: the divider between the list and the details took the dialog's whole height. It now ends with the taller column.
- Floors, roofs and ceilings created from walls follow the outer boundary of the level's walls: interior walls meeting the outer corners no longer lead them around an inner loop (an L-shaped house got its old rectangle), and walls hanging off the outline (garden walls) are left out. With separate buildings on a level, the largest is taken.
- Zoom to fit in plans frames the model and its annotation, leaving out the elevation markers around it and the far clip lines of sections, so the building no longer looks small.
- View filters: a rule on a field that none of the filter's categories has (`width` on walls) matched nothing without a word. `filter_create` and `filter_set` now refuse it, naming the categories the field applies to, and the filter editor only offers the fields of the checked categories. Door schedules can show the sill height.
- *Download* in the update notice and the repository link in *About* did nothing: the app was built without opening links in the browser.

## [0.4.2] - 2026-10-25

### Added

- Linux on arm64 (aarch64): releases include a `.deb` (`arm64`), an AppImage and a `.tar.gz` for 64-bit Arm, built natively on Ubuntu 22.04.

## [0.4.1] - 2026-10-24

### Changed

- A 2D view, sheet or 3D view left zoomed to fit fits again when its pane changes size (a maximized window restored, a split moved), until it is panned or zoomed by hand. Views named with `--view` replace the default layout: the first on the left, the second on the right.

### Fixed

- Linux: the AppImage starts on systems without the X11 keyboard and cursor libraries (it bundles libxkbcommon, libxkbcommon-x11, libXcursor, libXi and the libraries they need), and the Linux builds run on older distributions (built on Ubuntu 22.04, glibc 2.35, instead of 24.04).

## [0.4.0] - 2026-10-24

### Added

- Arc walls: a wall's location line can be a circular arc. The wall tool gains *Line*/*Arc* modes in the options bar; in *Arc* mode click the start, the end and then a point on the arc (with a live preview, radius and length), and the chain goes on from the arc's end. Layers, faces and cut patterns follow concentric arcs; arc walls join straight and arc walls at corners, tees and junctions (faces meet where their lines and circles cross; walls continuing along a common tangent butt square), host doors and windows along the arc (offsets measured along the centerline, frames square to the wall at the opening), bound rooms and give floors and roofs drawn from walls their curve. Length, side area and volume are exact (along the centerline arc). Plans draw the arcs (radial and diameter dimensions pick them), and elevations, sections and 3D draw curved faces without seams between facets. Splitting keeps each part on the arc, offsetting makes a concentric copy, end handles and move, rotate, mirror and copy keep the arc; trimming, extending and aligning need straight walls. The properties show the arc angle (editable) and radius.
- `wall_create` takes `mid` (a point on the arc) or `bulge` (tan of a quarter of the sweep); `element_get` shows `bulge` among a wall's `params` (editable with `element_set`) and the arc's `center`, `radius` and `sweep` among the derived values. Arcs too tight for the wall's thickness, and three points in line, are refused.
- Roofs over any footprint: gable and hip roofs are computed as a weighted straight skeleton of the outline (sloped eaves rise at the roof's slope, gable edges stay vertical), so L, T, U and other concave outlines get proper hips, valleys and ridges, and gables can stand above a lower roof beside them. Plans draw the ridges, hips and valleys; 3D, elevations, sections, volumes and IFC export (faceted B-rep) use the computed planes.
- Roof openings (courtyards): a roof can have openings inside its outline, around which pitched roofs slope down to their own eaves (the overhang reaches over them). The roof properties gain *Openings* with *Add Opening* (sketch it in the plan) and *Remove Openings*; `roof_create` takes `holes`, and `element_set` edits them (openings must lie inside the outline, apart). Flat and shed roofs are cut by them too; areas, volumes, schedules and IFC export (`Qto_RoofBaseQuantities`) leave them out. `element_get` reports a roof's `plan_area` and its `ridges`.
- IFC import brings this program's pitched, flat and shed roofs back with their outline, openings and exact slope (from the B-rep's faces). CI validates a roof fixture.
- IFC export writes an arc wall's axis as an `IfcTrimmedCurve` on an `IfcCircle` and its body as an extruded profile of arcs and lines (`IfcIndexedPolyCurve`), with its openings as sectors through the wall; import brings such walls back as arc walls with their doors and windows. CI validates an arc wall fixture.
- Update notice: at startup, at most once a day and in the background, the app asks the GitHub API for the latest release and, when it is newer than the running version (pre-releases ignored), shows a notice in the status bar and on the home screen with *Download* (opens the release page in the browser), *Skip This Version* and dismiss. Nothing is downloaded or installed. *File → Check for Updates* (also in *About*) checks on request and reports the result. Automatic checks can be turned off in *About*, and `CONSTRUCTCAD_NO_UPDATE_CHECK=1` turns every check off; failures are silent. See *Update notice* in the interface guide for what is contacted.
- View graphics in the engine: per-view category visibility and overrides (projection and cut lines: color, weight, pattern; surface and cut fills: color, pattern; halftone; transparency), element overrides and hiding in one view, applied to plans, ceiling plans, elevations, sections, 3D views, sheets and PDF/SVG export. Commands `view_graphics_get`, `view_graphics_set`, `element_override_set` and `element_hide_in_view`; deleting an element drops its overrides.
- View filters: named, project-wide sets of categories and rules on parameters (any schedule field or the level elevation; equals, contains, begins with, greater/less (or equal), has (no) value and their negations; all or any rule). Views apply them in order with visibility and overrides — the first matching filter wins per attribute, element overrides beat filters and filters beat the category. Commands `filter_create`, `filter_set`, `filter_delete`, `filters_list`, `filter_elements`, `view_filter_add`, `view_filter_set`, `view_filter_move` and `view_filter_remove`.
- View templates: scale, category graphics, filters, plan cut height, building services display and structural display, each with an include flag. Applied once (`view_template_apply`) or assigned (`view_template_assign`): an assigned view follows the template in the same undo step and refuses edits of what it controls; deleting a template frees its views. Commands `template_create` (from a view), `template_set`, `template_delete` and `templates_list`; `view_get` reports a view's `template`.
- Temporary hide/isolate of elements and categories per view (`view_temporary`): session display state, not saved, not undone and not printed; `view_temporary_apply` makes it permanent as one undo step.
- View graphics in the app (*View → Graphics*): a *Visibility/Graphics* dialog (VG) with model and annotation category tables and a filters tab, a filter editor (categories, rules, AND/OR, matching count), a view template manager (*Manage* too) with an include box per property, *Apply Properties* and *Assign to View*, *Override Graphics* and *Hide in View* (EH) for the selection, and the view's template in its properties (controlled properties read-only). Temporary *Hide/Isolate* of elements and categories (HH, HI, HC, IC, HR) marks the view with a colored border and a chip with *Reset* and *Make Permanent*. 3D views show category and element colors, transparency, halftone and hiding.
- Construction phases (renovation work) in the engine: an ordered list of project phases (*Existing* and *New Construction* in the template) that elements are built in and demolished in, and a phase and phase filter for each plan, elevation, section, 3D view and schedule. Relative to the view's phase an element is existing, demolished, new or temporary (built and demolished in it); elements built later or demolished earlier are left out. Phase filters (*Show All*, *Show Complete*, *Show New*, *Show Previous + Demo*, *Show Previous + New*, *Show Previous Phase*, *Show Demo + New*, and your own) show each status by category, with the project's graphics for it (existing faint, demolished dashed, temporary faint and dashed) or not at all, in plans, sections, elevations, 3D, schedules, sheets and PDF/SVG export; view templates control the phase filter. Doors, windows and rebar follow their hosts' phases (demolished with them unless demolished earlier), copies are built in the creation phase, rooms are bounded by the walls standing at the end of their phase, and views of a phase that leaves elements out get geometry of their own (later doors do not open existing walls; walls join within the phase). Schedules gain *Phase Created* and *Phase Demolished* fields. Commands `phases_list`, `phase_create`, `phase_set`, `phase_delete` (with `merge_into`), `elements_phase_set`, `elements_demolish`, `phase_creation_set`, `phase_filters_list`, `phase_filter_create`, `phase_filter_set`, `phase_filter_delete`, `phase_graphics_get` and `phase_graphics_set`; `view_set` takes `phase` and `phase_filter`, `template_set` a `phase_filter`, and `element_get` and `view_get` report them.
- IFC export writes each element's construction phase status relative to the last phase as the IFC4 `Status` enumeration (`NEW`, `EXISTING`, `DEMOLISH`, `TEMPORARY`) in its common property set (`Pset_WallCommon`, `Pset_DoorCommon`…), the names of its phases in `ConstructCAD_Phasing` and the project's phases in `ConstructCAD_Phases`; import restores the phases by name, or maps `Status` alone onto the last two phases. CI validates a phases fixture.
- Construction phases in the app: *Manage → Phases* (phases: add, insert, rename, reorder, delete with a merge target; phase filters: each status not displayed, by category or overridden; graphics of each status), with OK/Cancel undoing as one dialog. Views (plans, elevations, sections, 3D, schedules) get *Phase* and *Phase Filter* in a *Phasing* section of their properties (read-only when the template controls the filter), elements *Phase Created* and *Phase Demolished*, and view templates a *Phase Filter* row. *Modify → Demolish* (DE) demolishes the clicked elements in the active view's phase. New elements are built in the active view's phase (`phase_creation_set`), 3D views show their phase's geometry and schedules list their phase's elements.
- `ccad --version` (`-V`) prints the version.
- Installers in every release, built by the release workflow from `packaging/`: a Windows MSI (WiX; per-machine under Program Files, Start menu shortcut, `.ccad` file association, optional `ccad` on the PATH, in-place upgrades), a Linux `.deb` and AppImage (desktop entry, icon, `.ccad` MIME type) and a macOS `.dmg` per architecture with an ad-hoc signed `ConstructCAD.app` (icon, `.ccad` document type). All are listed in `SHA256SUMS.txt`, next to the existing archives.

### Changed

- `.ccad` format version 11 (construction phases); older files open with the template's phases, everything built in the last one and views showing all. Version 10 added view graphics, filters and templates; older files open with default graphics. Version 9 added arc walls and roof openings; older files open with straight walls and roofs without openings.
- On Linux the window reports the app id `constructcad`, matching the desktop entry, so desktops show its icon and name.

## [0.3.0] - 2026-10-21

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

- Building services (MEP), a new *MEP* tab for plumbing and electrical work:
  - Pipes and conduits, two new categories: straight runs with an elevation at each end, either level, sloped or vertical (risers). The *Pipe* (PI) and *Conduit* (CN) tools chain runs, and a drain keeps falling from one run to the next. Their types set the outside diameter and material; the template has PVC supply pipes (20–32 mm), drain pipes (40–100 mm) and flexible conduits (20–32 mm).
  - Systems and circuits are project settings: cold and hot water, sanitary drainage, vent and rainwater, from the template, plus power and lighting circuits fed by a distribution board, with a voltage. Each has a code shown in tags and a color. *Systems and Circuits* edits them and shows the circuit loads.
  - Pipes take their system's defaults. Supply pipes run in the walls at 0.60 m, drains under the floor at -0.30 m falling 2 % up to 75 mm and 1 % above. Conduits run in the slab above.
  - Plumbing fixtures and electrical devices are components with MEP data. Fixtures carry drainage fixture units, a supply weight and hot water; devices are general-purpose and dedicated outlets, switches, ceiling and wall lights, and distribution boards, with their power in VA. The library gains a shower, kitchen sink, floor drain, outlets, switch, lights and board. *Plumbing Fixture* and *Electrical Device* place them, devices at their usual mounting height and on the chosen circuit.
  - Plumbing and electrical plans (*MEP → Documentation*, `view_create` kinds `plumbing_plan` and `electrical_plan`) draw pipes and conduits over the architecture in their system's color. Tags give the system code, diameter and slope (`ES Ø100  i = 1,0 %`), and conduit tags list their circuits. Devices use the usual plan symbols: an outlet triangle filled by height, a ceiling light circle with its circuit and power, the switch, the board. Ordinary plans leave pipes, conduits and devices out. Pipes and conduits show in 3D, sections and elevations.
  - Schedules: pipes and conduits list system, diameter, length and slope (*MEP Schedules*), and components gain *System*, *Power* and *Fixture units* fields.
  - IFC export writes `IfcPipeSegment` and `IfcCableCarrierSegment` (circular profiles along each run, with length quantities and type objects). Devices are written as `IfcOutlet`, `IfcSwitchingDevice`, `IfcLightFixture` and `IfcElectricDistributionBoard`. Systems become `IfcDistributionSystem` and circuits `IfcDistributionCircuit`, grouping their elements. CI validates an MEP fixture.
  - Networks and fittings: pipes of one system, and any conduits, connect where their ends meet (within a centimeter) or where an end lands on another run. Fittings are derived like wall joins. Elbows (with their angle), couplings, reducers, tees and crosses show in plumbing plans as dots in the system's color, and conduit junctions as boxes. They appear as sockets in 3D and are exported as `IfcPipeFitting`/`IfcCableCarrierFitting` with type objects, grouped in their system. `mep_fittings` and *Systems and Circuits* count them by kind, angle and diameter.
  - *Connect to Network* (`mep_connect`) branches the selected fixtures to the nearest sanitary drain, falling towards it with a drop from the floor and 100 mm for toilets, and to the nearest cold and hot water pipes. It joins devices to the nearest conduit with a drop from their mounting height, on their circuit. The pipe and conduit tools snap to run ends, runs and fixtures.
  - Sizing (*MEP → Analysis → Sizing*, `mep_sizing`), from the networks with simplified tables of common Brazilian practice:
    - Sanitary drains carry the fixture units upstream of them, towards the network's lowest node, and get the usual branch diameters, with at least 100 mm after a toilet.
    - Water pipes carry a flow of 0.3 √ΣP l/s from the weights of the fixtures they feed, coming from the highest node, and get the smallest PVC pipe kept at most 3 m/s.
    - Circuits get their design current, the next standard breaker and a copper conductor that carries it (at least 1.5 mm² for lighting, 2.5 mm² for power). The conductor grows until the voltage drop to the farthest device, measured along the conduits (else in a straight line), is at most 4 %.
    - *Apply Diameters* (`mep_size_apply`) gives undersized pipes the smallest pipe type that is wide enough.
  - Panel diagrams, a new view category (*MEP → Documentation → Panel Diagram*, `view_create` kind `panel_diagram`): the single-line diagram of a distribution board and its load schedule, with the circuits' points, load, current, breaker, conductor and voltage drop, the totals and the main breaker. Diagrams go on sheets like any drawing and are deleted with their board. Views draw their own headings in the interface language.
  - In plumbing and electrical plans, the architecture is drawn faint (a new faint pen, with fills lightened) so the services stand out.
  - Engine commands `pipe_create`, `conduit_create`, `systems_list`, `system_create`, `system_set`, `system_delete`, `system_assign` and `circuit_loads` (installed load and current per circuit and board). `component_place` takes a `system`. Systems resolve by id, name, code or kind.

- Component pieces can be profiles of lines and arcs drawn on the plan, front or side plane, extruded along the plane's normal or revolved about an axis in the plane, and any piece can be a void cut out of the solid pieces it overlaps. Plans, 3D, schedules, legends and IFC export use the resolved pieces; the component editor edits them and previews plan, front and 3D edges of the real faces.
- Interface languages: Ukrainian, Spanish, French, German, Italian and Polish, chosen in the language menu, by `--lang` or from the system locale. Every language except English uses a decimal comma.

### Changed

- `.ccad` format version 8 (foundations, steel sections, sloped beams, reinforcement, structural plans, pipes, conduits, systems and MEP component data); older files still open and get the MEP types, library and plumbing systems.
- Elements cut by joined geometry (other than walls) are exported to IFC with their cut body as a B-rep, matching their exported volume.
- `*_set` commands replace an enum value given as another variant (a column section switching from rectangular to a profile) instead of merging the two.
- IFC validation tessellates curved profiles finely enough to compare their volumes, and CI validates a structural fixture.

### Fixed

- Seven ribbon icons (column, schedule, stair, text, truss, 3D view, window) were drawn without their soft fill.

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

[Unreleased]: https://github.com/francisreboucas/constructcad/compare/v0.6.1...HEAD
[0.6.1]: https://github.com/francisreboucas/constructcad/compare/v0.6.0...v0.6.1
[0.6.0]: https://github.com/francisreboucas/constructcad/compare/v0.5.0...v0.6.0
[0.5.0]: https://github.com/francisreboucas/constructcad/compare/v0.4.2...v0.5.0
[0.4.2]: https://github.com/francisreboucas/constructcad/compare/v0.4.1...v0.4.2
[0.4.1]: https://github.com/francisreboucas/constructcad/compare/v0.4.0...v0.4.1
[0.4.0]: https://github.com/francisreboucas/constructcad/compare/v0.3.0...v0.4.0
[0.3.0]: https://github.com/francisreboucas/constructcad/compare/v0.2.1...v0.3.0
[0.2.1]: https://github.com/francisreboucas/constructcad/compare/v0.2.0...v0.2.1
[0.2.0]: https://github.com/francisreboucas/constructcad/compare/v0.1.0...v0.2.0
[0.1.0]: https://github.com/francisreboucas/constructcad/releases/tag/v0.1.0
