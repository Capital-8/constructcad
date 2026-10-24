//! Construction phases: the project's phases, the phases of elements
//! (built, demolished), phase filters and the graphics of each status
//! (see [`cc_core::phase`]).

use super::edit;
use super::graphics::{overrides_json, patched};
use crate::error::{EngineError, Result};
use crate::refs::{self, Ref};
use crate::registry::{Command, CommandKind};
use crate::session::Session;
use crate::t;
use cc_core::{ElementId, Phase, PhaseDisplay, PhaseFilter, PhaseFilterId, PhaseId, PhaseStatus, Project};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

/// Longest name accepted.
const MAX_TEXT: usize = 200;
/// Most phases in a project.
const MAX_PHASES: usize = 100;

fn phase_ref_json(p: &Project, id: Option<PhaseId>) -> Value {
    match id.and_then(|id| p.phase(id)) {
        Some(ph) => json!({"id": ph.id.0, "name": ph.name}),
        None => Value::Null,
    }
}

/// The phases of an element as `element_get` shows them.
pub(crate) fn element_phases_json(p: &Project, e: &cc_core::Element) -> (Value, Value) {
    if !e.category().is_phased() {
        return (Value::Null, Value::Null);
    }
    let created = p.phases.get(p.created_index(e)).map(|ph| ph.id);
    (phase_ref_json(p, created), phase_ref_json(p, e.phase_demolished))
}

/// A view's phase and phase filter as `view_get` shows them.
pub(crate) fn view_phase_json(p: &Project, v: &cc_core::View) -> (Value, Value) {
    if !v.kind.is_phased() {
        return (Value::Null, Value::Null);
    }
    let filter = match v.phase_filter.and_then(|f| p.phase_filter(f)) {
        Some(f) => json!({"id": f.id.0, "name": f.name}),
        None => Value::Null,
    };
    (phase_ref_json(p, p.view_phase(v)), filter)
}

fn phase_json(p: &Project, index: usize, ph: &Phase) -> Value {
    let phased = || p.elements.values().filter(|e| e.category().is_phased());
    json!({
        "id": ph.id.0,
        "name": ph.name,
        "description": ph.description,
        "index": index,
        "created": phased().filter(|e| p.created_index(e) == index).count(),
        "demolished": phased().filter(|e| e.phase_demolished == Some(ph.id)).count(),
        "views": p.views.values().filter(|v| v.kind.is_phased() && p.view_phase(v) == Some(ph.id)).count(),
    })
}

fn phase_name(p: &Project, name: &str, except: Option<PhaseId>) -> Result<String> {
    let name = name.trim();
    if name.is_empty() || name.len() > MAX_TEXT {
        return Err(EngineError::invalid("the name is empty or too long"));
    }
    if p.phases.iter().any(|x| Some(x.id) != except && x.name.eq_ignore_ascii_case(name)) {
        return Err(EngineError::rejected(t!("msg-name-taken", "name" => name)));
    }
    Ok(name.to_string())
}

/// The project's construction phases in time order, with how many
/// elements each builds and demolishes and how many views show it.
#[derive(Debug, Deserialize, Serialize, JsonSchema, Default)]
#[serde(deny_unknown_fields)]
pub struct PhasesList {}

impl Command for PhasesList {
    const NAME: &'static str = "phases_list";
    const KIND: CommandKind = CommandKind::Query;
    fn run(self, s: &mut Session) -> Result<Value> {
        let p = s.project();
        Ok(p.phases.iter().enumerate().map(|(i, ph)| phase_json(p, i, ph)).collect())
    }
}

/// Add a construction phase, at the end (the latest) or before another.
/// Elements and views keep their phases.
#[derive(Debug, Deserialize, Serialize, JsonSchema, Default)]
#[serde(deny_unknown_fields)]
pub struct PhaseCreate {
    /// Name (unique).
    pub name: String,
    /// Description.
    #[serde(default)]
    pub description: Option<String>,
    /// Insert before this phase (id or name; default: after the last).
    #[serde(default)]
    pub before: Option<Ref>,
}

