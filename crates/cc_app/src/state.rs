//! Application state shared by all panels and views.

use crate::canvas::View2DState;
use crate::tools::{Interaction, Tool};
use crate::view3d::View3DState;
use cc_core::{Category, Document, Element, ElementId, ElementKind, ElementTypeId, LevelId, Project, ViewId, ViewKind};
use cc_geom::ModelGeometry;
use cc_views::Drawing2D;
use std::collections::{BTreeSet, HashMap};
use std::rc::Rc;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum RibbonTab {
    File,
    #[default]
    Architecture,
    Structure,
    Modify,
    Annotate,
    View,
    Manage,
}

/// An action that discards unsaved changes, waiting for confirmation.
#[derive(Debug, Clone, PartialEq)]
pub enum PendingAction {
    New,
    Open,
    OpenPath(std::path::PathBuf),
    OpenSample(&'static str),
    ImportIfc,
    Close,
    Quit,
}

#[derive(Default)]
pub struct Dialogs {
    pub project_info: bool,
    pub levels: bool,
    pub materials: bool,
    pub type_editor: Option<ElementTypeId>,
    /// Type name being edited by "Rename" in the type dialog.
    pub type_rename: Option<String>,
    /// The layer editor is expanded in the type dialog.
    pub type_layers_open: bool,
    /// Undo depth when the open data dialog started editing (see `dialog::session`).
    pub edit_start: Option<usize>,
    /// Level waiting for delete confirmation.
    pub delete_level: Option<LevelId>,
    /// Schedule whose definition dialog is open.
    pub schedule_def: Option<ViewId>,
    pub confirm: Option<PendingAction>,
    pub error: Option<String>,
    pub about: bool,
}

pub struct AppState {
    pub doc: Document,
    pub geom: ModelGeometry,
    geom_revision: u64,
    pub selection: BTreeSet<ElementId>,
    pub tool: Tool,
    pub interaction: Interaction,
    pub active_view: Option<ViewId>,
    pub view2d: HashMap<ViewId, View2DState>,
    pub view3d: HashMap<ViewId, View3DState>,
    drawings: HashMap<ViewId, (u64, Rc<Drawing2D>)>,
    sheets: HashMap<ViewId, (u64, Rc<cc_views::Sheet>)>,
    /// Views the dock should open (or focus) this frame.
    pub open_requests: Vec<ViewId>,
    pub dialogs: Dialogs,
    pub ribbon_tab: RibbonTab,
    /// Browser inline rename in progress.
    pub rename: Option<(ViewId, String)>,
    /// Selected viewport on a sheet: (sheet, viewport index).
    pub sheet_selection: Option<(ViewId, usize)>,
    /// Status bar message.
    pub message: String,
    /// Cursor position in the active 2D view's coordinates.
    pub cursor: Option<glam::DVec2>,
    /// Typed shortcut letters (e.g. "WA").
    pub key_buffer: String,
    /// Typed length for the wall tool.
    pub length_input: String,
    /// Default height for new walls.
    pub wall_height: f64,
    /// Location line for new walls.
    pub wall_location_line: cc_core::LocationLine,
    /// Level new walls rise to (`None`: unconnected, `wall_height` tall).
    pub wall_top_level: Option<LevelId>,
    /// Allow the window to close after the user confirmed.
    pub quit_confirmed: bool,
    /// egui textures of closed 3D views, freed by the app when the GPU is available.
    pub textures_to_free: Vec<egui::TextureId>,
    /// Bumped whenever the document is replaced (new/open).
    pub generation: u64,
    /// The home screen is shown instead of the project.
    pub show_home: bool,
    /// A project is open (false after "Close Project" and at startup without
    /// a file; the document is then an unused placeholder).
    pub project_open: bool,
    /// Recently opened or saved projects, most recent first.
    pub recent: Vec<std::path::PathBuf>,
    pub thumbnail_jobs: Vec<crate::thumbnails::Job>,
    /// Samples whose thumbnail was already requested this session.
    pub requested_samples: std::collections::BTreeSet<&'static str>,
}

impl AppState {
    pub fn new(doc: Document) -> Self {
        let geom = ModelGeometry::build(&doc.project);
        let geom_revision = doc.revision();
        Self {
            doc,
            geom,
            geom_revision,
            selection: BTreeSet::new(),
            tool: Tool::Select,
            interaction: Interaction::default(),
            active_view: None,
            view2d: HashMap::new(),
            view3d: HashMap::new(),
            drawings: HashMap::new(),
            sheets: HashMap::new(),
            open_requests: Vec::new(),
            dialogs: Dialogs::default(),
            ribbon_tab: RibbonTab::default(),
            rename: None,
            sheet_selection: None,
            message: String::new(),
            cursor: None,
            key_buffer: String::new(),
            length_input: String::new(),
            wall_height: 3.0,
            wall_location_line: Default::default(),
            wall_top_level: None,
            quit_confirmed: false,
            textures_to_free: Vec::new(),
            generation: 0,
            show_home: false,
            project_open: true,
            recent: Vec::new(),
            thumbnail_jobs: Vec::new(),
            requested_samples: Default::default(),
        }
    }

