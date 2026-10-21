# The interface: ribbon, commands and palette

*Versão em português: [interface.pt-BR.md](interface.pt-BR.md).*

ConstructCAD's window follows the layout desktop CAD and BIM users expect. The **quick access toolbar** sits at the top left. Below it is the **ribbon**, a row of tabs with groups of buttons. The **options bar** for the active tool comes next. The **project browser** and **properties** are docked on the left, the **views** fill the tabs in the middle, and the **status bar** is at the bottom.

## Commands and their tooltips

Every button in the ribbon and the quick access toolbar runs an *interface command*. So does every two-letter shortcut and every entry of the command palette. Each command defines, in one place:

- its **name** and **icon**;
- its **shortcut**: a Ctrl chord, or two letters typed while a view has the focus;
- a **description** of what it does;
- the **engine command** that makes the change. Scripts, the `ccad` command line and the MCP server call this same command (see the [automation guide](automation.md));
- whether it **can run now**, and why not.

Hover over a button to see its tooltip:

```
Move  [MV]
Moves the selected elements from a base point to a target.
Unavailable: Select the elements before using the tool
Engine command: elements_move
```

Buttons that cannot run are greyed out, and their tooltip gives the reason. Typical reasons are "nothing to undo", "select the elements first", "open a sheet to export it", "select one text note" and "copy elements first". A shortcut typed when its command cannot run shows the same reason in the status bar.

## Icons and badges

Icons are drawn in two tones: the strokes in the text color and the soft fills in the theme's accent tint. A small badge in the bottom-right corner marks what kind of action a command is:

| Badge | Meaning | Examples |
|---|---|---|
| green **+** | creates something new | new floor or ceiling plan, 3D view, legend, schedule, sheet, new component |
| blue arrow down | brings data in from a file | Import IFC, Load component |
| blue arrow up | writes data out to a file | Export PDF, Export SVG |
| red **−** | removes something | Delete |

The selected tool is highlighted in the accent color. Unavailable commands are greyed out, badges included.

## Collapsing the ribbon

To give the views more room, collapse the ribbon to its tab titles. Any of these collapses it or expands it again:

- **Ctrl+F1**;
- a **double click** on any tab;
- the **arrow** at the right end of the tab row.

While the ribbon is collapsed, a click on a tab shows that tab's panel over the views. The panel closes when a command runs, when you press Esc or click elsewhere, or when you click the same tab again. ConstructCAD remembers between sessions whether the ribbon was collapsed.

## Narrow windows

When the window is narrower than a tab's groups, the groups scroll sideways instead of being cut off. Arrows appear at both ends of the row: click them to scroll, or use the mouse wheel. Each arrow is greyed out once that end is reached.

## Command palette

**Ctrl+Shift+P** opens the command palette, as does the magnifier at the right end of the tab row. Type part of what you are looking for. The search ignores case and accents ("niveis" finds "Níveis"), and every word you type must match. The palette finds:

- **interface commands**, by name, description or shortcut (typing `WA` finds *Wall*);
- **plans to create** for each level ("Floor plan: Ground floor") and **schedules** for each category;
- **views to open** ("Open view: …");
- **engine commands** (`level_create`, `elements_list`, `type_set`…), shown in a monospaced font.

Use ↑ and ↓ to choose and Enter to run, or click an entry. Esc closes the palette. An unavailable command stays in the list, greyed out, with its reason in place of the description.

### Running engine commands

Choosing an engine command opens a form with its description and its arguments in JSON, already filled with the required fields:

```json
{
  "elevation": 9.0,
  "name": "Roof deck"
}
```

**Run** (or Ctrl+Enter) executes the command in the open project and shows its result or error message. An edit is one undo step and is recorded while a macro is recording, like any other change. **Back** (or Esc) returns to the search. The arguments are the same as in scripts and in the [command reference](commands.md). Lengths are in meters, angles in degrees, points are `[x, y]`, and levels, types, views and materials can be named by id or by name.

