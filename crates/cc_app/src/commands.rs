//! Interface commands: every ribbon, quick access and palette entry is a
//! [`UiCommand`], so its label, shortcut, description, the engine command it
//! runs and whether it is available now (and why not) are defined once and
//! read by every place that shows it.

use crate::actions;
use crate::icons::{Badge, Icon};
use crate::ribbon::{category_icon, dimension_icon};
use crate::state::{AppState, PendingAction};
use crate::t;
use crate::tools::{DimensionMode, Tool};
use cc_core::{Category, ViewCategory};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum UiCommand {
    Home,
    New,
    Open,
    Save,
    SaveAs,
    CloseProject,
    ImportIfc,
    ExportIfc,
    ExportPdf,
    ExportSvg,
    About,
    Undo,
    Redo,
    Default3D,
    /// The placement tool of a model category.
    Place(Category),
    NewComponent,
    LoadComponent,
    Select,
    Dimension(DimensionMode),
    Text,
    AddLeader,
    Move,
    CopyElements,
    Rotate,
    MirrorPick,
    MirrorDraw,
    Array,
    Align,
    Offset,
    TrimCorner,
    TrimExtend,
    Split,
    ToggleJoin,
    Pin,
    Unpin,
    Delete,
    Paste,
    Cut,
    ClipboardCopy,
    MatchType,
    FloorPlan,
    CeilingPlan,
    New3D,
    Section,
    Elevations,
    Legend,
    Schedule,
    Sheet,
    ZoomFit,
    ProjectInfo,
    Units,
    Levels,
    Materials,
    MacroRecord,
    MacroRun,
    ToggleRibbon,
}

/// Model categories placed by a tool of their own (dimensions and text have their own commands).
const PLACED: [Category; 13] = [
    Category::Wall,
    Category::Door,
    Category::Window,
    Category::Floor,
    Category::Ceiling,
    Category::Roof,
    Category::Stair,
    Category::Railing,
    Category::Room,
    Category::Component,
    Category::Grid,
    Category::Column,
    Category::Beam,
];

impl UiCommand {
    /// Every command, in ribbon order.
    pub fn all() -> Vec<UiCommand> {
        use UiCommand::*;
        let mut all = vec![
            Home,
            New,
            Open,
            Save,
            SaveAs,
            CloseProject,
            ImportIfc,
            ExportIfc,
            ExportPdf,
            ExportSvg,
            About,
            Undo,
            Redo,
            Default3D,
        ];
        all.extend(PLACED.map(Place));
        all.extend([NewComponent, LoadComponent, Select]);
        all.extend(DimensionMode::ALL.map(Dimension));
        all.extend([
            Text,
            AddLeader,
            Move,
            CopyElements,
            Rotate,
            MirrorPick,
            MirrorDraw,
            Array,
            Align,
            Offset,
            TrimCorner,
            TrimExtend,
            Split,
            ToggleJoin,
            Pin,
            Unpin,
            Delete,
            Paste,
            Cut,
            ClipboardCopy,
            MatchType,
            FloorPlan,
            CeilingPlan,
            New3D,
            Section,
            Elevations,
            Legend,
            Schedule,
            Sheet,
            ZoomFit,
            ProjectInfo,
            Units,
            Levels,
            Materials,
            MacroRecord,
            MacroRun,
            ToggleRibbon,
        ]);
        all
    }

    /// The command a two-letter key sequence runs.
    pub fn from_keys(keys: &str) -> Option<UiCommand> {
        Self::all().into_iter().find(|c| c.shortcut() == Some(keys))
    }

