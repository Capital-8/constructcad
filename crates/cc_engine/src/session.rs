use crate::error::{EngineError, ErrorCode, Result};
use crate::t;
use cc_core::{Document, Project, ViewId, ViewKind};
use cc_geom::ModelGeometry;
use cc_views::Drawing2D;
use serde_json::Value;
use std::collections::HashMap;
use std::rc::Rc;

/// An open project with its derived geometry and drawings, rebuilt lazily
/// when the document changes.
pub struct Session {
    pub doc: Document,
    /// Merge key for the next edit (see [`Session::run_merged`]).
    merge_key: Option<u64>,
    /// Commands recorded as a macro, while recording.
    recording: Option<Vec<Recorded>>,
    geom: ModelGeometry,
    geom_revision: Option<u64>,
    drawings: HashMap<ViewId, (u64, Rc<Drawing2D>)>,
    sheets: HashMap<ViewId, (u64, Rc<cc_views::Sheet>)>,
}

/// A command recorded in a macro.
#[derive(Debug, Clone)]
struct Recorded {
    step: crate::script::Step,
    merge_key: Option<u64>,
    /// Undo depth before the command ran.
    depth: usize,
}

/// A new project from the template, named in the active language.
pub fn new_project() -> Project {
    Project::from_template_localized(&crate::i18n::try_tr)
}

impl Default for Session {
    fn default() -> Self {
        Self::new(Document::new(new_project()))
    }
}

impl Session {
    pub fn new(doc: Document) -> Self {
        Self {
            doc,
            merge_key: None,
            recording: None,
            geom: ModelGeometry::default(),
            geom_revision: None,
            drawings: HashMap::new(),
            sheets: HashMap::new(),
        }
    }

    /// Open a `.ccad` file; returns the session and the number of repairs made.
    pub fn open(path: &std::path::Path) -> Result<(Self, usize)> {
        let (project, repairs) = guard(|| cc_core::load_with_repairs(path))??;
        let mut doc = Document::new(project);
        doc.path = Some(path.to_path_buf());
        Ok((Self::new(doc), repairs))
    }

    /// Replace the document (new, open, import), dropping derived data.
    pub fn replace(&mut self, doc: Document) {
        *self = Self::new(doc);
    }

    pub fn project(&self) -> &Project {
        &self.doc.project
    }

    /// Format quantities with the project's units (drawings, schedules).
    fn sync_units(&self) {
        cc_views::format::set_units(&self.doc.project.units);
    }

    /// Rebuild derived geometry if the document changed; returns whether it did.
    pub fn sync(&mut self) -> bool {
        let rev = self.doc.revision();
        if self.geom_revision == Some(rev) {
            return false;
        }
        self.geom = ModelGeometry::build(&self.doc.project);
        self.geom_revision = Some(rev);
        true
    }

    /// Derived geometry as of the last [`Session::sync`] (callers that keep
    /// it current, like the GUI once a frame, read it without `&mut`).
    pub fn geom(&self) -> &ModelGeometry {
        &self.geom
    }

    /// Derived geometry of the current revision.
    pub fn geometry(&mut self) -> &ModelGeometry {
        self.sync();
        &self.geom
    }

    /// Drawing of a graphical view (cached per revision).
    pub fn drawing(&mut self, view: ViewId) -> Option<Rc<Drawing2D>> {
        let rev = self.doc.revision();
        if let Some((r, d)) = self.drawings.get(&view)
            && *r == rev
        {
            return Some(d.clone());
        }
        self.sync_units();
        self.geometry();
        let v = self.doc.project.view(view)?;
        let d = Rc::new(cc_views::generate(&self.doc.project, &self.geom, v)?);
        self.drawings.insert(view, (rev, d.clone()));
        Some(d)
    }

    /// Drop cached drawings (after a language change alters their text).
    pub fn invalidate_drawings(&mut self) {
        self.drawings.clear();
        self.sheets.clear();
    }

    /// Composition of a sheet in paper millimeters (cached per revision).
    pub fn sheet(&mut self, view: ViewId) -> Option<Rc<cc_views::Sheet>> {
        let rev = self.doc.revision();
        if let Some((r, s)) = self.sheets.get(&view)
            && *r == rev
        {
            return Some(s.clone());
        }
        let sheet_view = self.doc.project.view(view)?.clone();
        let labels = cc_views::TitleLabels {
            project: t!("title-project"),
            sheet: t!("title-sheet"),
            client: t!("title-client"),
            number: t!("title-number"),
            project_number: t!("title-project-number"),
            author: t!("title-author"),
        };
        let ViewKind::Sheet { viewports, .. } = &sheet_view.kind else { return None };
        let drawings: HashMap<ViewId, Rc<Drawing2D>> =
            viewports.iter().filter_map(|vp| Some((vp.view, self.drawing(vp.view)?))).collect();
        let s =
            Rc::new(cc_views::compose_sheet(&self.doc.project, &sheet_view, &labels, |v| drawings.get(&v).cloned())?);
        self.sheets.insert(view, (rev, s.clone()));
        Some(s)
    }