impl Command for PhaseCreate {
    const NAME: &'static str = "phase_create";
    const KIND: CommandKind = CommandKind::Edit;
    fn run(self, s: &mut Session) -> Result<Value> {
        let p = s.project();
        let name = phase_name(p, &self.name, None)?;
        if p.phases.len() >= MAX_PHASES {
            return Err(EngineError::invalid(format!("at most {MAX_PHASES} phases")));
        }
        let at = match &self.before {
            Some(r) => {
                let before = refs::phase(p, r)?;
                p.phase_index(before).unwrap_or(p.phases.len())
            }
            None => p.phases.len(),
        };
        let description = self.description.unwrap_or_default();
        let id = edit(s, t!("tx-phase-create"), |e| {
            let id = PhaseId(e.alloc_id());
            let phases = e.phases_mut();
            phases.insert(at.min(phases.len()), Phase { id, name, description });
            Ok(id)
        })?;
        let p = s.project();
        let i = p.phase_index(id).unwrap_or(0);
        p.phase(id).map(|ph| phase_json(p, i, ph)).ok_or_else(|| EngineError::not_found("phase"))
    }
}

/// Rename a phase, change its description, or move it to another place
/// in time (`index`, 0 = earliest). A move that would put an element's
/// demolition before its construction is refused.
#[derive(Debug, Deserialize, Serialize, JsonSchema, Default)]
#[serde(deny_unknown_fields)]
pub struct PhaseSet {
    /// Phase id or name.
    pub phase: Ref,
    /// New name.
    #[serde(default)]
    pub name: Option<String>,
    /// New description.
    #[serde(default)]
    pub description: Option<String>,
    /// New position in time order (0 = earliest).
    #[serde(default)]
    pub index: Option<usize>,
}

impl Command for PhaseSet {
    const NAME: &'static str = "phase_set";
    const KIND: CommandKind = CommandKind::Edit;
    fn run(self, s: &mut Session) -> Result<Value> {
        let p = s.project();
        let id = refs::phase(p, &self.phase)?;
        let name = self.name.as_deref().map(|n| phase_name(p, n, Some(id))).transpose()?;
        let mut phases = p.phases.clone();
        let from = p.phase_index(id).ok_or_else(|| EngineError::not_found("phase"))?;
        if let Some(to) = self.index {
            let ph = phases.remove(from);
            phases.insert(to.min(phases.len()), ph);
            // Moving must not put a demolition before a construction (nor
            // a hosted element out of its host's time).
            let mut test = p.clone();
            test.phases = phases.clone();
            if !test.phase_fixes().is_empty() {
                return Err(EngineError::rejected(t!("msg-phase-order-conflict")));
            }
        }
        if let Some(ph) = phases.iter_mut().find(|x| x.id == id) {
            if let Some(n) = name {
                ph.name = n;
            }
            if let Some(d) = self.description {
                ph.description = d;
            }
        }
        edit(s, t!("tx-phase-modify"), |e| {
            *e.phases_mut() = phases;
            Ok(())
        })?;
        let p = s.project();
        let i = p.phase_index(id).unwrap_or(0);
        p.phase(id).map(|ph| phase_json(p, i, ph)).ok_or_else(|| EngineError::not_found("phase"))
    }
}

/// Delete a phase. A phase that elements are built or demolished in, or
/// that views show, needs `merge_into`: they move to that phase (an
/// element then built and demolished in the same phase is temporary). The
/// last remaining phase cannot be deleted.
#[derive(Debug, Deserialize, Serialize, JsonSchema, Default)]
#[serde(deny_unknown_fields)]
pub struct PhaseDelete {
    /// Phase id or name.
    pub phase: Ref,
    /// Phase that takes over its elements and views.
    #[serde(default)]
    pub merge_into: Option<Ref>,
}

