use crate::error::{EngineError, ErrorCode, Result};
use crate::t;
use cc_core::{Document, Project, ViewId, ViewKind};
use cc_geom::ModelGeometry;
use cc_views::Drawing2D;
use serde_json::Value;
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

/// How long [`Session::wait_job`] waits for the extension at a time.
const WAIT_STEP: Duration = Duration::from_millis(100);

/// An open project with its derived geometry and drawings, rebuilt lazily
/// when the document changes.
pub struct Session {
    pub doc: Document,
    /// Merge key for the next edit (see [`Session::run_merged`]).
    merge_key: Option<u64>,
    /// Commands recorded as a macro, while recording.
    recording: Option<Vec<Recorded>>,
    /// The level of the view the user works in (the app's active plan):
    /// recorded steps on it play on the level active when they are played.
    active_level: Option<cc_core::LevelId>,
    geom: ModelGeometry,
    geom_revision: Option<u64>,
    /// Geometry of the phases and design options whose views leave
    /// elements out, with the revision it was built at.
    phase_geoms: HashMap<GeomKey, (u64, ModelGeometry)>,
    drawings: HashMap<ViewId, (u64, Rc<Drawing2D>)>,
    sheets: HashMap<ViewId, (u64, Rc<cc_views::Sheet>)>,
    schedules: HashMap<ViewId, (u64, Rc<cc_views::Table>)>,
    /// Elements temporarily hidden or isolated per view: display state, not
    /// part of the model (not saved, not undone).
    temporary: HashMap<ViewId, cc_core::TemporaryVisibility>,
    /// Counts changes of `temporary` (3D views rebuild their meshes on it).
    temporary_revision: u64,
    /// Commands added at run time (plugins), kept when the document is replaced.
    extension: Option<Box<dyn crate::Extension>>,
    /// How many commands are running (a plugin command runs others).
    nesting: u32,
    /// The files the commands running may read and write (see
    /// [`Session::with_files`]).
    files: crate::FileAccess,
    /// The scopes of the plugin commands running, outermost first (see
    /// [`Session::with_plugin_scope`]).
    scopes: Vec<crate::PluginScope>,
    /// Whether the extension is handling an event (see [`Session::fire`]).
    firing: bool,
    /// Whether the document was replaced during the top-level command
    /// running ([`Session::replace`]).
    replaced: bool,
    /// Whether the event being handled follows an edit command that left its
    /// own undo step on top, which handlers' edits join (see
    /// [`Session::with_event_scope`]).
    fold_events: bool,
    /// Whether [`Session::execute`] waits for a plugin command that waits for
    /// an external program (the default), or leaves it pending (see
    /// [`Session::set_blocking`]).
    blocking: bool,
    /// The plugin command left waiting, if any (see [`Session::poll_job`]).
    job: Option<PendingJob>,
    /// Set by another thread to ask the running command to stop (see
    /// [`Session::set_stop_flag`]).
    stop: Option<Arc<AtomicBool>>,
}

/// What [`Session::execute`] noted before a command ran, to finish it when
/// it ends (at once, or after it waited).
#[derive(Debug, Clone)]
struct Begun {
    name: String,
    /// Run by the user (not by a command or an event handler).
    top: bool,
    /// The document's journal was started for it (`model_changed`).
    journal: bool,
    /// Undo depth before it ran.
    depth: usize,
    /// Undo depth and top step before it ran.
    history: (usize, Option<u64>),
    merge_key: Option<u64>,
    /// Its arguments, when a macro is being recorded.
    recorded_args: Option<Value>,
    /// The id counter before it ran.
    first_id: u64,
}

/// A plugin command waiting for an external program.
#[derive(Debug, Clone)]
struct PendingJob {
    begun: Begun,
    started: Instant,
}

/// A plugin command left waiting (see [`Session::job`]).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Job {
    /// The command's full name (`<plugin>__<name>`).
    pub command: String,
    /// What the plugin says it waits for.
    pub info: crate::JobInfo,
    /// Time since the command started.
    pub elapsed: Duration,
}

/// What a view's own geometry depends on: its phase (an index, when it
/// leaves elements out) and the design options it shows.
type GeomKey = (Option<usize>, std::collections::BTreeSet<cc_core::OptionId>);

/// A command recorded in a macro.
#[derive(Debug, Clone)]
struct Recorded {
    step: crate::script::Step,
    /// The ids it allocated and its result, to refer to from later steps.
    made: Option<crate::script::Made>,
    merge_key: Option<u64>,
    /// Undo depth before the command ran.
    depth: usize,
}

/// A new project from the metric template, named in the active language.
pub fn new_project() -> Project {
    new_project_with(cc_core::MeasurementSystem::Metric)
}

