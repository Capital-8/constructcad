# Command reference

<!-- Generated from the engine's catalog: `cargo run -p cc_cli -- reference > docs/commands.md`. Do not edit by hand. -->

Every command runs the same way from the desktop app, `ccad run` / `ccad script` and MCP (see [the automation guide](automation.md)). Lengths are in meters, angles in degrees, points `[x, y]` in plan (in a view's drawing coordinates for dimensions). *id or name* parameters accept a numeric id or the name of a level, type, view or material. Kinds: **query** reads the model, **edit** changes it (one undo step, refused if it would leave the model invalid), **file** works with files or replaces the project.

## Contents

- **Project and files**: [`project_new`](#project_new), [`project_open`](#project_open), [`project_save`](#project_save), [`project_info`](#project_info), [`project_info_set`](#project_info_set), [`undo`](#undo), [`redo`](#redo), [`units_get`](#units_get), [`units_set`](#units_set), [`language_set`](#language_set), [`ifc_import`](#ifc_import), [`ifc_export`](#ifc_export), [`sheet_export`](#sheet_export), [`schedule_export_csv`](#schedule_export_csv), [`component_load`](#component_load), [`component_save`](#component_save)
- **Queries**: [`categories_list`](#categories_list), [`levels_list`](#levels_list), [`types_list`](#types_list), [`type_get`](#type_get), [`elements_list`](#elements_list), [`element_get`](#element_get), [`groups_list`](#groups_list), [`measure`](#measure), [`views_list`](#views_list), [`view_get`](#view_get), [`materials_list`](#materials_list), [`material_get`](#material_get), [`schedule_table`](#schedule_table)
- **Creating elements**: [`wall_create`](#wall_create), [`door_create`](#door_create), [`window_create`](#window_create), [`floor_create`](#floor_create), [`ceiling_create`](#ceiling_create), [`roof_create`](#roof_create), [`room_create`](#room_create), [`grid_create`](#grid_create), [`column_create`](#column_create), [`beam_create`](#beam_create), [`stair_create`](#stair_create), [`railing_create`](#railing_create), [`component_place`](#component_place)
- **Modifying elements**: [`elements_delete`](#elements_delete), [`elements_move`](#elements_move), [`elements_copy`](#elements_copy), [`elements_rotate`](#elements_rotate), [`elements_mirror`](#elements_mirror), [`elements_pin`](#elements_pin), [`element_flip`](#element_flip), [`elements_turn`](#elements_turn), [`elements_export`](#elements_export), [`elements_paste`](#elements_paste), [`wall_join`](#wall_join), [`geometry_join`](#geometry_join), [`geometry_unjoin`](#geometry_unjoin), [`group_create`](#group_create), [`group_ungroup`](#group_ungroup), [`trim_corner`](#trim_corner), [`trim_extend`](#trim_extend), [`element_split`](#element_split), [`element_offset`](#element_offset), [`element_align`](#element_align), [`element_move_end`](#element_move_end), [`element_set`](#element_set), [`elements_set_type`](#elements_set_type)
- **Dimensions and text**: [`dimension_create`](#dimension_create), [`dimension_set_value`](#dimension_set_value), [`text_create`](#text_create), [`text_add_leader`](#text_add_leader), [`text_remove_leader`](#text_remove_leader)
- **Types, components, materials**: [`type_set`](#type_set), [`type_duplicate`](#type_duplicate), [`type_delete`](#type_delete), [`component_create`](#component_create), [`material_create`](#material_create), [`material_duplicate`](#material_duplicate), [`material_rename`](#material_rename), [`material_set`](#material_set), [`material_delete`](#material_delete)
- **Levels and views**: [`level_create`](#level_create), [`level_set`](#level_set), [`level_delete`](#level_delete), [`view_create`](#view_create), [`view_set`](#view_set), [`view_duplicate`](#view_duplicate), [`view_delete`](#view_delete), [`sheet_place_view`](#sheet_place_view), [`sheet_remove_viewport`](#sheet_remove_viewport)

## Project and files

### `project_new`

*file* — Start a new project from the template (unsaved changes are discarded).

No parameters.

### `project_open`

*file* — Open a ConstructCAD project (.ccad), replacing the session's project. Damaged files are repaired and the number of repairs reported.

| Parameter | Type | | Description |
|---|---|---|---|
| `path` | string | required | Path of the .ccad file. |

### `project_save`

*file* — Save the project to its file, or to `path` (which becomes its file).

| Parameter | Type | | Description |
|---|---|---|---|
| `path` | string | optional | Where to save; required when the project has no file yet. |

### `project_info`

*query* — Project information, file, unsaved state and counts.

No parameters.

### `project_info_set`

*edit* — Change the project information (title block fields).

| Parameter | Type | | Description |
|---|---|---|---|
| `address` | string | optional | Project address. |
| `author` | string | optional | Author of the drawings. |
| `client` | string | optional | Client name. |
| `name` | string | optional | Project name. |
| `number` | string | optional | Project number. |

### `undo`

*edit* — Undo the last edit.

No parameters.

### `redo`

*edit* — Redo the last undone edit.

No parameters.

### `units_get`

*query* — The project units (length, area, volume, angle, slope, currency, number format).

No parameters.

### `units_set`

*edit* — Change the project units: `values` is merged into the object `units_get` returns (only the fields given change).

| Parameter | Type | | Description |
|---|---|---|---|
| `values` | JSON | required | Fields to change, shaped like the result of `units_get`. |

### `language_set`

*edit* — Language of default names, messages and sheet labels (`en` or `pt-BR`).

| Parameter | Type | | Description |
|---|---|---|---|
| `language` | string | required | Language code: `en` or `pt-BR`. |

### `ifc_import`

*file* — Import an IFC file (IFC2X3 or IFC4), replacing the session's project. The result has no file yet; save it with `project_save` and a path.

| Parameter | Type | | Description |
|---|---|---|---|
| `path` | string | required | Path of the .ifc file. |

### `ifc_export`

*file* — Export the project as IFC4.

| Parameter | Type | | Description |
|---|---|---|---|
| `path` | string | required | Path of the .ifc file to write. |

### `sheet_export`

*file* — Export a sheet as vector PDF or SVG (by `format`, else by the file extension).

| Parameter | Type | | Description |
|---|---|---|---|
| `path` | string | required | Path of the file to write (.pdf or .svg). |
| `sheet` | id or name | required | Sheet id or name. |
| `format` | string | optional | `pdf` or `svg`. |

### `schedule_export_csv`

*file* — Export a schedule view as CSV (UTF-8; semicolons when the decimal comma is used).

| Parameter | Type | | Description |
|---|---|---|---|
| `path` | string | required | Path of the .csv file to write. |
| `view` | id or name | required | Schedule view id or name. |

### `component_load`

*edit* — Load a component file (.ccomp): its types and missing materials join the project.

| Parameter | Type | | Description |
|---|---|---|---|
| `path` | string | required | Path of the .ccomp file. |

### `component_save`

*file* — Save a component (all its types and the materials they use) to a .ccomp file.

| Parameter | Type | | Description |
|---|---|---|---|
| `component` | string | required | Component name (as in a component type's `params.component`). |
| `path` | string | required | Path of the .ccomp file to write. |

## Queries

### `categories_list`

*query* — Element categories with their display names and how many elements and types each has.

No parameters.

### `levels_list`

*query* — Levels from the lowest, with their elevations (meters).

No parameters.

### `types_list`

*query* — Types, optionally of one category.

| Parameter | Type | | Description |
|---|---|---|---|
| `category` | string | optional | Category name (`wall`, `door`, `component`…). |

### `type_get`

*query* — A type with all its parameters (`params`, editable with `type_set`) and identity data (`info`).

| Parameter | Type | | Description |
|---|---|---|---|
| `type` | id or name | required | Type id or name. |

### `elements_list`

*query* — Elements, optionally of one category, level and/or type, or only those shown in a view (all the instances of a type visible in it, for example), with a short summary each.

| Parameter | Type | | Description |
|---|---|---|---|
| `category` | string | optional | Category name (`wall`, `door`, `component`…); see `categories_list`. |
| `level` | id or name | optional | Level id or name. |
| `type` | id or name | optional | Type id or name: only its instances. |
| `view` | id or name | optional | View id or name: only the elements the view shows. |

### `element_get`

*query* — An element with all its parameters (`params`, editable with `element_set`) and derived values (length, height, thickness, volume, room area and perimeter).

| Parameter | Type | | Description |
|---|---|---|---|
| `id` | integer | required | Element id. |

### `groups_list`

*query* — The project's groups with their members.

No parameters.

### `measure`

*query* — Measure between two points of a view (meters on its plane: plan coordinates in plans): the distance, its horizontal and vertical parts and its angle. When both points lie on lines (`line_a`, `line_b`, each two points) that are parallel, the distance between the lines is given too. Nothing is created.

| Parameter | Type | | Description |
|---|---|---|---|
| `a` | [x, y] | required | First point `[x, y]`. |
| `b` | [x, y] | required | Second point `[x, y]`. |
| `line_a` | [[x, y], [x, y]] | optional | The line the first point was taken on, as two points. |
| `line_b` | [[x, y], [x, y]] | optional | The line the second point was taken on, as two points. |

### `views_list`

*query* — Views (plans, ceiling plans, elevations, sections, 3D, legends, schedules, sheets), optionally of one category.

| Parameter | Type | | Description |
|---|---|---|---|
| `category` | string | optional | `floor_plan`, `ceiling_plan`, `three_d`, `elevation`, `section`, `legend`, `schedule` or `sheet`. |

### `view_get`

*query* — A view with all its parameters (`params`, editable with `view_set`).

| Parameter | Type | | Description |
|---|---|---|---|
| `view` | id or name | required | View id or name. |

### `materials_list`

*query* — Materials with their colors and how many types use each.

No parameters.

### `material_get`

*query* — A material with all its fields (editable with `material_set`).

| Parameter | Type | | Description |
|---|---|---|---|
| `material` | id or name | required | Material name (or index in `materials_list`). |

### `schedule_table`

*query* — The table of a schedule view: headers, one row per element (with its id) and totals, formatted with the project units.

| Parameter | Type | | Description |
|---|---|---|---|
| `view` | id or name | required | Schedule view id or name. |

## Creating elements

### `wall_create`

*edit* — Create a straight wall from `start` to `end` on a level. Its top is `height` above its base, or attached to `top_level` (plus `top_offset`).

| Parameter | Type | | Description |
|---|---|---|---|
| `end` | [x, y] | required | End point of the wall line. |
| `start` | [x, y] | required | Start point of the wall line. |
| `base_offset` | number | optional | Base offset from the level, meters. |
| `height` | number | optional | Unconnected height in meters (default 3). |
| `level` | id or name | optional | Level id or name (default: the lowest level). |
| `location_line` | string | optional | Which line of the wall `start`-`end` is: `Centerline` (default), `CoreCenter`, `FinishExterior`, `FinishInterior`, `CoreExterior` or `CoreInterior`. |
| `top_level` | id or name | optional | Level the top is attached to, instead of a height. |
| `top_offset` | number | optional | Offset of the top from `top_level`, meters. |
| `type` | id or name | optional | Wall type id or name (default: the default wall type). |

### `door_create`

*edit* — Place a door in a wall (centered on the wall unless `offset` or `point` is given).

| Parameter | Type | | Description |
|---|---|---|---|
| `flip_facing` | boolean | optional | Face the other side of the wall. |
| `flip_hand` | boolean | optional | Hinge on the other jamb. |
| `offset` | number | optional | Distance along the wall from its start to the opening's center, meters. |
| `point` | [x, y] | optional | A point near the wall: the opening is centered at its projection, on its side. |
| `sill` | number | optional | Sill height (default: the type's default sill for windows, 0 for doors). |
| `type` | id or name | optional | Door or window type id or name (default: the default type). |
| `wall` | integer | optional | Id of the host wall; omit it to use the wall nearest `point`. |

### `window_create`

*edit* — Place a window in a wall (centered on the wall unless `offset` or `point` is given).

| Parameter | Type | | Description |
|---|---|---|---|
| `flip_facing` | boolean | optional | Face the other side of the wall. |
| `flip_hand` | boolean | optional | Hinge on the other jamb. |
| `offset` | number | optional | Distance along the wall from its start to the opening's center, meters. |
| `point` | [x, y] | optional | A point near the wall: the opening is centered at its projection, on its side. |
| `sill` | number | optional | Sill height (default: the type's default sill for windows, 0 for doors). |
| `type` | id or name | optional | Door or window type id or name (default: the default type). |
| `wall` | integer | optional | Id of the host wall; omit it to use the wall nearest `point`. |

### `floor_create`

*edit* — Create a floor slab from an outline (or the loop of walls).

| Parameter | Type | | Description |
|---|---|---|---|
| `boundary` | [[x, y], …] | optional | Closed outline (at least 3 points); omit with `from_walls`. |
| `from_walls` | boolean | optional | Follow the loop of walls of the level (exterior faces; interior faces for ceilings). |
| `level` | id or name | optional | Level id or name (default: the lowest level). |
| `offset` | number | optional | Floors: top above the level (default 0). Ceilings: bottom above the level (default 2.6). Roofs: base above the level (default: top of the tallest wall). |
| `type` | id or name | optional | Type id or name (default: the category's default type). |

### `ceiling_create`

*edit* — Create a ceiling from an outline (or the inside of the loop of walls).

| Parameter | Type | | Description |
|---|---|---|---|
| `boundary` | [[x, y], …] | optional | Closed outline (at least 3 points); omit with `from_walls`. |
| `from_walls` | boolean | optional | Follow the loop of walls of the level (exterior faces; interior faces for ceilings). |
| `level` | id or name | optional | Level id or name (default: the lowest level). |
| `offset` | number | optional | Floors: top above the level (default 0). Ceilings: bottom above the level (default 2.6). Roofs: base above the level (default: top of the tallest wall). |
| `type` | id or name | optional | Type id or name (default: the category's default type). |

### `roof_create`

*edit* — Create a roof over an outline (or the loop of walls): flat by default; change its shape (shed, gable, hip) with `element_set`.

| Parameter | Type | | Description |
|---|---|---|---|
| `boundary` | [[x, y], …] | optional | Closed outline (at least 3 points); omit with `from_walls`. |
| `from_walls` | boolean | optional | Follow the loop of walls of the level (exterior faces; interior faces for ceilings). |
| `level` | id or name | optional | Level id or name (default: the lowest level). |
| `offset` | number | optional | Floors: top above the level (default 0). Ceilings: bottom above the level (default 2.6). Roofs: base above the level (default: top of the tallest wall). |
| `overhang` | number | optional | Overhang beyond the outline, meters (default 0.5). |
| `type` | id or name | optional | Type id or name (default: the category's default type). |

### `room_create`

*edit* — Create a room at a point enclosed by room-bounding walls (its boundary follows the walls).

| Parameter | Type | | Description |
|---|---|---|---|
| `point` | [x, y] | required | A point inside the room. |
| `height` | number | optional | Upper limit above the level (default: the level's height). |
| `level` | id or name | optional | Level id or name (default: the lowest level). |
| `name` | string | optional | Default: the room default name. |
| `number` | string | optional | Default: the next free number. |

### `grid_create`

*edit* — Create a grid line (named with the next free number unless `name` is given).

| Parameter | Type | | Description |
|---|---|---|---|
| `end` | [x, y] | required | End point. |
| `start` | [x, y] | required | Start point (where the bubble is). |
| `level` | id or name | optional | Level id or name (default: the lowest level). |
| `name` | string | optional | Grid name (default: the next free number). |

### `column_create`

*edit* — Place a column; its top goes to the level above (or `height` when there is none).

| Parameter | Type | | Description |
|---|---|---|---|
| `position` | [x, y] | required | Center of the column in plan. |
| `height` | number | optional | Unconnected height (default: the level's height). |
| `level` | id or name | optional | Level id or name (default: the lowest level). |
| `rotation` | number | optional | Rotation in plan, degrees. |
| `structural` | boolean | optional | Load-bearing (default true); an architectural column is cut by the walls it is joined with. |
| `top_level` | id or name | optional | Level the top is attached to (default: the level above). |
| `top_offset` | number | optional | Offset of the top from `top_level`, meters. |
| `type` | id or name | optional | Type id or name (default: the category's default type). |

### `beam_create`

*edit* — Create a beam from `start` to `end`; its top is `top_offset` above the level (default: the level's height).

| Parameter | Type | | Description |
|---|---|---|---|
| `end` | [x, y] | required | End of the beam axis. |
| `start` | [x, y] | required | Start of the beam axis. |
| `level` | id or name | optional | Level id or name (default: the lowest level). |
| `top_offset` | number | optional | Top of the beam above the level, meters (default: the level height). |
| `type` | id or name | optional | Type id or name (default: the category's default type). |

### `stair_create`

*edit* — Create a straight stair starting at `start` and rising along `direction` (degrees) up to the level above (or `height`). Risers follow the type.

| Parameter | Type | | Description |
|---|---|---|---|
| `start` | [x, y] | required | Start of the run (center of the first step's edge). |
| `direction` | number | optional | Direction of the run in plan, degrees counter-clockwise from +X. |
| `height` | number | optional | Height when there is no level above (default 3). |
| `level` | id or name | optional | Level id or name (default: the lowest level). |
| `top_level` | id or name | optional | Level the stair rises to (default: the level above). |
| `type` | id or name | optional | Type id or name (default: the category's default type). |

### `railing_create`

*edit* — Create a railing along a path (at least 2 points).

| Parameter | Type | | Description |
|---|---|---|---|
| `path` | [[x, y], …] | required | Points of the railing path (at least 2). |
| `base_offset` | number | optional | Base above the level, meters. |
| `level` | id or name | optional | Level id or name (default: the lowest level). |
| `type` | id or name | optional | Type id or name (default: the category's default type). |

### `component_place`

*edit* — Place a component (furniture, fixture…) of a component type.

| Parameter | Type | | Description |
|---|---|---|---|
| `position` | [x, y] | required | Insertion point in plan. |
| `type` | id or name | required | Component type id or name. |
| `base_offset` | number | optional | Base above the level, meters. |
| `level` | id or name | optional | Level id or name (default: the lowest level). |
| `rotation` | number | optional | Rotation in plan, degrees. |

## Modifying elements

### `elements_delete`

*edit* — Delete elements (doors and windows of deleted walls go too; pinned elements stay).

| Parameter | Type | | Description |
|---|---|---|---|
| `ids` | [id, …] | required | Element ids. |

### `elements_move`

*edit* — Move elements by `delta` (walls take their doors and windows; doors and windows alone slide along their wall).

| Parameter | Type | | Description |
|---|---|---|---|
| `delta` | [x, y] | required | Displacement `[dx, dy]` in meters. |
| `ids` | [id, …] | required | Element ids. |

### `elements_copy`

*edit* — Copy elements by `delta`; with `count` > 1, a linear array of that many copies spaced by `delta`.

| Parameter | Type | | Description |
|---|---|---|---|
| `delta` | [x, y] | required | Displacement `[dx, dy]` of the first copy, meters (the next ones repeat it). |
| `ids` | [id, …] | required | Element ids. |
| `count` | integer | optional | Number of copies (default 1, at most 1000). |

### `elements_rotate`

*edit* — Rotate elements by `angle` degrees (counter-clockwise) about `center` (default: the center of their extents); with `copy`, rotated copies.

| Parameter | Type | | Description |
|---|---|---|---|
| `angle` | number | required | Angle in degrees, counter-clockwise. |
| `ids` | [id, …] | required | Element ids. |
| `center` | [x, y] | optional | Center of rotation (default: the center of the elements' extents). |
| `copy` | boolean | optional | Rotate copies, keeping the originals. |

### `elements_mirror`

*edit* — Mirror elements across an axis: two points, or the line of a wall, grid, beam or railing (`axis_element`). Copies by default (`copy: false` moves them).

| Parameter | Type | | Description |
|---|---|---|---|
| `ids` | [id, …] | required | Element ids. |
| `axis` | [[x, y], [x, y]] | optional | Two points of the axis. |
| `axis_element` | integer | optional | Id of an element whose line is the axis. |
| `copy` | boolean | optional | Keep the originals and mirror copies (default true). |

### `elements_pin`

*edit* — Pin elements (they cannot be moved or deleted) or unpin them.

| Parameter | Type | | Description |
|---|---|---|---|
| `ids` | [id, …] | required | Element ids. |
| `pinned` | boolean | required | `true` to pin, `false` to unpin. |

### `element_flip`

*edit* — Flip a door or window to the other side of its wall (`facing`) and/or to the other jamb (`hand`).

| Parameter | Type | | Description |
|---|---|---|---|
| `id` | integer | required | Id of the door or window. |
| `facing` | boolean | optional | Flip to the other side of the wall. |
| `hand` | boolean | optional | Flip the hinge to the other jamb. |

### `elements_turn`

*edit* — Turn elements a quarter: components and columns rotate 90° about their insertion point; doors and windows flip to the other side of their wall (the Space key on a selection). Others and pinned elements are left alone.

| Parameter | Type | | Description |
|---|---|---|---|
| `ids` | [id, …] | required | Element ids. |

### `elements_export`

*query* — The elements as a portable set for `elements_paste`: doors and windows of the given walls come along. The set keeps type, level and view ids, so it pastes into this project or one sharing them.

| Parameter | Type | | Description |
|---|---|---|---|
| `ids` | [id, …] | required | Element ids. |

### `elements_paste`

*edit* — Paste a set from `elements_export` moved by `delta`, on `level` when given (else on their own levels). Elements whose type, level, view or wall does not exist here are skipped. Returns the new ids.

| Parameter | Type | | Description |
|---|---|---|---|
| `elements` | JSON | required | The `elements` of an `elements_export` result. |
| `delta` | [x, y] | optional | Displacement `[dx, dy]`, meters (default: none). |
| `level` | id or name | optional | Level id or name to paste on (default: each element's own level). |

### `wall_join`

*edit* — Allow or disallow joining at one end of a wall.

| Parameter | Type | | Description |
|---|---|---|---|
| `end` | string | required | `start` or `end`. |
| `id` | integer | required | Wall id. |
| `joined` | boolean | required | `true` to allow joining at that end, `false` to keep it free. |

### `geometry_join`

*edit* — Join the geometry of two elements that touch or overlap (walls, floors, ceilings, roofs, columns, beams, stairs, railings, components). Where they overlap, one cuts the other: structural columns and beams cut walls, floors, ceilings and roofs; floors, ceilings and roofs cut walls; walls cut architectural columns; between equals the first cuts the second. The shared volume is counted once, and plans and sections draw two elements cut in the same material as one piece.

| Parameter | Type | | Description |
|---|---|---|---|
| `first` | integer | required | The first element. |
| `second` | integer | required | The second element. |

### `geometry_unjoin`

*edit* — Unjoin the geometry of two joined elements: each keeps its whole body again.

| Parameter | Type | | Description |
|---|---|---|---|
| `first` | integer | required | The first element. |
| `second` | integer | required | The second element. |

### `group_create`

*edit* — Group elements: selecting one of them selects them all, so they move, copy and rotate together. Elements already in a group leave it.

| Parameter | Type | | Description |
|---|---|---|---|
| `ids` | [id, …] | required | The elements (at least two). |
| `name` | string | optional | Group name (default: "Group N"). |

### `group_ungroup`

*edit* — Ungroup: the group goes, its elements stay.

| Parameter | Type | | Description |
|---|---|---|---|
| `group` | integer | required | Group id (see `groups_list`, or `group` in `element_get`). |

### `trim_corner`

*edit* — Trim or extend two linear elements (walls, beams, grids) to meet at a corner; each pick point marks the part of its element to keep.

| Parameter | Type | | Description |
|---|---|---|---|
| `a` | integer | required | First element id. |
| `b` | integer | required | Second element id. |
| `pick_a` | [x, y] | required | A point on the part of `a` to keep. |
| `pick_b` | [x, y] | required | A point on the part of `b` to keep. |

### `trim_extend`

*edit* — Trim or extend a linear element to the line of a reference element; the pick point marks the part to keep.

| Parameter | Type | | Description |
|---|---|---|---|
| `id` | integer | required | Element to trim or extend. |
| `pick` | [x, y] | required | A point on the part to keep. |
| `reference` | integer | required | Id of the element whose line is the boundary. |

### `element_split`

*edit* — Split a linear element (wall, beam, grid) at a point; returns the new element.

| Parameter | Type | | Description |
|---|---|---|---|
| `id` | integer | required | Element id. |
| `point` | [x, y] | required | Where to split (projected on the element's line). |

### `element_offset`

*edit* — Copy a linear element parallel at `distance` (positive: to the left of its direction).

| Parameter | Type | | Description |
|---|---|---|---|
| `distance` | number | required | Offset distance, meters: positive to the left of start→end, negative to the right. |
| `id` | integer | required | Element id. |

### `element_align`

*edit* — Move a linear element parallel onto the line of a reference element.

| Parameter | Type | | Description |
|---|---|---|---|
| `id` | integer | required | Element to move. |
| `reference` | integer | required | Element whose line it moves onto. |

### `element_move_end`

*edit* — Move one end of a wall, beam or grid (joined wall ends follow).

| Parameter | Type | | Description |
|---|---|---|---|
| `end` | string | required | `start` or `end`. |
| `id` | integer | required | Element id. |
| `point` | [x, y] | required | New position of that end. |

### `element_set`

*edit* — Change an element: its level (doors and windows of a wall follow it), type (of the same category), mark, comments, and any of its `params` as `element_get` shows them (merged: only the fields given change, e.g. `{"height": 2.8}` for a wall or `{"shape": {"Gable": {"slope": 0.5, "ridge_deg": 0}}}` for a roof). The edit is refused if it would leave the model invalid.

| Parameter | Type | | Description |
|---|---|---|---|
| `id` | integer | required | Element id. |
| `comments` | string | optional | New comments. |
| `level` | id or name | optional | New level id or name. |
| `mark` | string | optional | New mark. |
| `params` | JSON | optional | Fields of the element's parameters to change. |
| `type` | id or name | optional | New type id or name (same category). |

### `elements_set_type`

*edit* — Give elements a type (all must be of the type's category), like Match Type.

| Parameter | Type | | Description |
|---|---|---|---|
| `ids` | [id, …] | required | Element ids. |
| `type` | id or name | required | Type id or name. |

## Dimensions and text

### `dimension_create`

*edit* — Place a dimension in a 2D view (points in the view's drawing coordinates: plan X/Y in plans; horizontal position and height in elevations and sections). Its ends follow the elements (or level lines) their points lie on, so it keeps measuring them. Aligned dimensions measure from `a` to `b`, linear ones along a horizontal or vertical `direction`, angular ones the angle at `vertex` between the rays through `a` and `b`, radial and diameter ones the arc centered at `a` through `b`.

| Parameter | Type | | Description |
|---|---|---|---|
| `a` | [x, y] | required | First point (center for radial and diameter). |
| `b` | [x, y] | required | Second point (a point on the arc for radial and diameter). |
| `view` | id or name | required | View id or name. |
| `direction` | string | optional | Linear dimensions: `horizontal` or `vertical` (default: the larger extent). |
| `kind` | string | optional | `aligned` (default), `linear`, `angular`, `radial` or `diameter`. |
| `offset` | number | optional | Distance of the dimension line from `a` (left of a→b positive), or the arc radius for angular ones (default 1). |
| `type` | id or name | optional | Dimension type id or name (default: the default dimension type). |
| `vertex` | [x, y] | optional | Vertex of an angular dimension. |

### `dimension_set_value`

*edit* — Give a dimension a new value (meters; degrees for angular dimensions): in plans what its moving end measures moves or stretches; between level lines in elevations and sections the upper level moves. Radial and diameter dimensions are read-only.

| Parameter | Type | | Description |
|---|---|---|---|
| `id` | integer | required | Dimension element id. |
| `value` | number | required | New value: meters, or degrees for angular dimensions. |

### `text_create`

*edit* — Place a text note in a 2D view (points in the view's drawing coordinates, as for dimensions). Without `width` it is single-line text (only line breaks in `text` break it); with `width` (paper millimeters) it is multiline text wrapped to that width. Leaders point from the text to what it describes, each with one segment or two (with an `elbow`).

| Parameter | Type | | Description |
|---|---|---|---|
| `position` | [x, y] | required | Top left corner of the text. |
| `text` | string | required | The text; a line break starts a new line. |
| `view` | id or name | required | View id or name. |
| `leaders` | [object, …] | optional | Leaders from the text. |
| `rotation` | number | optional | Rotation in degrees counter-clockwise. |
| `type` | id or name | optional | Text type id or name (default: the default text type). |
| `width` | number | optional | Wrapping width in paper millimeters (multiline text). |

### `text_add_leader`

*edit* — Add a leader to a text note.

| Parameter | Type | | Description |
|---|---|---|---|
| `end` | [x, y] | required | The point the leader points at, where its symbol is drawn. |
| `id` | integer | required | Text element id. |
| `elbow` | [x, y] | optional | A bend point that splits the leader in two segments (text → elbow → end). |
| `symbol` | string | optional | End symbol: `Arrow`, `OpenArrow`, `Triangle`, `Dot`, `Circle`, `Square`, `FilledSquare`, `Tick` or `None` (default: the text type's symbol). |

### `text_remove_leader`

*edit* — Remove a leader from a text note.

| Parameter | Type | | Description |
|---|---|---|---|
| `id` | integer | required | Text element id. |
| `index` | integer | required | Index of the leader (from 0, as in `params.leaders`). |

## Types, components, materials

### `type_set`

*edit* — Change a type: its name, its parameters (`params`, merged into what `type_get` shows: layers, sizes, materials, a component's `def`…) and its identity data (`info`: type mark, description, manufacturer, cost…). Changing a component's definition updates every type of that component, each keeping its own parameter values.

| Parameter | Type | | Description |
|---|---|---|---|
| `type` | id or name | required | A level, type, view or material: its numeric id or its name. |
| `info` | JSON | optional | Fields of the identity data to change, shaped like `info` in `type_get`. |
| `name` | string | optional | New name (unique in its category). |
| `params` | JSON | optional | Fields of the type's parameters to change, shaped like `params` in `type_get`. |
| `renames` | [[old, new], …] | optional | Component parameters renamed by a new `def`, as `[old, new]` pairs in order, so the component's other types keep their values. |

### `type_duplicate`

*edit* — Copy a type under a new name (default "<name> (copy)"); the given elements of the type switch to the copy.

| Parameter | Type | | Description |
|---|---|---|---|
| `type` | id or name | required | A level, type, view or material: its numeric id or its name. |
| `elements` | [id, …] | optional | Elements of the type to switch to the copy. |
| `name` | string | optional | Name of the copy (default: "<name> (copy)"). |

### `type_delete`

*edit* — Delete a type that no element uses (the last type of a category is kept).

| Parameter | Type | | Description |
|---|---|---|---|
| `type` | id or name | required | A level, type, view or material: its numeric id or its name. |

### `component_create`

*edit* — Create a component type: `def` holds its parameters and pieces (boxes, cylinders, extrusions whose fields are expressions over the parameters), shaped like a component type's `params.def` in `type_get`.

| Parameter | Type | | Description |
|---|---|---|---|
| `def` | JSON | required | The definition: `params` (name, value, kind, instance) and `items` (pieces), shaped like a component type's `params.def` in `type_get`. |
| `name` | string | required | Name of the component and of its first type. |

### `material_create`

*edit* — Create a material (default name "Material"; a number is added when taken).

| Parameter | Type | | Description |
|---|---|---|---|
| `color` | [id, …] | optional | Shading color `[r, g, b]`, 0–255 (default light gray). |
| `name` | string | optional | Name (default: "Material"; a number is added when taken). |

### `material_duplicate`

*edit* — Copy a material under a new name (default "<name> (copy)").

| Parameter | Type | | Description |
|---|---|---|---|
| `material` | id or name | required | A level, type, view or material: its numeric id or its name. |
| `name` | string | optional | Name of the copy (default: "<name> (copy)"). |

### `material_rename`

*edit* — Rename a material; every type using it follows.

| Parameter | Type | | Description |
|---|---|---|---|
| `material` | id or name | required | A level, type, view or material: its numeric id or its name. |
| `name` | string | required | New name. |

### `material_set`

*edit* — Change a material's fields (merged into what `material_get` shows: description, class, colors, patterns, transparency, finish, physical values…). Use `material_rename` to rename it.

| Parameter | Type | | Description |
|---|---|---|---|
| `material` | id or name | required | A level, type, view or material: its numeric id or its name. |
| `values` | JSON | required | Fields to change, shaped like the result of `material_get`. |

### `material_delete`

*edit* — Delete a material no type uses.

| Parameter | Type | | Description |
|---|---|---|---|
| `material` | id or name | required | A level, type, view or material: its numeric id or its name. |

## Levels and views

### `level_create`

*edit* — Add a level (default: 3 m above the highest, named "Level n") with its floor and ceiling plans.

| Parameter | Type | | Description |
|---|---|---|---|
| `elevation` | number | optional | Elevation in meters. |
| `name` | string | optional | Level name (default: "Level n"). |
| `plans` | boolean | optional | Also create its floor and ceiling plans (default true). |

### `level_set`

*edit* — Rename a level or change its elevation (what is attached to it moves along).

| Parameter | Type | | Description |
|---|---|---|---|
| `level` | id or name | required | A level, type, view or material: its numeric id or its name. |
| `elevation` | number | optional | New elevation, meters. |
| `name` | string | optional | New name. |

### `level_delete`

*edit* — Delete a level no element is on, with its plan views; walls rising to it keep their current height, unconnected. The last level is kept.

| Parameter | Type | | Description |
|---|---|---|---|
| `level` | id or name | required | A level, type, view or material: its numeric id or its name. |

### `view_create`

*edit* — Create a view. `kind` is `floor_plan` or `ceiling_plan` (with `level`), `section` (with `start`, `end` and optional `depth`), `elevation` (with `direction`: north, south, east or west), `three_d`, `legend` (all types), `schedule` (with `category`) or `sheet` (A3, numbered after the others).

| Parameter | Type | | Description |
|---|---|---|---|
| `kind` | string | required | `floor_plan`, `ceiling_plan`, `section`, `elevation`, `three_d`, `legend`, `schedule` or `sheet`. |
| `category` | string | optional | Schedules: the category scheduled (default `wall`). |
| `depth` | number | optional | Section depth beyond the cut line, meters (default 8). |
| `direction` | string | optional | Elevations: `north`, `south` (default), `east` or `west`. |
| `end` | [x, y] | optional | Sections: end of the cut line. |
| `level` | id or name | optional | Plans: level id or name (default: the lowest). |
| `name` | string | optional | View name (default: by kind). |
| `scale` | integer | optional | Scale denominator (e.g. 50 for 1:50). |
| `start` | [x, y] | optional | Sections: start of the cut line. |

### `view_set`

*edit* — Change a view: name, scale and its `params` as `view_get` shows them (merged: cut height, section depth, legend types, schedule fields, sort, filter and totals, sheet number and size, viewport positions…).

| Parameter | Type | | Description |
|---|---|---|---|
| `view` | id or name | required | A level, type, view or material: its numeric id or its name. |
| `name` | string | optional | New name. |
| `params` | JSON | optional | Fields of the view's parameters to change, shaped like `params` in `view_get`. |
| `scale` | integer | optional | Scale denominator (e.g. 100 for 1:100). |

### `view_duplicate`

*edit* — Copy a view (a copied sheet starts without viewports).

| Parameter | Type | | Description |
|---|---|---|---|
| `view` | id or name | required | A level, type, view or material: its numeric id or its name. |

### `view_delete`

*edit* — Delete a view (and its viewports on sheets; dimensions drawn in it go too).

| Parameter | Type | | Description |
|---|---|---|---|
| `view` | id or name | required | A level, type, view or material: its numeric id or its name. |

### `sheet_place_view`

*edit* — Place a view on a sheet at `center` (paper millimeters; default left of center). A view can be on only one sheet; sheets cannot be placed.

| Parameter | Type | | Description |
|---|---|---|---|
| `sheet` | id or name | required | A level, type, view or material: its numeric id or its name. |
| `view` | id or name | required | A level, type, view or material: its numeric id or its name. |
| `center` | [x, y] | optional | Center of the viewport on the sheet, paper millimeters. |

### `sheet_remove_viewport`

*edit* — Remove a view from a sheet.

| Parameter | Type | | Description |
|---|---|---|---|
| `sheet` | id or name | required | A level, type, view or material: its numeric id or its name. |
| `view` | id or name | required | A level, type, view or material: its numeric id or its name. |
