# Changelog

All notable changes to this project are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- IFC4 export (`cc_ifc` crate and *File → Export IFC…*), validated in CI with IfcOpenShell.
- Initial MVP: parametric walls, doors, windows, floors, ceilings and roofs; plans, ceiling plans, elevations, sections, 3D views, legends, schedules and sheets; English and Portuguese (Brazil) interface; `.ccad` project files.

### Fixed

- Per-wall volumes in schedules were wrong for walls joined at corners; they are now computed exactly.