    pub fn icon(self) -> Icon {
        use UiCommand::*;
        match self {
            Home => Icon::Home,
            New => Icon::New,
            Open | ImportIfc => Icon::Open,
            Save => Icon::Save,
            SaveAs => Icon::SaveAs,
            CloseProject => Icon::CloseProject,
            ExportIfc => Icon::Export,
            ExportPdf | ExportSvg | Sheet => Icon::Sheet,
            About => Icon::About,
            Undo => Icon::Undo,
            Redo => Icon::Redo,
            Default3D | New3D => Icon::View3D,
            Place(category) => category_icon(category),
            NewComponent | LoadComponent => Icon::Component,
            Select => Icon::Select,
            Dimension(mode) => dimension_icon(mode),
            Text => Icon::Text,
            AddLeader => Icon::Leader,
            Move => Icon::Move,
            CopyElements | ClipboardCopy => Icon::Copy,
            Rotate => Icon::Rotate,
            MirrorPick => Icon::MirrorPick,
            MirrorDraw => Icon::MirrorDraw,
            Array => Icon::Array,
            Align => Icon::Align,
            Offset => Icon::Offset,
            TrimCorner => Icon::TrimCorner,
            TrimExtend => Icon::TrimExtend,
            Split => Icon::Split,
            ToggleJoin => Icon::Join,
            Pin => Icon::Pin,
            Unpin => Icon::Unpin,
            Delete => Icon::Delete,
            Paste => Icon::Paste,
            Cut => Icon::Cut,
            MatchType => Icon::MatchType,
            FloorPlan => Icon::FloorPlan,
            CeilingPlan => Icon::CeilingPlan,
            Section => Icon::Section,
            Elevations => Icon::Elevation,
            Legend => Icon::Legend,
            Schedule => Icon::Schedule,
            ZoomFit => Icon::ZoomFit,
            ProjectInfo => Icon::ProjectInfo,
            Units => Icon::Dimension,
            Levels => Icon::Levels,
            Materials => Icon::Materials,
            MacroRecord => Icon::MacroRecord,
            MacroRun => Icon::MacroRun,
            ToggleRibbon => Icon::ChevronUp,
        }
    }

    /// The mark of the kind of action: creating, importing, exporting.
    pub fn badge(self) -> Option<Badge> {
        use UiCommand::*;
        match self {
            NewComponent | FloorPlan | CeilingPlan | New3D | Legend | Schedule | Sheet => Some(Badge::New),
            ImportIfc | LoadComponent => Some(Badge::Import),
            ExportPdf | ExportSvg => Some(Badge::Export),
            Delete => Some(Badge::Remove),
            _ => None,
        }
    }

    /// Button label.
    pub fn label(self, state: &AppState) -> String {
        use UiCommand::*;
        match self {
            Home => t!("action-home"),
            New => t!("action-new"),
            Open => t!("action-open"),
            Save => t!("action-save"),
            SaveAs => t!("action-save-as"),
            CloseProject => t!("action-close-project"),
            ImportIfc => t!("action-import-ifc"),
            ExportIfc => t!("action-export-ifc"),
            ExportPdf => t!("action-export-pdf"),
            ExportSvg => t!("action-export-svg"),
            About => t!("dlg-about"),
            Undo => t!("action-undo"),
            Redo => t!("action-redo"),
            Default3D => t!("action-default-3d"),
            Place(category) => place_label(category),
            NewComponent => t!("action-new-component"),
            LoadComponent => t!("action-load-component"),
            Select => t!("tool-select"),
            Dimension(mode) => crate::i18n::tr(mode.key()),
            Text => t!("tool-text"),
            AddLeader => t!("tool-add-leader"),
            Move => t!("tool-move"),
            CopyElements => t!("tool-copy"),
            Rotate => t!("tool-rotate"),
            MirrorPick => t!("tool-mirror-pick"),
            MirrorDraw => t!("tool-mirror-draw"),
            Array => t!("tool-array"),
            Align => t!("tool-align"),
            Offset => t!("tool-offset"),
            TrimCorner => t!("tool-trim-corner"),
            TrimExtend => t!("tool-trim-extend"),
            Split => t!("tool-split"),
            ToggleJoin => t!("tool-toggle-join"),
            Pin => t!("action-pin"),
            Unpin => t!("action-unpin"),
            Delete => t!("action-delete"),
            Paste => t!("action-paste"),
            Cut => t!("action-cut"),
            ClipboardCopy => t!("action-copy"),
            MatchType => t!("tool-match-type"),
            FloorPlan => t!("view-new-floor-plan"),
            CeilingPlan => t!("view-new-ceiling-plan"),
            New3D => t!("view-new-3d"),
            Section => t!("tool-section"),
            Elevations => t!("view-elevations"),
            Legend => t!("view-new-legend"),
            Schedule => t!("view-new-schedule"),
            Sheet => t!("view-new-sheet"),
            ZoomFit => t!("action-zoom-fit"),
            ProjectInfo => t!("dlg-project-info"),
            Units => t!("dlg-units"),
            Levels => t!("dlg-levels"),
            Materials => t!("dlg-materials"),
            MacroRecord if state.session.is_recording() => t!("action-macro-stop"),
            MacroRecord => t!("action-macro-record"),
            MacroRun => t!("action-macro-run"),
            ToggleRibbon if state.ribbon_collapsed => t!("action-ribbon-expand"),
            ToggleRibbon => t!("action-ribbon-collapse"),
        }
    }