    /// A schedule table of a schedule view, with the project's units.
    pub fn schedule(&mut self, view: ViewId) -> Option<cc_views::Table> {
        let (category, def) = match &self.doc.project.view(view)?.kind {
            ViewKind::Schedule { category, def } => (*category, def.clone()),
            _ => return None,
        };
        self.sync_units();
        self.geometry();
        Some(cc_views::schedule(&self.doc.project, &self.geom, category, &def))
    }

    /// Run a typed command (the way the GUI calls the engine): it goes
    /// through [`Session::execute`] like a command from the CLI or MCP.
    pub fn run<C: crate::Command + serde::Serialize>(&mut self, command: C) -> Result<Value> {
        let args = serde_json::to_value(&command).map_err(|e| EngineError::invalid(e.to_string()))?;
        self.execute(C::NAME, args)
    }

    /// Run a typed command whose edit merges with the previous one of the
    /// same `key` into one undo step (a value being dragged); a different
    /// key, an unmerged edit or [`Document::break_merge`] starts a new step.
    pub fn run_merged<C: crate::Command + serde::Serialize>(&mut self, key: u64, command: C) -> Result<Value> {
        self.merge_key = Some(key);
        let result = self.run(command);
        self.merge_key = None;
        result
    }

    /// The merge key for the edit being run.
    pub(crate) fn take_merge_key(&mut self) -> Option<u64> {
        self.merge_key.take()
    }

    /// Start recording the commands that change the model (a macro).
    pub fn start_recording(&mut self) {
        self.recording = Some(Vec::new());
    }

    pub fn is_recording(&self) -> bool {
        self.recording.is_some()
    }

    /// Stop recording; returns the recorded steps (a dragged value counts once,
    /// with its final value).
    pub fn stop_recording(&mut self) -> Vec<crate::script::Step> {
        self.recording.take().unwrap_or_default().into_iter().map(|r| r.step).collect()
    }

    /// Undo back to undo depth `depth` (cancelling a dialog), dropping what
    /// was recorded since.
    pub fn undo_to(&mut self, depth: usize) {
        while self.doc.undo_depth() > depth && self.doc.undo() {}
        if let Some(rec) = &mut self.recording {
            rec.retain(|r| r.depth < depth);
        }
    }

    fn record(&mut self, name: &str, args: &Value, merge_key: Option<u64>, depth: usize) {
        let kind = crate::describe(name).map(|c| c.kind);
        let Some(rec) = &mut self.recording else { return };
        // Edits only: replaying a macro should not open, save or export files.
        if kind != Some(crate::CommandKind::Edit) {
            return;
        }
        let step = crate::script::Step { command: name.to_string(), args: args.clone() };
        match rec.last_mut() {
            // A value being dragged: keep its last state only.
            Some(last) if merge_key.is_some() && last.merge_key == merge_key && last.step.command == name => {
                last.step = step;
            }
            _ => rec.push(Recorded { step, merge_key, depth }),
        }
    }

    /// Run the command `name` with JSON `args`. A panic inside the command
    /// becomes an internal error and the document is kept.
    pub fn execute(&mut self, name: &str, args: Value) -> Result<Value> {
        let depth = self.doc.undo_depth();
        let merge_key = self.merge_key;
        let recorded_args = self.recording.is_some().then(|| args.clone());
        let result = guard(|| crate::registry::dispatch(self, name, args)).and_then(|r| r);
        if let (Ok(_), Some(args)) = (&result, recorded_args) {
            self.record(name, &args, merge_key, depth);
        }
        if let Err(EngineError { code: ErrorCode::Internal, .. }) = &result {
            // Undo whatever the failed command left behind.
            while self.doc.undo_depth() > depth && self.doc.undo() {}
        }
        result
    }
}

/// Last-resort guard: a panic becomes an internal error.
pub(crate) fn guard<T>(f: impl FnOnce() -> T) -> Result<T> {
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(f)).map_err(|payload| {
        let detail = payload
            .downcast_ref::<&str>()
            .map(|s| s.to_string())
            .or_else(|| payload.downcast_ref::<String>().cloned())
            .unwrap_or_default();
        EngineError::new(ErrorCode::Internal, format!("internal error: {detail}"))
    })
}
