# ConstructCAD — English
# Format: Fluent subset (key = value, { $name } placeholders).

# Categories
category-wall = Walls
category-door = Doors
category-window = Windows
category-floor = Floors
category-ceiling = Ceilings
category-roof = Roofs

# Project browser
browser-title = Project Browser
browser-floor-plans = Floor Plans
browser-ceiling-plans = Ceiling Plans
browser-3d-views = 3D Views
browser-elevations = Elevations
browser-sections = Sections
browser-legends = Legends
browser-schedules = Schedules
browser-sheets = Sheets
browser-new = + New
browser-open = Open
browser-rename = Rename
browser-duplicate = Duplicate
browser-delete = Delete
browser-place-on-sheet = Place on active sheet

# Tab titles
tab-floor-plan = Floor Plan: { $name }
tab-ceiling-plan = Ceiling Plan: { $name }
tab-elevation = Elevation: { $name }
tab-section = Section: { $name }

# Ribbon
ribbon-file = File
ribbon-architecture = Architecture
ribbon-modify = Modify
ribbon-view = View
ribbon-manage = Manage
group-project = Project
group-export = Export
group-settings = Settings
group-help = Help
group-build = Build
group-horizontal = Floors & Roofs
group-selection = Selection
group-history = History
group-create-views = Create
group-documentation = Documentation
group-navigate = Navigate

# Tools
tool-select = Select
tool-wall = Wall
tool-door = Door
tool-window = Window
tool-floor = Floor
tool-ceiling = Ceiling
tool-roof = Roof
tool-section = Section

# Actions
action-new = New
action-open = Open…
action-save = Save
action-save-as = Save As…
action-export-ifc = Export IFC…
action-undo = Undo
action-redo = Redo
action-copy = Copy
action-delete = Delete
action-cancel = Cancel
action-discard = Discard
action-ok = OK
action-zoom-fit = Zoom to Fit

# View
view-new-floor-plan = Floor Plan
view-new-ceiling-plan = Ceiling Plan
view-new-3d = 3D View
view-elevations = Elevations
view-new-legend = Legend
view-new-schedule = Schedule
view-new-sheet = Sheet

# Hints
hint-shortcut = Shortcut
hint-copy = Duplicates the selection offset by 1 m (or Ctrl + drag)
hint-select = Click to select · drag to move (Ctrl copies) · window: left-to-right selects enclosed, right-to-left selects crossing
hint-wall = Click the start and next points · type a length + Enter · Shift = orthogonal · Esc ends
hint-hosted = Hover over a wall and click · Space flips the opening
hint-sketch = Click the vertices · click the first point or press Enter to finish · Backspace removes the last one
hint-from-walls = Creates the boundary from the closed walls on this level

# Options bar
sketch-finish = Finish
sketch-from-walls = From Walls

# Status bar
status-length = Length: { $value }
status-selected = { $count } selected
status-unsaved-file = (unsaved)

# Messages
msg-plan-only = This tool works in plan views. Open a floor or ceiling plan.
msg-open-plan = Open a floor plan to use this tool.
msg-no-type = No type is available for this category.
msg-deleted = { $count } element(s) deleted.
msg-wall-too-short = Wall too short
msg-sketch-invalid = The boundary needs at least 3 points and a non-zero area.
msg-section-hint = Click two points in a plan to define the section line.
msg-no-wall-loop = No closed loop of walls was found on this level.
msg-view-already-placed = This view is already placed on a sheet.
msg-level-in-use = The level has elements and cannot be deleted.
msg-new-project = New project created.
msg-opened = Opened: { $path }
msg-saved = Saved: { $path }
msg-open-failed = Could not open the file: { $error }
msg-save-failed = Could not save the file: { $error }
msg-exported = Exported: { $path }
msg-export-failed = Could not export the file: { $error }
msg-no-gpu = The 3D view requires the wgpu renderer.
msg-no-views-open = Open a view from the Project Browser.

# Drawing labels
label-offset = Offset:
label-move = Move
label-copy = Copy

# Properties
props-title = Properties
props-nothing = Nothing selected.
props-multi = { $count } elements selected
props-edit-type = Edit Type…
props-viewport = Viewport
prop-name = Name
prop-scale = Scale
prop-level = Level
prop-category = Category
prop-base-offset = Base Offset
prop-height = Height
prop-height-offset = Height Offset from Level
prop-start-x = Start X
prop-start-y = Start Y
prop-end-x = End X
prop-end-y = End Y
prop-length = Length
prop-thickness = Thickness
prop-volume = Volume
prop-host = Host
prop-offset = Position on Wall
prop-sill = Sill Height
prop-flip-facing = Flip Facing
prop-flip-hand = Flip Hand
prop-overhang = Overhang
prop-roof-shape = Shape
prop-slope = Slope
prop-direction = Direction
prop-material = Material
prop-cut-height = Cut Plane Height
prop-depth = Far Clip Offset
prop-flip-section = Flip Direction
prop-sheet-number = Sheet Number
prop-sheet-size = Paper Size
prop-sheet-views = Views on Sheet
prop-add-view = Add View
prop-no-views = All views are already on sheets
prop-remove-from-sheet = Remove from Sheet
prop-center-x = Center X
prop-center-y = Center Y
prop-legend-types = Displayed Types
roof-flat = Flat
roof-shed = Shed

