# Interface reorganization: ribbon, contextual tab, panels

The user asked (2026-11-25) for "um estudo completo de UI, a usabilidade é
muito espartana, os botões estão espalhados e misturados". The study was
made on `main` at 54239e16, with `examples/casa_simples.ccad` open at
1600 × 950 in Portuguese, and published as a page for the user. This file
keeps its findings and the plan. Phase 1 is being implemented first.

The window's skeleton is right (quick access toolbar, ribbon, options bar,
browser and properties docked at the sides, view tabs, status bar). What
is wrong is how the commands are spread over it.

## Findings

- **P1. Modify holds generic and per-category edits together.** 41
  buttons in 9 groups (`ribbon.rs::modify_tab`). Move, copy and rotate apply
  to anything; Join/Unjoin Roof, Attach/Detach Top, Flip Walls, Edit/Reset
  Wall Profile, Edit Boundary, Edit Profile, Edit Title Block and Demolish
  apply to one category each. With a grid selected about 12 buttons are grey
  and none is about grids, while Align Grid Ends lives in Architecture and
  Structure. The tab overflows even at 1600 px.
- **P2. No right-click menu on model elements.** `tools.rs` opens
  `context_menu` only for tags, fitting labels and level heads.
- **P3. Commands repeated across tabs.** Select (Architecture, Structure,
  MEP, Annotate, Modify); Grid and Align Grid Ends (Architecture,
  Structure); Floor (Architecture, Structure/Concrete); Profiles
  (Structure, Manage); Pipe Catalog (MEP, Manage); View Templates and
  Filters (View, Manage); Export IFC/DXF… (File, Insert); Load Component
  and Component Library (Architecture, Insert). The quick access toolbar,
  context menus and the palette may repeat commands; tabs should not.
- **P4. Tabs follow no single rule.** Insert holds exports; View mixes
  creating views with Browser Organization (a setting) and New Title Block
  (making a reusable type); Manage mixes project settings, catalogs, macros
  and plugins; Massing & Site has 7 commands while Site Location sits in
  Manage; Insert sits between Massing & Site and Modify.
- **P5. Unlabeled small icons.** `stacked`/`small_button` draw only the
  icon (label in the tooltip). Structure's Reinforcement, Foundation,
  Analytical and Documentation groups, MEP's Documentation (6), View's
  Hide/Isolate (8), Modify (15). Generic glyphs (move, copy, rotate) work
  without text; "Framing plan" or "Analytical export" do not.
- **P6. Structure's material switch is hidden state.** A column of
  selectable labels that swaps the tab's groups and persists in the
  preferences.
- **P7. Too many grey buttons.** Grey is right for a passing lack (nothing
  to paste); a command that does not apply to the selection should not show.