impl Command for PhaseDelete {
    const NAME: &'static str = "phase_delete";
    const KIND: CommandKind = CommandKind::Edit;
    fn run(self, s: &mut Session) -> Result<Value> {
        let p = s.project();
        let id = refs::phase(p, &self.phase)?;
        if p.phases.len() <= 1 {
            return Err(EngineError::rejected(t!("msg-phase-last")));
        }
        let target = self.merge_into.as_ref().map(|r| refs::phase(p, r)).transpose()?;
        if target == Some(id) {
            return Err(EngineError::invalid("a phase cannot merge into itself"));
        }
        let index = p.phase_index(id).unwrap_or(0);
        // Elements left unassigned read as the last phase: deleting it uses it too.
        let is_last = p.last_phase() == Some(id);
        let built: Vec<ElementId> = p
            .elements
            .values()
            .filter(|e| e.phase_created == id || (is_last && e.phase_created.0 == 0 && e.category().is_phased()))
            .map(|e| e.id)
            .collect();
        let razed: Vec<ElementId> =
            p.elements.values().filter(|e| e.phase_demolished == Some(id)).map(|e| e.id).collect();
        let shows = |v: &cc_core::View| v.phase == id || (is_last && v.phase.0 == 0);
        let views: Vec<cc_core::ViewId> = p.views.values().filter(|v| shows(v)).map(|v| v.id).collect();
        let used = !built.is_empty() || !razed.is_empty() || p.views.values().any(|v| v.kind.is_phased() && shows(v));
        let target = match target {
            Some(t) => t,
            None if used => {
                let name = p.phases.get(index).map(|x| x.name.clone()).unwrap_or_default();
                return Err(EngineError::rejected(t!("msg-phase-in-use", "name" => name)));
            }
            None => PhaseId(0),
        };
        edit(s, t!("tx-phase-delete"), |e| {
            for x in &built {
                if let Some(el) = e.element_mut(*x) {
                    el.phase_created = target;
                }
            }
            for x in &razed {
                if let Some(el) = e.element_mut(*x) {
                    el.phase_demolished = Some(target);
                }
            }
            for v in &views {
                if let Some(view) = e.view_mut(*v) {
                    view.phase = target;
                }
            }
            e.phases_mut().retain(|x| x.id != id);
            Ok(())
        })?;
        Ok(json!({ "deleted": id.0, "merged_into": (target.0 != 0).then_some(target.0) }))
    }
}

fn phase_of(p: &Project, r: &Ref) -> Result<(PhaseId, usize)> {
    let id = refs::phase(p, r)?;
    Ok((id, p.phase_index(id).unwrap_or(0)))
}

fn element_phase_out(s: &Session, ids: &[ElementId]) -> Value {
    let p = s.project();
    ids.iter()
        .filter_map(|id| p.element(*id))
        .map(|e| {
            let (created, demolished) = element_phases_json(p, e);
            json!({"id": e.id.0, "phase_created": created, "phase_demolished": demolished})
        })
        .collect()
}

fn phased_elements(p: &Project, ids: &[u64]) -> Result<Vec<ElementId>> {
    if ids.is_empty() {
        return Err(EngineError::invalid("no elements given"));
    }
    let ids = refs::elements(p, ids)?;
    if ids.iter().any(|id| p.element(*id).is_some_and(|e| !e.category().is_phased())) {
        return Err(EngineError::rejected(t!("msg-phase-annotation")));
    }
    Ok(ids)
}

/// Set the phase elements are built in and/or demolished in (`null`: not
/// demolished). A demolition cannot come before the construction. Doors,
/// windows and rebar follow their hosts: they are never built before them
/// and are demolished with them (unless demolished earlier).
#[derive(Debug, Deserialize, Serialize, JsonSchema, Default)]
#[serde(deny_unknown_fields)]
pub struct ElementsPhaseSet {
    /// Element ids.
    pub ids: Vec<u64>,
    /// Phase built in (id or name).
    #[serde(default)]
    pub created: Option<Ref>,
    /// Phase demolished in (id or name), or `null` to keep it standing.
    #[serde(default, deserialize_with = "refs::nullable", skip_serializing_if = "Option::is_none")]
    pub demolished: Option<Option<Ref>>,
}