# Schedule columns
col-mark = Mark
col-type = Type
col-width = Width
col-height = Height
col-sill = Sill Height
col-level = Level
col-host = Wall
col-comments = Comments
col-length = Length (m)
col-thickness = Thickness (m)
col-area = Area (m²)
col-perimeter = Perimeter (m)
col-volume = Volume (m³)
schedule-total = Total: { $count }
schedule-hint = Click a row to select the element. Schedules update automatically.

# Title block
title-project = PROJECT
title-sheet = SHEET
title-client = CLIENT
title-number = NUMBER
title-project-number = Project no.
title-author = Author:

# Dialogs
dlg-project-info = Project Information
dlg-levels = Levels
dlg-materials = Materials
dlg-type = Type Properties
dlg-unsaved = Unsaved Changes
dlg-unsaved-text = The project has unsaved changes. Do you want to save them?
dlg-error = Error
dlg-about = About
info-name = Project Name
info-number = Number
info-client = Client
info-address = Address
info-author = Author
level-elevation = Elevation
level-add = + Add Level
material-add = + Add Material
type-layers = Layers (exterior - interior)
type-total-thickness = Total thickness
type-add-layer = + Layer
type-duplicate = Duplicate Type
about-version = Version
about-text = Free and open source Building Information Modeling (BIM) software.
about-license = License GPL-3.0-or-later.
about-icon-credit = Application icon by Magnific – Flaticon.
file-filter = ConstructCAD Project
file-filter-ifc = IFC File

# Default names
default-section-name = Section { $n }
default-3d-name = 3D { $n }
default-legend-name = Legend { $n }
default-schedule-name = { $category } Schedule
default-sheet-name = New Sheet
default-level-name = Level { $n }
default-material-name = Material { $n }
copy-of = { $name } (copy)

# Transactions (undo history)
tx-create-wall = Create Wall
tx-create-door = Place Door
tx-create-window = Place Window
tx-create-floor = Create Floor
tx-create-ceiling = Create Ceiling
tx-create-roof = Create Roof
tx-create-section = Create Section
tx-delete = Delete
tx-move = Move
tx-copy = Copy
tx-modify = Modify
tx-change-type = Change Type
tx-modify-type = Modify Type
tx-duplicate-type = Duplicate Type
tx-modify-view = Modify View
tx-new-view = New View
tx-new-sheet = New Sheet
tx-duplicate-view = Duplicate View
tx-delete-view = Delete View
tx-rename-view = Rename View
tx-place-view = Place View on Sheet
tx-move-viewport = Move Viewport
tx-remove-viewport = Remove Viewport
tx-add-level = Add Level
tx-delete-level = Delete Level
tx-modify-level = Modify Level
tx-modify-material = Modify Material
tx-project-info = Project Information

# Language
settings-language = Language
msg-language-changed = Language changed. New projects will use names in this language.

# File errors
err-io = read/write error ({ $detail })
err-parse = the file is not a valid ConstructCAD project ({ $detail })
err-serialize = failed to write the file ({ $detail })
err-version = the file uses format version { $version }; this version of the program supports up to { $max }

# New project template (level, material, type and view names)
tpl-project-name = New Project
tpl-level-ground = Ground Floor
tpl-level-first = Level 1
tpl-mat-masonry = Masonry
tpl-mat-plaster = Plaster
tpl-mat-concrete = Concrete
tpl-mat-wood = Wood
tpl-mat-glass = Glass
tpl-mat-gypsum = Gypsum
tpl-mat-roof-tile = Clay Roof Tile
tpl-mat-screed = Screed
tpl-type-masonry-15 = Masonry 15 cm
tpl-type-masonry-25 = Masonry 25 cm
tpl-type-drywall-10 = Gypsum Partition 10 cm
tpl-type-door-80 = Door 80x210
tpl-type-door-90 = Door 90x210
tpl-type-window-120 = Window 120x120
tpl-type-window-150 = Window 150x100
tpl-type-slab-12 = Slab 12 cm
tpl-type-ceiling-gypsum = Gypsum Ceiling
tpl-type-roof-tile = Clay Tile Roof
tpl-elev-north = North
tpl-elev-south = South
tpl-elev-east = East
tpl-elev-west = West
tpl-legend = Type Legend
tpl-schedule-doors = Door Schedule
tpl-schedule-windows = Window Schedule
tpl-schedule-walls = Wall Schedule
tpl-sheet-plans = Plans

# 3D compass
axis-east = E
axis-north = N