/// A new project from the template of `system`, named in the active language.
pub fn new_project_with(system: cc_core::MeasurementSystem) -> Project {
    Project::from_template_system(system, &crate::i18n::try_tr)
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
            active_level: None,
            geom: ModelGeometry::default(),
            geom_revision: None,
            phase_geoms: HashMap::new(),
            drawings: HashMap::new(),
            sheets: HashMap::new(),
            schedules: HashMap::new(),
            temporary: HashMap::new(),
            temporary_revision: 0,
            extension: None,
            nesting: 0,
            files: crate::FileAccess::All,
            scopes: Vec::new(),
            firing: false,
            replaced: false,
            fold_events: false,
            blocking: true,
            job: None,
            stop: None,
        }
    }

    /// Open a `.ccad` file; returns the session and the number of repairs made.
    pub fn open(path: &std::path::Path) -> Result<(Self, usize)> {
        let (project, repairs) = guard(|| cc_core::load_with_repairs_localized(path, &crate::i18n::try_tr))??;
        let mut doc = Document::new(project);
        doc.path = Some(path.to_path_buf());
        Ok((Self::new(doc), repairs))
    }

    /// Replace the document (new, open, import), dropping derived data.
    pub fn replace(&mut self, doc: Document) {
        // A command left waiting belongs to the document that goes.
        self.cancel_job();
        let extension = self.extension.take();
        let nesting = self.nesting;
        let files = std::mem::take(&mut self.files);
        let scopes = std::mem::take(&mut self.scopes);
        let firing = self.firing;
        let blocking = self.blocking;
        *self = Self::new(doc);
        self.blocking = blocking;
        self.extension = extension;
        self.nesting = nesting;
        self.files = files;
        self.scopes = scopes;
        self.firing = firing;
        // The command running fires `project_opened` when it ends.
        self.replaced = true;
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
        self.phase_geoms.retain(|_, (r, _)| *r == rev);
        // Drop what is kept for views that are gone.
        let views = &self.doc.project.views;
        self.drawings.retain(|v, _| views.contains_key(v));
        self.sheets.retain(|v, _| views.contains_key(v));
        self.schedules.retain(|v, _| views.contains_key(v));
        true
    }

    /// The phase (an index, when it leaves elements out) and design options
    /// whose own geometry `view` shows, or `None` when it shows the whole
    /// model's (see [`ModelGeometry::build_view`]).
    fn geom_key(&self, view: ViewId) -> Option<GeomKey> {
        let p = &self.doc.project;
        let v = p.view(view)?;
        if !v.kind.is_phased() {
            return None;
        }
        let phase = p.view_phase(v).and_then(|ph| p.phase_index(ph)).filter(|i| p.phase_differs(*i));
        let options = p.shown_options(Some(v));
        if phase.is_none() && options == p.shown_options(None) {
            return None;
        }
        Some((phase, options))
    }

    /// Build (if needed) the geometry `view` shows; returns its key.
    fn sync_view(&mut self, view: ViewId) -> Option<GeomKey> {
        self.sync();
        let key = self.geom_key(view)?;
        let rev = self.doc.revision();
        if self.phase_geoms.get(&key).is_none_or(|(r, _)| *r != rev) {
            let g = ModelGeometry::build_view(&self.doc.project, key.0, &key.1, &self.geom);
            self.phase_geoms.insert(key.clone(), (rev, g));
        }
        Some(key)
    }

    fn geom_of(&self, key: Option<GeomKey>) -> &ModelGeometry {
        key.and_then(|k| self.phase_geoms.get(&k)).map_or(&self.geom, |(_, g)| g)
    }

    /// Geometry of the model as `view` shows it, in its phase: only the
    /// elements of that phase join, open walls and bound rooms. The whole
    /// model's geometry when the phase leaves nothing out.
    pub fn view_geometry(&mut self, view: ViewId) -> &ModelGeometry {
        let key = self.sync_view(view);
        self.geom_of(key)
    }

    /// [`Session::view_geometry`] as of the last sync (the GUI calls
    /// [`Session::view_geometry`] once a frame per open view first); falls
    /// back to the whole model's geometry when the phase's is not built.
    pub fn geom_for(&self, view: ViewId) -> &ModelGeometry {
        let rev = self.doc.revision();
        match self.geom_key(view).and_then(|k| self.phase_geoms.get(&k)) {
            Some((r, g)) if *r == rev => g,
            _ => &self.geom,
        }
    }

    /// Geometry of the model in which element `id` stands: the whole
    /// model's, or, for an element of a design option views do not show by
    /// default, the main model with that option (cached like a view's).
    pub fn element_geometry(&mut self, id: cc_core::ElementId) -> &ModelGeometry {
        self.sync();
        let p = &self.doc.project;
        let mut options = p.shown_options(None);
        let key = match p.element(id).and_then(|e| e.design_option) {
            Some(o) if !options.contains(&o) => {
                if let Some(set) = p.option_set_of(o) {
                    options.retain(|x| !set.contains(*x));
                }
                options.insert(o);
                Some((None, options))
            }
            _ => None,
        };
        let rev = self.doc.revision();
        if let Some(key) = &key
            && self.phase_geoms.get(key).is_none_or(|(r, _)| *r != rev)
        {
            let g = ModelGeometry::build_view(&self.doc.project, None, &key.1, &self.geom);
            self.phase_geoms.insert(key.clone(), (rev, g));
        }
        self.geom_of(key)
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

    /// The project with its current geometry.
    pub fn project_and_geometry(&mut self) -> (&Project, &ModelGeometry) {
        self.sync();
        (&self.doc.project, &self.geom)
    }

    /// Drawing of a graphical view, with what is temporarily hidden or
    /// isolated in it left out (cached per revision).
    pub fn drawing(&mut self, view: ViewId) -> Option<Rc<Drawing2D>> {
        let rev = self.doc.revision();
        if let Some((r, d)) = self.drawings.get(&view)
            && *r == rev
        {
            return Some(d.clone());
        }
        self.sync_units();
        let key = self.sync_view(view);
        let v = self.doc.project.view(view)?;
        let temporary = self.temporary.get(&view);
        let geom = self.geom_of(key);
        let d = Rc::new(cc_views::generate_temporary(&self.doc.project, geom, v, &crate::i18n::tr, temporary)?);
        self.drawings.insert(view, (rev, d.clone()));
        Some(d)
    }

    /// Drawing of a view as printed: without temporary hiding or isolation,
    /// and without its underlay unless the view prints it.
    pub fn printed_drawing(&mut self, view: ViewId) -> Option<Rc<Drawing2D>> {
        let d = if self.temporary.contains_key(&view) {
            self.sync_units();
            let key = self.sync_view(view);
            let v = self.doc.project.view(view)?;
            Rc::new(cc_views::generate_labeled(&self.doc.project, self.geom_of(key), v, &crate::i18n::tr)?)
        } else {
            self.drawing(view)?
        };
        let printed = self.doc.project.view(view).and_then(|v| v.kind.underlay()).is_some_and(|u| u.printed);
        Some(if d.underlay > 0 && !printed { Rc::new(d.without_underlay()) } else { d })
    }

    /// What is temporarily hidden or isolated in `view`.
    pub fn temporary(&self, view: ViewId) -> Option<&cc_core::TemporaryVisibility> {
        self.temporary.get(&view)
    }

    /// Set (or, with `None` or an empty set, reset) what is temporarily
    /// hidden or isolated in `view`. Not an edit: nothing to undo or save.
    pub fn set_temporary(&mut self, view: ViewId, temporary: Option<cc_core::TemporaryVisibility>) {
        match temporary.filter(|t| !t.is_empty()) {
            Some(t) => self.temporary.insert(view, t),
            None => self.temporary.remove(&view),
        };
        self.drawings.remove(&view);
        self.temporary_revision += 1;
    }

    /// Keep the camera of 3D view `view` (not undoable). The model's geometry
    /// stays: only the view's drawing and the sheets showing it are redrawn.
    pub fn set_view_camera(&mut self, view: ViewId, camera: cc_core::CameraState) {
        self.doc.set_view_state(|p| {
            if let Some(ViewKind::ThreeD { camera: c, .. }) = p.views.get_mut(&view).map(|v| &mut v.kind) {
                *c = camera;
            }
        });
        self.drawings.remove(&view);
        let project = &self.doc.project;
        self.sheets.retain(|sheet, _| {
            !matches!(project.view(*sheet).map(|v| &v.kind),
                Some(ViewKind::Sheet { viewports, .. }) if viewports.iter().any(|vp| vp.view == view))
        });
    }

    /// Changes whenever any view's temporary visibility changes.
    pub fn temporary_revision(&self) -> u64 {
        self.temporary_revision
    }

    /// Drop cached drawings (after a language change alters their text).
    pub fn invalidate_drawings(&mut self) {
        self.drawings.clear();
        self.sheets.clear();
        self.schedules.clear();
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
        let env = cc_views::SheetEnv {
            today: crate::i18n::today(),
            format_date: crate::i18n::format_date,
            as_indicated: t!("sheet-as-indicated"),
        };
        let ViewKind::Sheet { viewports, .. } = &sheet_view.kind else { return None };
        let drawings: HashMap<ViewId, Rc<Drawing2D>> =
            viewports.iter().filter_map(|vp| Some((vp.view, self.printed_drawing(vp.view)?))).collect();
        let s = Rc::new(cc_views::compose_sheet(&self.doc.project, &sheet_view, &env, |v| drawings.get(&v).cloned())?);
        self.sheets.insert(view, (rev, s.clone()));
        Some(s)
    }

    /// A schedule table of a schedule view, with the project's units: the
    /// elements its phase and phase filter show (cached per revision).
    pub fn schedule(&mut self, view: ViewId) -> Option<Rc<cc_views::Table>> {
        let rev = self.doc.revision();
        if let Some((r, t)) = self.schedules.get(&view)
            && *r == rev
        {
            return Some(t.clone());
        }
        let (category, def) = match &self.doc.project.view(view)?.kind {
            ViewKind::Schedule { category, def } => (*category, def.clone()),
            _ => return None,
        };
        self.sync_units();
        self.geometry();
        let p = &self.doc.project;
        let v = p.view(view)?;
        let phase = cc_views::graphics::PhaseView::of(p, v);
        let options = p.shown_options(Some(v));
        let table = Rc::new(cc_views::schedule_where(p, &self.geom, category, &def, &mut |e| {
            p.option_shows(e, &options) && phase.is_none_or(|ph| ph.shows(p, e))
        }));
        self.schedules.insert(view, (rev, table.clone()));
        Some(table)
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
    /// with its final value). Ids of objects an earlier step created become
    /// `{"$step": n}` references and the active level `{"$level": "active"}`
    /// (see [`crate::script`]), so the macro plays on any level and project.
    pub fn stop_recording(&mut self) -> Vec<crate::script::Step> {
        let (mut steps, made): (Vec<_>, Vec<_>) =
            self.recording.take().unwrap_or_default().into_iter().map(|r| (r.step, r.made)).unzip();
        crate::script::link(&mut steps, &made);
        steps
    }

    /// Set the level of the view the user works in (`None` outside plans);
    /// see [`Session::active_level`].
    pub fn set_active_level(&mut self, level: Option<cc_core::LevelId>) {
        self.active_level = level;
    }

    /// A level by id or name (`ccad script level=…`).
    pub fn level(&self, r: &crate::Ref) -> Result<cc_core::LevelId> {
        crate::refs::level(&self.doc.project, r)
    }

    /// The level of the view the user works in, if it exists: recording
    /// writes it as the active level placeholder, and scripts play that
    /// placeholder on it.
    pub fn active_level(&self) -> Option<cc_core::LevelId> {
        self.active_level.filter(|l| self.doc.project.level(*l).is_some())
    }

    /// Undo back to undo depth `depth` (cancelling a dialog), dropping what
    /// was recorded since.
    pub fn undo_to(&mut self, depth: usize) {
        while self.doc.undo_depth() > depth && self.doc.undo() {}
        if let Some(rec) = &mut self.recording {
            rec.retain(|r| r.depth < depth);
        }
    }

    /// Record a command that succeeded; `first_id` is the id counter before
    /// it ran.
    fn record(&mut self, name: &str, args: Value, result: &Value, first_id: u64, merge_key: Option<u64>, depth: usize) {
        if self.recording.is_none() {
            return;
        }
        let kind = self.kind_of(name);
        // Edits only: replaying a macro should not open, save or export files.
        if kind != Some(crate::CommandKind::Edit) {
            return;
        }
        let mut args = args;
        if let Some(level) = self.active_level().and_then(|l| self.doc.project.level(l)) {
            crate::script::mark_level(&mut args, level.id, &level.name, 0);
        }
        let next_id = self.doc.project.next_id();
        let made = (next_id > first_id).then(|| crate::script::Made { ids: first_id..next_id, result: result.clone() });
        let Some(rec) = &mut self.recording else { return };
        let step = crate::script::Step { command: name.to_string(), args };
        match rec.last_mut() {
            // A value being dragged: keep its last state only.
            Some(last) if merge_key.is_some() && last.merge_key == merge_key && last.step.command == name => {
                last.step = step;
                if let Some(m) = made {
                    last.made = Some(match last.made.take() {
                        Some(old) => crate::script::Made { ids: old.ids.start..m.ids.end, result: m.result },
                        None => m,
                    });
                }
            }
            _ => rec.push(Recorded { step, made, merge_key, depth }),
        }
    }

    /// Set the source of commands beyond the built-in catalog (plugins), or
    /// remove it with `None`; returns the one it replaces. A new extension
    /// hears `project_opened` for the project the session holds (so plugins
    /// set on an open project can start), unless a command is running.
    pub fn set_extension(&mut self, extension: Option<Box<dyn crate::Extension>>) -> Option<Box<dyn crate::Extension>> {
        // A command left waiting lives in the extension that goes.
        self.cancel_job();
        let old = std::mem::replace(&mut self.extension, extension);
        if self.extension.is_some() && self.top_level() {
            let path = self.doc.path.clone();
            self.fire(crate::Event::ProjectOpened { new: path.is_none(), path });
        }
        old
    }

    /// Whether a command run now would be a top-level one: not run by a
    /// command or an event handler, nor while a plugin command waits.
    fn top_level(&self) -> bool {
        self.nesting == 0 && self.scopes.is_empty() && !self.firing && self.job.is_none()
    }

    /// Whether the extension handles events of `kind`.
    fn listens(&self, kind: crate::EventKind) -> bool {
        self.extension.as_ref().is_some_and(|e| e.listens(kind))
    }

    /// Give `event` to the extension, if it listens: it is taken out of the
    /// session meanwhile, and what it runs is nested (not top-level, not
    /// recorded in a macro), so no event fires during it. A panic in it is
    /// caught; the extension stays.
    fn fire(&mut self, event: crate::Event) {
        if !self.listens(event.kind()) {
            return;
        }
        let Some(mut ext) = self.extension.take() else { return };
        let (nesting, merge_key) = (self.nesting, self.merge_key.take());
        self.nesting = nesting.saturating_add(1);
        self.firing = true;
        let _ = guard(|| ext.on_event(self, &event));
        self.firing = false;
        self.nesting = nesting;
        self.merge_key = merge_key;
        if self.extension.is_none() {
            self.extension = Some(ext);
        }
    }

    /// Run `f`, the handlers of the plugin `plugin` (named `name` for the
    /// user) for `event`, in its scope: queries, plus `plugin_data_set` for
    /// its own id (only queries after `undo` and `redo`, whose edits would
    /// clear the redo history and come back on every undo), and no files.
    /// What they change after an edit command (`model_changed`) joins that
    /// command's undo step, so one undo takes back both and a dragged value
    /// stays one merged step; after other events (or a command that left no
    /// step of its own) it becomes one undo step of its own, named after the
    /// plugin (`tx-plugin-event`). Nothing when they change nothing.
    pub fn with_event_scope<R>(
        &mut self,
        event: &crate::Event,
        plugin: &str,
        name: &str,
        f: impl FnOnce(&mut Session) -> R,
    ) -> R {
        let edits = match event {
            crate::Event::ModelChanged(c) if c.is_history() => crate::ScopeEdits::None,
            _ => crate::ScopeEdits::OwnData,
        };
        let scope = crate::PluginScope { plugin: plugin.to_string(), edits, files: crate::FileAccess::None };
        let mark = self.doc.history_mark();
        let out = self.with_plugin_scope(scope, f);
        let fold = self.fold_events && matches!(event, crate::Event::ModelChanged(_));
        if !(fold && self.doc.fold_since(mark)) {
            self.doc.squash_since(mark.depth(), t!("tx-plugin-event", "plugin" => name));
        }
        out
    }

    /// [`Session::set_extension`] without `project_opened`: moving the loaded
    /// plugins to a session whose project the user did not open (the app's
    /// blank document after closing a project).
    pub fn set_extension_quietly(
        &mut self,
        extension: Option<Box<dyn crate::Extension>>,
    ) -> Option<Box<dyn crate::Extension>> {
        self.cancel_job();
        std::mem::replace(&mut self.extension, extension)
    }

    pub fn extension(&self) -> Option<&dyn crate::Extension> {
        self.extension.as_deref()
    }

    /// The extension's messages for the user since the last call.
    pub fn take_extension_messages(&mut self) -> Vec<String> {
        self.extension.as_mut().map(|e| e.take_messages()).unwrap_or_default()
    }

    /// Whether the command running was called by another command (a plugin
    /// command), not by the user.
    pub fn called_by_command(&self) -> bool {
        self.nesting > 1
    }

    /// Whether a command is running (a plugin command running others).
    pub fn is_nested(&self) -> bool {
        self.nesting > 0
    }

    /// Whether the commands running may read and write any file: true unless
    /// a caller with less access (a plugin without the `files` permission, a
    /// graph run without `allow_files`) runs them.
    pub fn files_allowed(&self) -> bool {
        self.files.is_all()
    }

    /// The files the commands running may read and write.
    pub fn file_access(&self) -> &crate::FileAccess {
        &self.files
    }

    /// Run `f` with file access narrowed to `access` (it is never widened: a
    /// caller without files cannot give them to what it runs). The access is
    /// restored however `f` ends.
    pub fn with_files<R>(&mut self, access: crate::FileAccess, f: impl FnOnce(&mut Session) -> R) -> R {
        let outer = self.files.clone();
        self.files = outer.narrow(&access);
        let scopes = self.scopes.len();
        self.restoring(outer, scopes, f)
    }

    /// Run `f` (a plugin command) in `scope`: every command executed during
    /// it, also by graphs, scripts or macros it starts, may touch only the
    /// plugin's own data, may not replace the document, move in its history
    /// or change the language, may edit only when `scope.edits`, and works
    /// with the files `scope.files` allows. Scopes nest (each one holds) and
    /// are restored however `f` ends.
    pub fn with_plugin_scope<R>(&mut self, scope: crate::PluginScope, f: impl FnOnce(&mut Session) -> R) -> R {
        let outer = self.files.clone();
        self.files = outer.narrow(&scope.files);
        let scopes = self.scopes.len();
        self.scopes.push(scope);
        self.restoring(outer, scopes, f)
    }

    /// The scope of the plugin command running (the innermost), if any.
    pub fn plugin_scope(&self) -> Option<&crate::PluginScope> {
        self.scopes.last()
    }

    /// Run `f`, then put back the file access `files` and the first `scopes`
    /// scopes, also when `f` panics (the panic goes on).
    fn restoring<R>(&mut self, files: crate::FileAccess, scopes: usize, f: impl FnOnce(&mut Session) -> R) -> R {
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| f(self)));
        self.files = files;
        self.scopes.truncate(scopes);
        match result {
            Ok(r) => r,
            Err(payload) => std::panic::resume_unwind(payload),
        }
    }

    /// Refuse reading the file `path` given to a command (in its parameter
    /// `param`) when the commands running may not read it.
    pub fn require_read(&self, param: &str, path: &std::path::Path) -> Result<()> {
        if self.files.may_read(path) { Ok(()) } else { Err(self.file_refused(param, path, "read")) }
    }

    /// Refuse writing the file `path` given to a command (in its parameter
    /// `param`) when the commands running may not write it.
    pub fn require_write(&self, param: &str, path: &std::path::Path) -> Result<()> {
        if self.files.may_write(path) { Ok(()) } else { Err(self.file_refused(param, path, "write")) }
    }

    fn file_refused(&self, param: &str, path: &std::path::Path, verb: &str) -> EngineError {
        match &self.files {
            crate::FileAccess::Granted(_) => EngineError::rejected(format!(
                "`{param}` ({}): this plugin command may not {verb} that file (without the \"files\" permission a \
                 plugin works only with the files given in its save_path and open_path parameters)",
                path.display()
            )),
            _ => EngineError::rejected(format!(
                "`{param}` names a file, and this run may not work with files (a plugin needs the \"files\" \
                 permission, a graph `allow_files`)"
            )),
        }
    }

    /// Why `name` may not run now, in the scopes of the plugin commands
    /// running and with the file access of the run.
    fn refusal(&self, name: &str, args: &Value) -> Option<EngineError> {
        let kind = self.kind_of(name);
        // While a plugin command waits, its undo step is open: only queries
        // run (what the command runs when it goes on is not refused, as the
        // job is taken out of the session meanwhile).
        if let Some(job) = &self.job
            && (crate::extension::is_extension_name(name) || kind != Some(crate::CommandKind::Query))
        {
            return Some(EngineError::rejected(format!(
                "{name}: a plugin command is running ({}); wait for it to end or cancel it",
                job.begun.name
            )));
        }
        for scope in &self.scopes {
            // A plugin command is one undo step of this document, in the
            // user's language.
            if matches!(name, "project_new" | "project_open" | "undo" | "redo" | "language_set") {
                return Some(EngineError::rejected(format!("{name} cannot run inside a plugin command")));
            }
            if name.starts_with("plugin_data_") && args.get("plugin").and_then(Value::as_str) != Some(&scope.plugin) {
                return Some(EngineError::rejected(format!(
                    "{name}: a plugin reads and writes only its own data (plugin = \"{}\")",
                    scope.plugin
                )));
            }
            if kind == Some(crate::CommandKind::Edit) {
                match scope.edits {
                    crate::ScopeEdits::All => {}
                    // Its own id: checked above.
                    crate::ScopeEdits::OwnData if name == "plugin_data_set" => {}
                    crate::ScopeEdits::OwnData => {
                        return Some(EngineError::rejected(format!(
                            "{name} changes the model, which an event handler may not do (it may store its own \
                             plugin data with plugin_data_set)"
                        )));
                    }
                    crate::ScopeEdits::None => {
                        return Some(EngineError::rejected(format!(
                            "{name} changes the model, which a query command may not do"
                        )));
                    }
                }
            }
        }
        if kind != Some(crate::CommandKind::File) {
            return None;
        }
        match &self.files {
            crate::FileAccess::All => None,
            crate::FileAccess::None => Some(EngineError::rejected(format!(
                "{name} works with files, and this run may not (a plugin needs the \"files\" permission, a graph \
                 `allow_files`)"
            ))),
            // A command that checks each path it touches may work with the
            // granted ones.
            crate::FileAccess::Granted(_) if crate::registry::checks_paths(name) => None,
            crate::FileAccess::Granted(_) => Some(EngineError::rejected(format!(
                "{name} works with files: a plugin without the \"files\" permission runs only the file commands that \
                 write or read the paths given in its save_path and open_path parameters"
            ))),
        }
    }

    /// Every command this session runs: the built-in catalog, then the
    /// extension's commands.
    pub fn commands(&self) -> Vec<crate::CommandInfo> {
        let mut all = crate::catalog();
        if let Some(ext) = &self.extension {
            all.extend(ext.commands());
        }
        all
    }

    /// The catalog entry of one command, built-in or from the extension.
    pub fn describe(&self, name: &str) -> Option<crate::CommandInfo> {
        if crate::extension::is_extension_name(name) {
            self.extension.as_ref()?.commands().into_iter().find(|c| c.name == name)
        } else {
            crate::describe(name)
        }
    }

    /// The kind of a command, built-in or from the extension.
    pub fn kind_of(&self, name: &str) -> Option<crate::CommandKind> {
        if crate::extension::is_extension_name(name) {
            self.extension.as_ref()?.commands().into_iter().find(|c| c.name == name).map(|c| c.kind)
        } else {
            crate::command_kind(name)
        }
    }

    /// Run the command `name` with JSON `args`. A panic inside the command
    /// becomes an internal error and the document is kept. Commands of the
    /// extension (names with `__`) are one undo step however many commands
    /// they run, and leave nothing behind when they fail. A top-level
    /// command fires the extension's events (see [`crate::extension`]).
    ///
    /// A plugin command that waits for an external program is waited for
    /// when the session is blocking (the default) or the command was run by
    /// another command; otherwise it is left pending and this returns
    /// [`ErrorCode::Pending`]: [`Session::poll_job`] goes on with it, and
    /// meanwhile only queries run.
    pub fn execute(&mut self, name: &str, args: Value) -> Result<Value> {
        if let Some(refused) = self.refusal(name, &args) {
            return Err(refused);
        }
        let top = self.top_level();
        if top && name == "project_save" {
            let path = args.get("path").and_then(Value::as_str).map(std::path::PathBuf::from);
            if let Some(path) = path.or_else(|| self.doc.path.clone()) {
                self.fire(crate::Event::ProjectSaving { path });
            }
        }
        // What the command changes, for `model_changed`.
        let journal = top && self.listens(crate::EventKind::ModelChanged);
        if journal {
            self.doc.start_journal();
        }
        if top {
            self.replaced = false;
        }
        let depth = self.doc.undo_depth();
        let begun = Begun {
            name: name.to_string(),
            top,
            journal,
            depth,
            history: (depth, self.doc.undo_top_id()),
            merge_key: self.merge_key,
            // Commands run by another command are part of it, not steps of a macro.
            recorded_args: (self.recording.is_some() && self.nesting == 0).then(|| args.clone()),
            first_id: self.doc.project.next_id(),
        };
        let outer = self.nesting;
        self.nesting = outer.saturating_add(1);
        // Names and messages a command writes follow the project's units.
        self.sync_units();
        let outcome = if crate::extension::is_extension_name(name) {
            self.start_extension(name, args)
        } else {
            crate::Outcome::Done(guard(|| crate::registry::dispatch(self, name, args)).and_then(|r| r))
        };
        self.nesting = outer;
        match outcome {
            crate::Outcome::Done(result) => self.finish(begun, result),
            crate::Outcome::Pending => {
                self.job = Some(PendingJob { begun, started: Instant::now() });
                if self.blocking || !top {
                    return self.wait_job().unwrap_or_else(|| {
                        Err(EngineError::new(ErrorCode::Internal, format!("{name}: the command was lost")))
                    });
                }
                let tool = self.job().map(|j| j.info.tool).filter(|t| !t.is_empty());
                Err(EngineError::new(
                    ErrorCode::Pending,
                    format!("{name} is running {} in the background", tool.as_deref().unwrap_or("a program")),
                ))
            }
        }
    }

    /// End the command `begun` with `result`: an extension's command is
    /// squashed into one undo step (undone when it failed), a command that
    /// succeeded is recorded in the macro being recorded, and a top-level
    /// one fires its events.
    fn finish(&mut self, begun: Begun, result: Result<Value>) -> Result<Value> {
        let Begun { name, top, journal, depth, history, merge_key, recorded_args, first_id } = begun;
        if crate::extension::is_extension_name(&name) {
            match &result {
                Ok(_) => {
                    self.doc.squash_since(depth, &name);
                }
                // What a cancelled command did goes, and cannot be redone.
                Err(EngineError { code: ErrorCode::Cancelled, .. }) => self.doc.discard_since(depth),
                Err(_) => self.undo_to(depth),
            }
        }
        if let (Ok(out), Some(args)) = (&result, recorded_args) {
            self.record(&name, args, out, first_id, merge_key, depth);
        }
        if let Err(EngineError { code: ErrorCode::Internal, .. }) = &result {
            // Undo whatever the failed command left behind.
            while self.doc.undo_depth() > depth && self.doc.undo() {}
        }
        if top {
            let changes = if journal { self.doc.take_journal() } else { None };
            if result.is_ok() {
                self.fire_after(&name, changes, history);
            }
        }
        result
    }

    /// The events of the top-level command `name` that succeeded, with what
    /// its document's journal noted (`None` when it was off or the document
    /// was replaced) and the undo history's depth and top step before it.
    fn fire_after(&mut self, name: &str, changes: Option<cc_core::ElementChanges>, history: (usize, Option<u64>)) {
        if std::mem::take(&mut self.replaced) {
            let path = self.doc.path.clone();
            self.fire(crate::Event::ProjectOpened { new: path.is_none(), path });
        } else if name == "project_save" {
            if let Some(path) = self.doc.path.clone() {
                self.fire(crate::Event::ProjectSaved { path });
            }
        } else if let Some(changes) = changes.filter(|c| !c.is_empty()) {
            let change = crate::ModelChange::new(name, changes);
            // The command left a step of its own on top (a new one, or one it
            // merged into), unless it moved in the history.
            let now = (self.doc.undo_depth(), self.doc.undo_top_id());
            self.fold_events = !change.is_history() && now != history && now.0 >= history.0;
            self.fire(crate::Event::ModelChanged(change));
            self.fold_events = false;
        }
    }

    fn start_extension(&mut self, name: &str, args: Value) -> crate::Outcome {
        let Some(mut ext) = self.extension.take() else {
            let message = if self.nesting > 1 {
                format!("{name}: a plugin command cannot run inside another plugin command")
            } else {
                format!("unknown command: {name}")
            };
            return crate::Outcome::Done(Err(EngineError::new(ErrorCode::UnknownCommand, message)));
        };
        // Its edits are squashed into one step: none merges with an earlier one.
        self.merge_key = None;
        self.doc.break_merge();
        let outcome = guard(|| ext.start(self, name, args));
        if self.extension.is_none() {
            self.extension = Some(ext);
        }
        match outcome {
            Ok(Some(o)) => o,
            Ok(None) => crate::Outcome::Done(Err(EngineError::new(
                ErrorCode::UnknownCommand,
                format!("unknown command: {name}"),
            ))),
            Err(e) => crate::Outcome::Done(Err(e)),
        }
    }

    /// Wait for plugin commands that wait for an external program (`true`,
    /// the default: the command line, scripts, MCP), or leave them pending
    /// (`false`: the app, which calls [`Session::poll_job`] every frame).
    /// Commands run by other commands (graphs, scripts) and the steps of
    /// [`crate::script`] are always waited for.
    pub fn set_blocking(&mut self, blocking: bool) {
        self.blocking = blocking;
    }

    /// Whether [`Session::execute`] waits for plugin commands that wait.
    pub fn is_blocking(&self) -> bool {
        self.blocking
    }

    /// Run `f` with [`Session::set_blocking`] set to `blocking`, then put the
    /// previous mode back (also when `f` panics).
    pub fn with_blocking<R>(&mut self, blocking: bool, f: impl FnOnce(&mut Session) -> R) -> R {
        let outer = std::mem::replace(&mut self.blocking, blocking);
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| f(self)));
        self.blocking = outer;
        match result {
            Ok(r) => r,
            Err(payload) => std::panic::resume_unwind(payload),
        }
    }

    /// Let another thread ask the commands of this session to stop: a long
    /// command that can stop (`rvt_import`) checks the flag while it runs and
    /// fails as [`ErrorCode::Cancelled`] once it is set. The app sets it when
    /// the user cancels a job running on a worker thread.
    pub fn set_stop_flag(&mut self, flag: Arc<AtomicBool>) {
        self.stop = Some(flag);
    }

    /// Whether the running command was asked to stop ([`Session::set_stop_flag`]).
    pub fn stop_requested(&self) -> bool {
        self.stop.as_ref().is_some_and(|f| f.load(Ordering::Relaxed))
    }

    /// Whether a plugin command is waiting (see [`Session::poll_job`]).
    pub fn has_job(&self) -> bool {
        self.job.is_some()
    }

    /// The plugin command waiting, if any: its name, what the plugin says it
    /// waits for, and the time since it started.
    pub fn job(&self) -> Option<Job> {
        let job = self.job.as_ref()?;
        let info = self.extension.as_ref().and_then(|e| e.job()).unwrap_or_default();
        Some(Job { command: job.begun.name.clone(), info, elapsed: job.started.elapsed() })
    }

    /// Go on with the plugin command waiting, without waiting: `None` while
    /// it still waits (or when none does); its result once it ended, finished
    /// like any command (one undo step, undone when it failed, its events
    /// fired).
    pub fn poll_job(&mut self) -> Option<Result<Value>> {
        self.drive_job(Duration::ZERO)
    }

    /// Wait for the plugin command waiting to end; its result (`None` when
    /// none waits).
    pub fn wait_job(&mut self) -> Option<Result<Value>> {
        while self.job.is_some() {
            if let Some(result) = self.drive_job(WAIT_STEP) {
                return Some(result);
            }
        }
        None
    }

    /// Cancel the plugin command waiting: what it waits for is stopped and
    /// the command fails ([`ErrorCode::Cancelled`]), leaving nothing behind.
    /// `None` when none waits.
    pub fn cancel_job(&mut self) -> Option<Result<Value>> {
        let job = self.job.take()?;
        if let Some(mut ext) = self.extension.take() {
            let outer = self.nesting;
            self.nesting = outer.saturating_add(1);
            let _ = guard(|| ext.cancel(self));
            self.nesting = outer;
            if self.extension.is_none() {
                self.extension = Some(ext);
            }
        }
        let message = format!("{}: cancelled", job.begun.name);
        Some(self.finish(job.begun, Err(EngineError::new(ErrorCode::Cancelled, message))))
    }

    /// Let the extension go on with the command waiting, waiting at most
    /// `wait`; its result when it ended.
    fn drive_job(&mut self, wait: Duration) -> Option<Result<Value>> {
        let job = self.job.take()?;
        let outcome = match self.extension.take() {
            None => crate::Outcome::Done(Err(EngineError::new(
                ErrorCode::Internal,
                format!("{}: the plugins were unloaded while it ran", job.begun.name),
            ))),
            Some(mut ext) => {
                let (outer, merge_key) = (self.nesting, self.merge_key.take());
                // What it runs now is part of it, as when it started.
                self.nesting = outer.saturating_add(1);
                self.sync_units();
                let outcome = match guard(|| ext.resume(self, wait)) {
                    Ok(o) => o,
                    Err(e) => {
                        let _ = guard(|| ext.cancel(self));
                        crate::Outcome::Done(Err(e))
                    }
                };
                self.nesting = outer;
                self.merge_key = merge_key;
                if self.extension.is_none() {
                    self.extension = Some(ext);
                }
                outcome
            }
        };
        match outcome {
            crate::Outcome::Pending => {
                self.job = Some(job);
                None
            }
            crate::Outcome::Done(result) => Some(self.finish(job.begun, result)),
        }
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