    /// Tooltip title: the label, plus what undo and redo would change.
    pub fn title(self, state: &AppState) -> String {
        let doc = &state.session.doc;
        let step = match self {
            UiCommand::Undo => doc.undo_name(),
            UiCommand::Redo => doc.redo_name(),
            _ => None,
        };
        match step {
            Some(name) => format!("{}: {name}", self.label(state)),
            None => self.label(state),
        }
    }

    /// What the command does, one or two sentences.
    pub fn description(self) -> String {
        use UiCommand::*;
        match self {
            Home => t!("cmd-desc-home"),
            New => t!("cmd-desc-new"),
            Open => t!("cmd-desc-open"),
            Save => t!("cmd-desc-save"),
            SaveAs => t!("cmd-desc-save-as"),
            CloseProject => t!("cmd-desc-close-project"),
            ImportIfc => t!("cmd-desc-import-ifc"),
            ExportIfc => t!("cmd-desc-export-ifc"),
            ExportPdf => t!("cmd-desc-export-pdf"),
            ExportSvg => t!("cmd-desc-export-svg"),
            About => t!("cmd-desc-about"),
            Undo => t!("cmd-desc-undo"),
            Redo => t!("cmd-desc-redo"),
            Default3D => t!("cmd-desc-default-3d"),
            Place(category) => place_description(category),
            NewComponent => t!("cmd-desc-new-component"),
            LoadComponent => t!("cmd-desc-load-component"),
            Select => t!("cmd-desc-select"),
            Dimension(DimensionMode::Aligned) => t!("cmd-desc-dim-aligned"),
            Dimension(DimensionMode::Linear) => t!("cmd-desc-dim-linear"),
            Dimension(DimensionMode::Angular) => t!("cmd-desc-dim-angular"),
            Dimension(DimensionMode::Radial) => t!("cmd-desc-dim-radial"),
            Dimension(DimensionMode::Diameter) => t!("cmd-desc-dim-diameter"),
            Text => t!("cmd-desc-text"),
            AddLeader => t!("hint-add-leader-button"),
            Move => t!("cmd-desc-move"),
            CopyElements => t!("cmd-desc-copy-elements"),
            Rotate => t!("cmd-desc-rotate"),
            MirrorPick => t!("cmd-desc-mirror-pick"),
            MirrorDraw => t!("cmd-desc-mirror-draw"),
            Array => t!("cmd-desc-array"),
            Align => t!("cmd-desc-align"),
            Offset => t!("cmd-desc-offset"),
            TrimCorner => t!("cmd-desc-trim-corner"),
            TrimExtend => t!("cmd-desc-trim-extend"),
            Split => t!("cmd-desc-split"),
            ToggleJoin => t!("cmd-desc-toggle-join"),
            Pin => t!("cmd-desc-pin"),
            Unpin => t!("cmd-desc-unpin"),
            Delete => t!("cmd-desc-delete"),
            Paste => t!("cmd-desc-paste"),
            Cut => t!("cmd-desc-cut"),
            ClipboardCopy => t!("cmd-desc-copy"),
            MatchType => t!("cmd-desc-match-type"),
            FloorPlan => t!("cmd-desc-floor-plan"),
            CeilingPlan => t!("cmd-desc-ceiling-plan"),
            New3D => t!("cmd-desc-new-3d"),
            Section => t!("cmd-desc-section"),
            Elevations => t!("cmd-desc-elevations"),
            Legend => t!("cmd-desc-legend"),
            Schedule => t!("cmd-desc-schedule"),
            Sheet => t!("cmd-desc-sheet"),
            ZoomFit => t!("cmd-desc-zoom-fit"),
            ProjectInfo => t!("cmd-desc-project-info"),
            Units => t!("cmd-desc-units"),
            Levels => t!("cmd-desc-levels"),
            Materials => t!("cmd-desc-materials"),
            MacroRecord => t!("hint-macro-record"),
            MacroRun => t!("hint-macro-run"),
            ToggleRibbon => t!("cmd-desc-toggle-ribbon"),
        }
    }