impl Command for ElementsPhaseSet {
    const NAME: &'static str = "elements_phase_set";
    const KIND: CommandKind = CommandKind::Edit;
    fn run(self, s: &mut Session) -> Result<Value> {
        let p = s.project();
        let ids = phased_elements(p, &self.ids)?;
        let created = self.created.as_ref().map(|r| phase_of(p, r)).transpose()?;
        let demolished = match &self.demolished {
            Some(Some(r)) => Some(Some(phase_of(p, r)?)),
            Some(None) => Some(None),
            None => None,
        };
        for id in &ids {
            let Some(e) = p.element(*id) else { continue };
            let c = created.map_or(p.created_index(e), |(_, i)| i);
            let d = match demolished {
                Some(d) => d.map(|(_, i)| i),
                None => p.demolished_index(e),
            };
            if d.is_some_and(|d| d < c) {
                return Err(EngineError::rejected(t!("msg-demolish-before-built")));
            }
        }
        edit(s, t!("tx-phase-elements"), |e| {
            for id in &ids {
                if let Some(x) = e.element_mut(*id) {
                    if let Some((c, _)) = created {
                        x.phase_created = c;
                    }
                    if let Some(d) = demolished {
                        x.phase_demolished = d.map(|(d, _)| d);
                    }
                }
            }
            Ok(())
        })?;
        Ok(json!({ "elements": element_phase_out(s, &ids) }))
    }
}

/// Demolish elements in a phase: `phase` when given, else the phase of
/// `view`, else the phase new elements are built in (the last by default).
/// Their doors, windows and rebar go with them (unless demolished
/// earlier). Elements not built by then, or already demolished earlier,
/// are refused.
#[derive(Debug, Deserialize, Serialize, JsonSchema, Default)]
#[serde(deny_unknown_fields)]
pub struct ElementsDemolish {
    /// Element ids.
    pub ids: Vec<u64>,
    /// Phase id or name.
    #[serde(default)]
    pub phase: Option<Ref>,
    /// Or the view whose phase to use.
    #[serde(default)]
    pub view: Option<Ref>,
}

impl Command for ElementsDemolish {
    const NAME: &'static str = "elements_demolish";
    const KIND: CommandKind = CommandKind::Edit;
    fn run(self, s: &mut Session) -> Result<Value> {
        let p = s.project();
        let ids = phased_elements(p, &self.ids)?;
        let phase = match (&self.phase, &self.view) {
            (Some(r), _) => refs::phase(p, r)?,
            (None, Some(v)) => {
                let v = refs::view(p, v)?;
                p.view(v).and_then(|v| p.view_phase(v)).ok_or_else(|| EngineError::not_found("phase"))?
            }
            (None, None) => s
                .doc
                .creation_phase
                .filter(|c| p.phase(*c).is_some())
                .or(p.last_phase())
                .ok_or_else(|| EngineError::not_found("phase"))?,
        };
        let index = p.phase_index(phase).unwrap_or(0);
        for id in &ids {
            let Some(e) = p.element(*id) else { continue };
            if p.created_index(e) > index || p.demolished_index(e).is_some_and(|d| d < index) {
                return Err(EngineError::rejected(t!("msg-demolish-before-built")));
            }
        }
        let before: Vec<(ElementId, Option<PhaseId>)> =
            p.elements.values().map(|e| (e.id, e.phase_demolished)).collect();
        edit(s, t!("tx-demolish"), |e| {
            for id in &ids {
                if let Some(x) = e.element_mut(*id) {
                    x.phase_demolished = Some(phase);
                }
            }
            Ok(())
        })?;
        // What changed, hosted elements included.
        let p = s.project();
        let changed: Vec<ElementId> = before
            .into_iter()
            .filter(|(id, d)| p.element(*id).is_some_and(|e| e.phase_demolished != *d))
            .map(|(id, _)| id)
            .collect();
        Ok(json!({ "phase": phase.0, "demolished": changed.iter().map(|i| i.0).collect::<Vec<_>>() }))
    }
}

