# Changelog

All notable changes to this project are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

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

- `.ccad` format version 2 (adds the fields above; version 1 files still open).
- Redesigned About dialog with version, author, license, repository and credits.
- Polished interface: light neutral theme with the Inter typeface, ribbon with icons grouped into captioned panels and a quick access toolbar, project browser with category icons and counts as an indented tree, property palette with headers, floating scale and zoom-to-fit control in 2D views and a clearer 3D orientation gizmo.

### Fixed

- Stray lines on roofs and other large faces in 3D sheet views, elevations and sections: faces are now ordered by comparing depths where they overlap.
- Closing the last open view tab crashed the application.
- Per-wall volumes in schedules were wrong for walls joined at corners; they are now computed exactly.