    /// Keyboard shortcut: a Ctrl chord or a two-letter sequence typed in a view.
    pub fn shortcut(self) -> Option<&'static str> {
        use UiCommand::*;
        Some(match self {
            New => "Ctrl+N",
            Open => "Ctrl+O",
            Save => "Ctrl+S",
            SaveAs => "Ctrl+Shift+S",
            CloseProject => "Ctrl+W",
            Undo => "Ctrl+Z",
            Redo => "Ctrl+Y",
            Place(Category::Wall) => "WA",
            Place(Category::Door) => "DR",
            Place(Category::Window) => "WN",
            Place(Category::Floor) => "SB",
            Place(Category::Ceiling) => "FO",
            Place(Category::Roof) => "TE",
            Place(Category::Stair) => "ST",
            Place(Category::Railing) => "RA",
            Place(Category::Room) => "RM",
            Place(Category::Component) => "CM",
            Place(Category::Grid) => "GR",
            Place(Category::Column) => "CL",
            Place(Category::Beam) => "BM",
            Select => "MD",
            Dimension(DimensionMode::Aligned) => "DI",
            Text => "TT",
            Move => "MV",
            CopyElements => "CO",
            Rotate => "RO",
            MirrorPick => "MM",
            MirrorDraw => "DM",
            Array => "AR",
            Align => "AL",
            Offset => "OF",
            TrimCorner => "TR",
            TrimExtend => "TX",
            Split => "SL",
            Pin => "PN",
            Unpin => "UP",
            Delete => "Delete",
            Paste => "Ctrl+V",
            Cut => "Ctrl+X",
            ClipboardCopy => "Ctrl+C",
            MatchType => "MA",
            Section => "CT",
            ZoomFit => "ZE",
            ToggleRibbon => "Ctrl+F1",
            _ => return None,
        })
    }

    /// The engine command that makes the change, as scripts and the MCP server call it.
    pub fn engine(self) -> Option<&'static str> {
        use UiCommand::*;
        Some(match self {
            New => "project_new",
            Open => "project_open",
            Save | SaveAs => "project_save",
            ImportIfc => "ifc_import",
            ExportIfc => "ifc_export",
            ExportPdf | ExportSvg => "sheet_export",
            Undo => "undo",
            Redo => "redo",
            Place(Category::Wall) => "wall_create",
            Place(Category::Door) => "door_create",
            Place(Category::Window) => "window_create",
            Place(Category::Floor) => "floor_create",
            Place(Category::Ceiling) => "ceiling_create",
            Place(Category::Roof) => "roof_create",
            Place(Category::Stair) => "stair_create",
            Place(Category::Railing) => "railing_create",
            Place(Category::Room) => "room_create",
            Place(Category::Grid) => "grid_create",
            Place(Category::Column) => "column_create",
            Place(Category::Beam) => "beam_create",
            Place(_) => "component_place",
            NewComponent => "component_create",
            LoadComponent => "component_load",
            Dimension(_) => "dimension_create",
            Text => "text_create",
            AddLeader => "text_add_leader",
            Move => "elements_move",
            CopyElements | Array => "elements_copy",
            Rotate => "elements_rotate",
            MirrorPick | MirrorDraw => "elements_mirror",
            Align => "element_align",
            Offset => "element_offset",
            TrimCorner => "trim_corner",
            TrimExtend => "trim_extend",
            Split => "element_split",
            ToggleJoin => "wall_join",
            Pin | Unpin => "elements_pin",
            Delete | Cut => "elements_delete",
            Paste => "elements_paste",
            ClipboardCopy => "elements_export",
            MatchType => "elements_set_type",
            FloorPlan | CeilingPlan | New3D | Section | Legend | Schedule | Sheet => "view_create",
            ProjectInfo => "project_info_set",
            Units => "units_set",
            Levels => "level_set",
            Materials => "material_set",
            _ => return None,
        })
    }

    /// Why the command cannot run now, or `None` when it can.
    pub fn unavailable(self, state: &AppState) -> Option<String> {
        use UiCommand::*;
        let project_open = state.project_open && !state.show_home;
        match self {
            Home | New | Open | ImportIfc | About | ToggleRibbon => return None,
            _ if !project_open => return Some(t!("why-no-project")),
            _ => {}
        }
        let has_sel = !state.selection.is_empty();
        let project = state.project();
        match self {
            Undo if !state.session.doc.can_undo() => Some(t!("why-nothing-undo")),
            Redo if !state.session.doc.can_redo() => Some(t!("why-nothing-redo")),
            ExportPdf | ExportSvg if actions::active_sheet(state).is_none() => Some(t!("hint-export-sheet")),
            Place(category) if Tool::for_category(project, category).is_none() => Some(t!("msg-no-type")),
            Dimension(_) if Tool::for_category(project, Category::Dimension).is_none() => Some(t!("msg-no-type")),
            Text if Tool::for_category(project, Category::Text).is_none() => Some(t!("msg-no-type")),
            AddLeader if crate::ribbon::selected_text(state).is_none() && !self.selected(state) => {
                Some(t!("why-select-text"))
            }
            Move | CopyElements | Rotate | MirrorPick | MirrorDraw | Array if !has_sel && !self.selected(state) => {
                Some(t!("msg-select-first"))
            }
            Pin | Unpin | Delete | Cut | ClipboardCopy if !has_sel => Some(t!("msg-select-first")),
            Paste if state.clipboard.is_empty() => Some(t!("why-clipboard-empty")),
            FloorPlan | CeilingPlan if project.levels.is_empty() => Some(t!("why-no-levels")),
            Elevations if project.views_of(ViewCategory::Elevation).next().is_none() => Some(t!("why-no-elevations")),
            ZoomFit if state.active_view.is_none() => Some(t!("why-no-view")),
            MacroRun if state.session.is_recording() => Some(t!("why-recording")),
            _ => None,
        }
    }

    /// Whether the command's tool or mode is the active one.
    pub fn selected(self, state: &AppState) -> bool {
        use UiCommand::*;
        let tool = &state.tool;
        match self {
            Place(category) => tool.category() == Some(category) && *tool != Tool::Section,
            Select => *tool == Tool::Select,
            Dimension(mode) => matches!(tool, Tool::Dimension { mode: m, .. } if *m == mode),
            Text => matches!(tool, Tool::Text { .. }),
            AddLeader => matches!(tool, Tool::AddLeader { .. }),
            Move => *tool == Tool::Move,
            CopyElements => *tool == Tool::Copy,
            Rotate => *tool == Tool::Rotate,
            MirrorPick => *tool == Tool::Mirror { draw: false },
            MirrorDraw => *tool == Tool::Mirror { draw: true },
            Array => *tool == Tool::Array,
            Align => *tool == Tool::Align,
            Offset => *tool == Tool::Offset,
            TrimCorner => *tool == Tool::TrimCorner,
            TrimExtend => *tool == Tool::TrimExtend,
            Split => *tool == Tool::Split,
            ToggleJoin => *tool == Tool::ToggleJoin,
            Paste => *tool == Tool::Paste,
            MatchType => *tool == Tool::MatchType,
            Section => *tool == Tool::Section,
            MacroRecord => state.session.is_recording(),
            ToggleRibbon => state.ribbon_collapsed,
            _ => false,
        }
    }
}