The engine's file commands (`project_open`, `project_save`, `ifc_import`, `ifc_export`, `sheet_export`…) are not offered here. Their interface commands in the *File* tab and the palette handle files through the usual dialogs.

## Keyboard

| Key | Action |
|---|---|
| Ctrl+Shift+P | Command palette |
| Ctrl+F1 | Collapse / expand the ribbon |
| Ctrl+N / Ctrl+O / Ctrl+S / Ctrl+Shift+S / Ctrl+W | New / Open / Save / Save as / Close project |
| Ctrl+Z / Ctrl+Y | Undo / Redo |
| Ctrl+X / Ctrl+C / Ctrl+V | Cut / Copy / Paste |
| Delete | Delete the selection |
| Esc | Cancel the operation, leave the tool, clear the selection |
| Space | Rotate or flip the selection or the element being placed |

Two-letter keys work while a view has the focus and no text field is being edited. The README lists them all. Each tooltip shows its own shortcut as well.

## Accessibility

The ribbon buttons, tab titles, scroll arrows, palette entries, browser rows, home screen cards, dialog close buttons, property sections and the view cube's home button are drawn by ConstructCAD itself. They report their role (button, selectable item, collapsible header), their name and their state (enabled, selected, expanded) to the operating system's accessibility interface, so screen readers can announce them.

## For developers

### Adding a command to the interface

1. Make the change an engine command first (see *For developers* in the [automation guide](automation.md)).
2. Add a variant to `UiCommand` in `crates/cc_app/src/commands.rs`. Then fill in its `icon`, `label`, `description` (an i18n key `cmd-desc-…` in every `.ftl` file), `shortcut` (if any), `engine` (the engine command's name), `badge`, `unavailable` (a localized reason for each state in which it cannot run), `selected` (for tools) and `run`. Add it to `UiCommand::all`.
3. Place it in a ribbon group with `command_button` (large) or in a family of small buttons with `stacks` (`crates/cc_app/src/ribbon.rs`).

The tooltip, enabling, the palette entry and a two-letter shortcut all follow from the command. Tests check that each engine name exists in the catalog, that shortcuts are unique, and that every command appears in the ribbon with an accessible name.

### Icons

Icons are SVG files in `assets/icons/ui` on a 24-unit grid. They use white strokes (`stroke-width` 1.75, round caps and joins) and soft fills written as `fill="#fff" fill-opacity=".2"`. `icons.rs` splits each icon into two layers by those attributes: the strokes and solid marks form the ink, and the soft fills form the tint. Each layer is rasterized with resvg at the exact pixel size and painted in the theme's colors (`icons::Tones`). Add the new icon to `Icon`, `Icon::ALL` and `Icon::source`. `ICON_SHEET=sheet.png cargo test -p cc_app icon_sheet` writes every icon at several sizes for review.

### Theme

Colors are tokens of `theme::Palette`: `PALETTE` (light) and `DARK`, with a test of their contrast. Paint with `theme::palette(ctx)` so a dark theme can be switched on later.

### Interface tests

`crates/cc_app/src/ui_tests.rs` runs the ribbon and the palette headlessly with [egui_kittest](https://crates.io/crates/egui_kittest). The tests click and type through the accessibility tree, so custom widgets must call `Response::widget_info`. Screenshot references live in `crates/cc_app/tests/snapshots`. They cover each ribbon tab, the narrow and collapsed ribbon, a rich tooltip, the palette, and an icon gallery in both themes. GPUs render slightly differently, so the screenshot comparisons run on request:

```sh
CC_UI_SNAPSHOTS=1 cargo test -p cc_app snapshots     # compare with the references
UPDATE_SNAPSHOTS=1 cargo test -p cc_app snapshots    # rewrite them after an intended change
```

Review the new images before committing them. On a mismatch, kittest writes `*.new.png` and `*.diff.png` next to the references; Git ignores those files.