    /// Replace the document (new/open), resetting view state.
    pub fn replace_document(&mut self, doc: Document) {
        let message = std::mem::take(&mut self.message);
        let wall_height = self.wall_height;
        let mut textures: Vec<_> = self.view3d.values().filter_map(|v| v.texture).collect();
        textures.append(&mut self.textures_to_free);
        let generation = self.generation + 1;
        let recent = std::mem::take(&mut self.recent);
        let jobs = std::mem::take(&mut self.thumbnail_jobs);
        let requested = std::mem::take(&mut self.requested_samples);
        *self = Self::new(doc);
        self.recent = recent;
        self.thumbnail_jobs = jobs;
        self.requested_samples = requested;
        self.textures_to_free = textures;
        self.generation = generation;
        self.message = message;
        self.wall_height = wall_height;
    }

    /// Rebuild derived geometry after model changes and drop stale selection.
    pub fn sync(&mut self) {
        if self.geom_revision != self.doc.revision() {
            self.geom = ModelGeometry::build(&self.doc.project);
            self.geom_revision = self.doc.revision();
            let project = &self.doc.project;
            self.selection.retain(|id| project.elements.contains_key(id));
            if let Some((sheet, _)) = self.sheet_selection
                && project.view(sheet).is_none()
            {
                self.sheet_selection = None;
            }
        }
    }

    pub fn project(&self) -> &Project {
        &self.doc.project
    }

    /// Cached drawing for a graphical view.
    pub fn drawing(&mut self, view: ViewId) -> Option<Rc<Drawing2D>> {
        let rev = self.doc.revision();
        if let Some((r, d)) = self.drawings.get(&view)
            && *r == rev
        {
            return Some(d.clone());
        }
        let v = self.doc.project.view(view)?;
        let d = Rc::new(cc_views::generate(&self.doc.project, &self.geom, v)?);
        self.drawings.insert(view, (rev, d.clone()));
        Some(d)
    }

    /// Drop cached drawings (e.g. after a language change alters their text).
    pub fn invalidate_drawings(&mut self) {
        self.drawings.clear();
        self.sheets.clear();
    }

    /// Cached composition of a sheet view (paper millimeters).
    pub fn sheet(&mut self, view: ViewId) -> Option<Rc<cc_views::Sheet>> {
        let rev = self.doc.revision();
        if let Some((r, s)) = self.sheets.get(&view)
            && *r == rev
        {
            return Some(s.clone());
        }
        let sheet_view = self.doc.project.view(view)?.clone();
        let labels = cc_views::TitleLabels {
            project: crate::t!("title-project"),
            sheet: crate::t!("title-sheet"),
            client: crate::t!("title-client"),
            number: crate::t!("title-number"),
            project_number: crate::t!("title-project-number"),
            author: crate::t!("title-author"),
        };
        let ViewKind::Sheet { viewports, .. } = &sheet_view.kind else { return None };
        let drawings: HashMap<ViewId, Rc<Drawing2D>> =
            viewports.iter().filter_map(|vp| Some((vp.view, self.drawing(vp.view)?))).collect();
        let s =
            Rc::new(cc_views::compose_sheet(&self.doc.project, &sheet_view, &labels, |v| drawings.get(&v).cloned())?);
        self.sheets.insert(view, (rev, s.clone()));
        Some(s)
    }

    pub fn open_view(&mut self, view: ViewId) {
        self.open_requests.push(view);
    }

    pub fn set_tool(&mut self, tool: Tool) {
        self.tool = tool;
        self.interaction = Interaction::default();
        self.length_input.clear();
    }

    /// Level of the active plan view, if the active view is a plan.
    pub fn active_level(&self) -> Option<LevelId> {
        self.active_view.and_then(|v| self.doc.project.view(v)).and_then(|v| v.level())
    }

    pub fn delete_selection(&mut self) {
        if self.selection.is_empty() {
            return;
        }
        let ids: Vec<_> = self.selection.iter().copied().collect();
        let n = ids.len();
        self.doc.edit(crate::t!("tx-delete"), |e| {
            for id in ids {
                e.remove_element(id);
            }
        });
        self.selection.clear();
        self.message = crate::t!("msg-deleted", "count" => n);
    }

    /// Create an element in one transaction and select it.
    pub fn create_element(
        &mut self,
        name: String,
        level: LevelId,
        type_id: ElementTypeId,
        kind: ElementKind,
    ) -> ElementId {
        let category = Element {
            id: ElementId(0),
            level,
            type_id,
            mark: String::new(),
            comments: String::new(),
            uid: Default::default(),
            kind: kind.clone(),
        }
        .category();
        let id = self.doc.edit(name, |e| {
            let mark = if matches!(category, Category::Door | Category::Window) {
                e.project().next_mark(category)
            } else {
                String::new()
            };
            let id = ElementId(e.alloc_id());
            e.add_element(Element { id, level, type_id, mark, comments: String::new(), kind, uid: Default::default() })
        });
        self.selection = BTreeSet::from([id]);
        id
    }

    /// Title shown in tabs and window title for a view.
    pub fn view_title(&self, view: ViewId) -> String {
        let Some(v) = self.doc.project.view(view) else { return "?".into() };
        match &v.kind {
            cc_core::ViewKind::Sheet { number, .. } => format!("{number} - {}", v.name),
            cc_core::ViewKind::FloorPlan { .. } => crate::t!("tab-floor-plan", "name" => v.name),
            cc_core::ViewKind::CeilingPlan { .. } => crate::t!("tab-ceiling-plan", "name" => v.name),
            cc_core::ViewKind::Elevation { .. } => crate::t!("tab-elevation", "name" => v.name),
            cc_core::ViewKind::Section { .. } => crate::t!("tab-section", "name" => v.name),
            _ => v.name.clone(),
        }
    }
}