- **P8. Narrow windows scroll** the ribbon instead of compacting groups.
- **P9. Panels.** Property labels are clipped with no ellipsis ("Altura em
  relação ao"); the properties panel ends with a Delete button; the options
  bar repeats a long hint for the Select tool; the status bar keeps
  "Aberto: …" forever; clicking a wall that lies on a grid picks the grid,
  and nothing tells about Tab cycling; the quick access toolbar is fixed.

## Principles

1. One home per command in the ribbon tabs (QAT, context menu and palette
   may repeat).
2. Fixed tabs create; a contextual tab edits what is selected.
3. Modify keeps the generic verbs only.
4. What does not apply is hidden; what cannot run yet is grey.
5. Three button sizes: large (main commands of a group), medium (icon and
   label on one line, secondary commands), small icon-only (universal
   glyph families only).
6. A group's settings open from a launcher (↘) on its caption.
7. A narrow ribbon compacts (large → medium → group menu) before scrolling.

## Target layout

Tab order: File · Architecture · Structure · MEP · Massing & Site ·
Annotate · View · Insert · Manage · Plugins · Modify (Modify | <Category>
with a selection).

| Tab | Groups | Change |
|---|---|---|
| File | Backstage (phase 3): New, Open, Recent, Save, Save As, Export ▸, Close, Preferences, About, Updates | Only home of exports |
| Architecture | Build: Wall, Door, Window, Niche, Curtain Grid, Component · Floors & roofs: Floor, Ceiling, Roof, Shaft · Circulation · Room · Model: Sweep · Datum: Grid, Align Grid Ends | Select and the component make/edit/load/library buttons leave (to Insert); Align Grid Ends goes contextual in phase 2 |
| Structure | System: segmented Concrete/Steel/Timber · Structure · Framing · Connections · Foundation · Reinforcement · Analytical · Documentation (medium labelled buttons) | Profiles → Manage; Grid, Align Grid Ends, Select leave; Floor stays (structural slab, accepted exception) |
| MEP | Plumbing (↘ Pipe Catalog) · Electrical · Communications · Lightning · Systems (+ Sizing) · Documentation (medium) | Pipe Catalog button → Manage (launcher in phase 3); Select leaves |
| Massing & Site | Massing · Site: Topography, Pad, Site Location | Site Location from Manage |
| Annotate | Dimension · Detail (↘ Line Styles) · Text · Level marks · Tags (↘ Tag Types) | Create Detail Component → Insert; Add Leader → contextual (phase 2); Select leaves |
| View | Create · Documentation: Legend, Drafting View, Schedule, Sheet · Graphics: V/G, Filters, Templates, Sun, Render · Hide/Isolate · Windows | Browser Organization → Manage; New Title Block → Insert; Zoom Fit leaves (view bar, ZE) |
| Insert | Import · Library: Load Component, Component Library, New Component, Edit Component, New Detail Component, New Title Block | Exports leave |
| Manage | Project: Info, Units, Levels, Materials, Fill Patterns, Profiles, Pipe Catalog · Browser: Organization · Phasing · Design Options · Macros · Plugins | Site Location → Massing & Site; View Templates and Filters stay in View only |
| Modify | Selection · Clipboard · Modify · Measure · Geometry (join, paint) · Group · Phasing; category groups move to the contextual tab in phase 2 | 41 → ~28 buttons after phase 2 |

### Contextual tab and context menu (phase 2)

A single source, `UiCommand::contextual(category) -> &[UiCommand]`, feeds
both the contextual tab's highlighted groups and the canvas context menu:

| Selection | Contextual commands |
|---|---|
| Walls | Flip, Attach Top, Detach Top, Edit Wall Profile, Reset Wall Profile; wall reinforcement, wall framing |
| Roofs | Edit Boundary, Join Roof, Unjoin Roof |
| Floors, ceilings, slabs | Edit Boundary, add opening, slab reinforcement |
| Columns, beams | Typical Rebar, Connection ▾ |
| Grids | Align Grid Ends |
| Sweep | Edit Profile |
| Component | Edit Component |
| Text | Add Leader |
| Sheet (active view) | Edit Title Block |
| Several categories | Filter selection |

With a selection the Modify tab is titled "Modify | Walls" and activated
(a preference turns this off); clearing the selection returns to the tab
shown before. The right-click menu on an element: Cancel, Repeat last,
Select All Instances ▸, Create Similar, Edit Type…, the contextual commands,
Hide in View ▸, Isolate ▸, Override Graphics…, Delete. On empty canvas:
Cancel, Repeat last, Zoom to Fit, View Properties, Visibility/Graphics.

## Phases

1. **Tidy up** (ribbon.rs, i18n, snapshots, docs): remove the P3 repeats
   and Select from four tabs; exports out of Insert; new tab order; Site
   Location to Massing & Site; medium labelled buttons for the stacks of
   Structure, MEP, View and Architecture; a real segmented control for the
   structural material; ellipsis and tooltip on property labels; no Delete
   button in the properties panel; update `docs/interface*.md` and the
   snapshots.
2. **Contextual tab and right-click menu** (largest gain), with
   `ui_tests.rs` checks (a wall shows Flip; a grid does not show Attach Top).
3. **Backstage File, customizable QAT, compacting ribbon, group launchers.**
4. **Selection**: model before datum in `Drawing2D::hits_at`, a "Tab: next
   (2 of 3)" hint, stronger pre-highlight, Filter selection from the status
   bar, search in properties.

Engine commands do not change in any phase; only where the interface shows
them. Scripts, MCP and `ccad` are unaffected.

## Status

- Phase 1: PR #511.
- Phase 2: PR #512.
- Phase 3: branch `feat/ui-reorg-phase-3` (File page, customizable quick access toolbar, compaction, launchers).
- Phase 2 notes: Not done: *Repeat last* and *Cancel* in the context menu (the app keeps no last command), and a preference to turn off the automatic switch to Modify (that switch predates this work).

## Measuring

Five CAD/BIM users before and after each phase, timing these tasks and
counting hovers needed to identify a button: attach a wall top to the
roof; align grid ends; create the Ground floor framing plan; export IFC;
change a detail line's style; hide every door in a view. Goal: no task
needs a hover to identify a button, and no edit is reached through a tab
other than Modify.