/// Set the phase new elements are built in by this session (the active
/// view's phase in the application): display state, not saved, not undone.
/// `phase` (id or name), or the phase of `view`; neither resets it to the
/// last phase.
#[derive(Debug, Deserialize, Serialize, JsonSchema, Default)]
#[serde(deny_unknown_fields)]
pub struct PhaseCreationSet {
    /// Phase id or name.
    #[serde(default)]
    pub phase: Option<Ref>,
    /// Or the view whose phase to use.
    #[serde(default)]
    pub view: Option<Ref>,
}

impl Command for PhaseCreationSet {
    const NAME: &'static str = "phase_creation_set";
    const KIND: CommandKind = CommandKind::Query;
    fn run(self, s: &mut Session) -> Result<Value> {
        let p = s.project();
        let phase = match (&self.phase, &self.view) {
            (Some(r), _) => Some(refs::phase(p, r)?),
            (None, Some(v)) => {
                let v = refs::view(p, v)?;
                p.view(v).and_then(|v| p.view_phase(v))
            }
            (None, None) => None,
        };
        s.doc.creation_phase = phase;
        let p = s.project();
        Ok(json!({ "creation_phase": phase_ref_json(p, phase.or(p.last_phase())) }))
    }
}

// ---------------------------------------------------------------- filters

fn display(name: &str) -> Result<PhaseDisplay> {
    match name.trim().to_ascii_lowercase().replace([' ', '-'], "_").as_str() {
        "not_displayed" | "hidden" | "none" => Ok(PhaseDisplay::NotDisplayed),
        "by_category" | "category" => Ok(PhaseDisplay::ByCategory),
        "overridden" | "override" => Ok(PhaseDisplay::Overridden),
        other => Err(EngineError::invalid(format!("unknown phase display '{other}'"))),
    }
}

pub fn display_name(d: PhaseDisplay) -> &'static str {
    match d {
        PhaseDisplay::NotDisplayed => "not_displayed",
        PhaseDisplay::ByCategory => "by_category",
        PhaseDisplay::Overridden => "overridden",
    }
}

pub fn status_name(st: PhaseStatus) -> &'static str {
    match st {
        PhaseStatus::Existing => "existing",
        PhaseStatus::Demolished => "demolished",
        PhaseStatus::New => "new",
        PhaseStatus::Temporary => "temporary",
    }
}

fn status(name: &str) -> Result<PhaseStatus> {
    PhaseStatus::ALL
        .into_iter()
        .find(|st| status_name(*st) == name.trim().to_ascii_lowercase())
        .ok_or_else(|| EngineError::invalid(format!("unknown phase status '{name}'")))
}

pub(crate) fn phase_filter_json(p: &Project, f: &PhaseFilter) -> Value {
    json!({
        "id": f.id.0,
        "name": f.name,
        "existing": display_name(f.existing),
        "demolished": display_name(f.demolished),
        "new": display_name(f.new),
        "temporary": display_name(f.temporary),
        "views": p.views.values().filter(|v| v.phase_filter == Some(f.id)).count(),
    })
}

fn filter_name(p: &Project, name: &str, except: Option<PhaseFilterId>) -> Result<String> {
    let name = name.trim();
    if name.is_empty() || name.len() > MAX_TEXT {
        return Err(EngineError::invalid("the name is empty or too long"));
    }
    if p.phase_filters.values().any(|f| Some(f.id) != except && f.name.eq_ignore_ascii_case(name)) {
        return Err(EngineError::rejected(t!("msg-name-taken", "name" => name)));
    }
    Ok(name.to_string())
}