fn place_label(category: Category) -> String {
    match category {
        Category::Wall => t!("tool-wall"),
        Category::Door => t!("tool-door"),
        Category::Window => t!("tool-window"),
        Category::Floor => t!("tool-floor"),
        Category::Ceiling => t!("tool-ceiling"),
        Category::Roof => t!("tool-roof"),
        Category::Stair => t!("tool-stair"),
        Category::Railing => t!("tool-railing"),
        Category::Room => t!("tool-room"),
        Category::Grid => t!("tool-grid"),
        Category::Column => t!("tool-column"),
        Category::Beam => t!("tool-beam"),
        Category::Dimension => t!("tool-dimension"),
        Category::Text => t!("tool-text"),
        Category::Component => t!("tool-component"),
    }
}

fn place_description(category: Category) -> String {
    match category {
        Category::Wall => t!("cmd-desc-wall"),
        Category::Door => t!("cmd-desc-door"),
        Category::Window => t!("cmd-desc-window"),
        Category::Floor => t!("cmd-desc-floor"),
        Category::Ceiling => t!("cmd-desc-ceiling"),
        Category::Roof => t!("cmd-desc-roof"),
        Category::Stair => t!("cmd-desc-stair"),
        Category::Railing => t!("cmd-desc-railing"),
        Category::Room => t!("cmd-desc-room"),
        Category::Grid => t!("cmd-desc-grid"),
        Category::Column => t!("cmd-desc-column"),
        Category::Beam => t!("cmd-desc-beam"),
        Category::Dimension => t!("cmd-desc-dim-aligned"),
        Category::Text => t!("cmd-desc-text"),
        Category::Component => t!("cmd-desc-component"),
    }
}