/// The phase filters: for each status (`existing`, `demolished`, `new`,
/// `temporary`) whether views using the filter show it `by_category`,
/// `overridden` (with the status graphics, see `phase_graphics_get`) or
/// `not_displayed`.
#[derive(Debug, Deserialize, Serialize, JsonSchema, Default)]
#[serde(deny_unknown_fields)]
pub struct PhaseFiltersList {}

impl Command for PhaseFiltersList {
    const NAME: &'static str = "phase_filters_list";
    const KIND: CommandKind = CommandKind::Query;
    fn run(self, s: &mut Session) -> Result<Value> {
        let p = s.project();
        Ok(p.phase_filters.values().map(|f| phase_filter_json(p, f)).collect())
    }
}

/// Create a phase filter; statuses not given show `by_category`.
#[derive(Debug, Deserialize, Serialize, JsonSchema, Default)]
#[serde(deny_unknown_fields)]
pub struct PhaseFilterCreate {
    /// Name (unique).
    pub name: String,
    /// Existing elements: `by_category`, `overridden` or `not_displayed`.
    #[serde(default)]
    pub existing: Option<String>,
    /// Demolished elements.
    #[serde(default)]
    pub demolished: Option<String>,
    /// New elements.
    #[serde(default)]
    pub new: Option<String>,
    /// Temporary elements.
    #[serde(default)]
    pub temporary: Option<String>,
}

fn apply_displays(f: &mut PhaseFilter, values: [&Option<String>; 4]) -> Result<()> {
    for (st, v) in PhaseStatus::ALL.into_iter().zip(values) {
        if let Some(v) = v {
            f.set_display(st, display(v)?);
        }
    }
    Ok(())
}

impl Command for PhaseFilterCreate {
    const NAME: &'static str = "phase_filter_create";
    const KIND: CommandKind = CommandKind::Edit;
    fn run(self, s: &mut Session) -> Result<Value> {
        let p = s.project();
        let name = filter_name(p, &self.name, None)?;
        let mut f = PhaseFilter {
            id: PhaseFilterId(0),
            name,
            existing: PhaseDisplay::ByCategory,
            demolished: PhaseDisplay::ByCategory,
            new: PhaseDisplay::ByCategory,
            temporary: PhaseDisplay::ByCategory,
        };
        apply_displays(&mut f, [&self.existing, &self.demolished, &self.new, &self.temporary])?;
        let id = edit(s, t!("tx-phase-filter-create"), |e| {
            let id = PhaseFilterId(e.alloc_id());
            e.phase_filters_mut().insert(id, PhaseFilter { id, ..f });
            Ok(id)
        })?;
        let p = s.project();
        p.phase_filter(id).map(|f| phase_filter_json(p, f)).ok_or_else(|| EngineError::not_found("phase filter"))
    }
}

/// Change a phase filter: its name or how it shows each status. Views
/// using it follow.
#[derive(Debug, Deserialize, Serialize, JsonSchema, Default)]
#[serde(deny_unknown_fields)]
pub struct PhaseFilterSet {
    /// Phase filter id or name.
    pub filter: Ref,
    /// New name.
    #[serde(default)]
    pub name: Option<String>,
    /// Existing elements: `by_category`, `overridden` or `not_displayed`.
    #[serde(default)]
    pub existing: Option<String>,
    /// Demolished elements.
    #[serde(default)]
    pub demolished: Option<String>,
    /// New elements.
    #[serde(default)]
    pub new: Option<String>,
    /// Temporary elements.
    #[serde(default)]
    pub temporary: Option<String>,
}