/// Run `command` if it is available; otherwise say why in the status bar.
pub fn invoke(state: &mut AppState, ctx: &egui::Context, command: UiCommand) {
    match command.unavailable(state) {
        Some(why) => state.message = why,
        None => run(state, ctx, command),
    }
}

/// Run `command` (callers check availability first). Menu commands act on
/// their most likely choice: the active level, the first elevation.
pub fn run(state: &mut AppState, ctx: &egui::Context, command: UiCommand) {
    use UiCommand::*;
    // A ribbon panel shown over the views closes once a command runs.
    state.ribbon_peek = false;
    match command {
        Home => state.show_home = true,
        New => actions::guarded(state, PendingAction::New),
        Open => actions::guarded(state, PendingAction::Open),
        Save => {
            actions::save(state, false);
        }
        SaveAs => {
            actions::save(state, true);
        }
        CloseProject => actions::guarded(state, PendingAction::Close),
        ImportIfc => actions::guarded(state, PendingAction::ImportIfc),
        ExportIfc => actions::export_ifc(state),
        ExportPdf => actions::export_sheet(state, actions::SheetFormat::Pdf),
        ExportSvg => actions::export_sheet(state, actions::SheetFormat::Svg),
        About => state.dialogs.about = true,
        Undo => {
            actions::run_command(state, cc_engine::commands::project::Undo {});
        }
        Redo => {
            actions::run_command(state, cc_engine::commands::project::Redo {});
        }
        Default3D => actions::open_default_3d(state),
        Place(category) => actions::activate_category_tool(state, category),
        NewComponent => {
            state.dialogs.component_editor = Some(crate::component_editor::Editor::new_component());
        }
        LoadComponent => actions::load_component(state),
        Select => state.set_tool(Tool::Select),
        Dimension(mode) => {
            if let Some(Tool::Dimension { type_id, .. }) = Tool::for_category(state.project(), Category::Dimension) {
                state.set_tool(Tool::Dimension { type_id, mode });
                state.message = crate::i18n::tr(mode.hint_key());
            }
        }
        Text => {
            if let Some(tool) = Tool::for_category(state.project(), Category::Text) {
                state.set_tool(tool);
                state.message = t!("hint-text");
            }
        }
        AddLeader => {
            if let Some(id) = crate::ribbon::selected_text(state) {
                state.set_tool(Tool::AddLeader { id });
                state.message = t!("hint-add-leader");
            }
        }
        Move => crate::modify::start(state, Tool::Move),
        CopyElements => crate::modify::start(state, Tool::Copy),
        Rotate => crate::modify::start(state, Tool::Rotate),
        MirrorPick => crate::modify::start(state, Tool::Mirror { draw: false }),
        MirrorDraw => crate::modify::start(state, Tool::Mirror { draw: true }),
        Array => crate::modify::start(state, Tool::Array),
        Align => crate::modify::start_pick(state, Tool::Align),
        Offset => crate::modify::start_pick(state, Tool::Offset),
        TrimCorner => crate::modify::start_pick(state, Tool::TrimCorner),
        TrimExtend => crate::modify::start_pick(state, Tool::TrimExtend),
        Split => crate::modify::start_pick(state, Tool::Split),
        ToggleJoin => crate::modify::start_pick(state, Tool::ToggleJoin),
        Pin => crate::modify::set_pinned(state, true),
        Unpin => crate::modify::set_pinned(state, false),
        Delete => state.delete_selection(),
        Paste => crate::clipboard::paste(state),
        Cut => crate::clipboard::cut(ctx, state),
        ClipboardCopy => {
            crate::clipboard::copy(ctx, state);
        }
        MatchType => crate::clipboard::start_match_type(state),
        FloorPlan | CeilingPlan => {
            let level = state.active_level().or_else(|| state.project().levels.first().map(|l| l.id));
            if let Some(level) = level {
                actions::new_plan(state, level, command == CeilingPlan);
            }
        }
        New3D => {
            actions::new_3d(state);
        }
        Section => {
            state.set_tool(Tool::Section);
            state.message = t!("msg-section-hint");
        }
        Elevations => {
            let first = state.project().views_of(ViewCategory::Elevation).next().map(|v| v.id);
            if let Some(id) = first {
                state.open_view(id);
            }
        }
        Legend => {
            actions::new_legend(state);
        }
        Schedule => {
            actions::new_schedule(state, Category::Wall);
        }
        Sheet => {
            actions::new_sheet(state);
        }
        ZoomFit => crate::app::zoom_fit(state),
        ProjectInfo => state.dialogs.project_info = true,
        Units => state.dialogs.units = true,
        Levels => state.dialogs.levels = true,
        Materials => state.dialogs.materials = true,
        MacroRecord => actions::toggle_macro_recording(state),
        MacroRun => actions::run_macro(state),
        ToggleRibbon => state.ribbon_collapsed = !state.ribbon_collapsed,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn engine_commands_exist() {
        let names: HashSet<String> = cc_engine::catalog().into_iter().map(|c| c.name.to_string()).collect();
        for command in UiCommand::all() {
            if let Some(name) = command.engine() {
                assert!(names.contains(name), "{command:?} runs unknown engine command {name}");
            }
        }
    }

    #[test]
    fn shortcuts_are_unique() {
        let mut seen = HashSet::new();
        for command in UiCommand::all() {
            if let Some(keys) = command.shortcut() {
                assert!(seen.insert(keys), "{keys} is used twice");
                if keys.len() == 2 {
                    assert_eq!(UiCommand::from_keys(keys), Some(command));
                }
            }
        }
        let all = UiCommand::all();
        assert_eq!(all.iter().collect::<HashSet<_>>().len(), all.len(), "a command is listed twice");
    }

    #[test]
    fn availability_follows_the_state() {
        let mut state = AppState::new(cc_core::Document::new(crate::home::sample_project("casa_simples").unwrap()));
        state.project_open = true;
        state.show_home = false;
        assert_eq!(UiCommand::Undo.unavailable(&state), Some(t!("why-nothing-undo")));
        assert_eq!(UiCommand::Move.unavailable(&state), Some(t!("msg-select-first")));
        assert_eq!(UiCommand::Place(Category::Wall).unavailable(&state), None);
        let wall = state.project().walls().next().map(|(e, _)| e.id).unwrap();
        state.selection = [wall].into_iter().collect();
        assert_eq!(UiCommand::Move.unavailable(&state), None);
        state.show_home = true;
        assert_eq!(UiCommand::Save.unavailable(&state), Some(t!("why-no-project")));
        assert_eq!(UiCommand::Open.unavailable(&state), None);
    }
}