impl Command for PhaseFilterSet {
    const NAME: &'static str = "phase_filter_set";
    const KIND: CommandKind = CommandKind::Edit;
    fn run(self, s: &mut Session) -> Result<Value> {
        let p = s.project();
        let id = refs::phase_filter(p, &self.filter)?;
        let mut f = p.phase_filter(id).cloned().ok_or_else(|| EngineError::not_found("phase filter"))?;
        if let Some(n) = &self.name {
            f.name = filter_name(p, n, Some(id))?;
        }
        apply_displays(&mut f, [&self.existing, &self.demolished, &self.new, &self.temporary])?;
        edit(s, t!("tx-phase-filter-modify"), |e| {
            e.phase_filters_mut().insert(id, f);
            Ok(())
        })?;
        let p = s.project();
        p.phase_filter(id).map(|f| phase_filter_json(p, f)).ok_or_else(|| EngineError::not_found("phase filter"))
    }
}

/// Delete a phase filter; views and view templates using it are left
/// without one (showing every status by category).
#[derive(Debug, Deserialize, Serialize, JsonSchema, Default)]
#[serde(deny_unknown_fields)]
pub struct PhaseFilterDelete {
    /// Phase filter id or name.
    pub filter: Ref,
}

impl Command for PhaseFilterDelete {
    const NAME: &'static str = "phase_filter_delete";
    const KIND: CommandKind = CommandKind::Edit;
    fn run(self, s: &mut Session) -> Result<Value> {
        let p = s.project();
        let id = refs::phase_filter(p, &self.filter)?;
        let views: Vec<cc_core::ViewId> =
            p.views.values().filter(|v| v.phase_filter == Some(id)).map(|v| v.id).collect();
        edit(s, t!("tx-phase-filter-delete"), |e| {
            for t in e.templates_mut().values_mut().filter(|t| t.phase_filter == Some(id)) {
                t.phase_filter = None;
            }
            for v in views {
                if let Some(view) = e.view_mut(v) {
                    view.phase_filter = None;
                }
            }
            e.phase_filters_mut().remove(&id);
            Ok(())
        })?;
        Ok(json!({ "deleted": id.0 }))
    }
}

// ---------------------------------------------------------------- graphics

fn phase_graphics_json(p: &Project) -> Value {
    PhaseStatus::ALL
        .into_iter()
        .map(|st| (status_name(st).to_string(), overrides_json(p.phase_graphics.get(st))))
        .collect::<serde_json::Map<_, _>>()
        .into()
}

/// The graphics of each phase status (`existing`, `demolished`, `new`,
/// `temporary`), used by phase filters that show a status `overridden`.
/// Overrides are shaped as in `view_graphics_get`.
#[derive(Debug, Deserialize, Serialize, JsonSchema, Default)]
#[serde(deny_unknown_fields)]
pub struct PhaseGraphicsGet {}

impl Command for PhaseGraphicsGet {
    const NAME: &'static str = "phase_graphics_get";
    const KIND: CommandKind = CommandKind::Query;
    fn run(self, s: &mut Session) -> Result<Value> {
        Ok(phase_graphics_json(s.project()))
    }
}

/// Change the graphics of a phase status: `overrides` is merged into the
/// current ones (`null` resets an attribute; see `view_graphics_get`),
/// `reset` clears them first.
#[derive(Debug, Deserialize, Serialize, JsonSchema, Default)]
#[serde(deny_unknown_fields)]
pub struct PhaseGraphicsSet {
    /// `existing`, `demolished`, `new` or `temporary`.
    pub status: String,
    /// Overrides to merge.
    #[serde(default)]
    pub overrides: Option<Value>,
    /// Clear the status overrides before merging.
    #[serde(default)]
    pub reset: bool,
}

impl Command for PhaseGraphicsSet {
    const NAME: &'static str = "phase_graphics_set";
    const KIND: CommandKind = CommandKind::Edit;
    fn run(self, s: &mut Session) -> Result<Value> {
        let st = status(&self.status)?;
        let current = *s.project().phase_graphics.get(st);
        let new = patched(&current, self.overrides.as_ref(), self.reset)?;
        edit(s, t!("tx-phase-graphics"), |e| {
            *e.phase_graphics_mut().get_mut(st) = new;
            Ok(())
        })?;
        Ok(phase_graphics_json(s.project()))
    }
}
