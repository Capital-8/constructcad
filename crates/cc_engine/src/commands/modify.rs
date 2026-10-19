//! Modifying elements: delete, move, copy/array, rotate, mirror, pin, flip,
//! joins, trim/extend, split, offset, align, end points.

use super::{edit, ids_json};
use crate::error::{EngineError, Result};
use crate::refs::{self, Point, finite, point};
use crate::registry::{Command, CommandKind};
use crate::session::Session;
use crate::t;
use cc_core::{ElementId, ElementKind, PlanTransform, Project};
use glam::DVec2;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

/// Split `ids` into free and pinned elements (pinned ones are not changed).
fn unpinned(p: &Project, ids: Vec<ElementId>) -> (Vec<ElementId>, Vec<ElementId>) {
    ids.into_iter().partition(|id| !p.is_pinned(*id))
}

fn transform(s: &mut Session, ids: &[u64], t: PlanTransform, tx: String) -> Result<Value> {
    let ids = refs::elements(s.project(), ids)?;
    let (free, pinned) = unpinned(s.project(), ids);
    let changed = edit(s, tx, |e| Ok(e.transform_elements(&free, &t)))?;
    Ok(json!({ "changed": ids_json(&changed), "pinned_skipped": ids_json(&pinned) }))
}

fn copy(s: &mut Session, ids: &[u64], transforms: &[PlanTransform], tx: String) -> Result<Value> {
    let ids = refs::elements(s.project(), ids)?;
    let created = edit(s, tx, |e| Ok(e.copy_elements(&ids, transforms)))?;
    Ok(json!({ "created": ids_json(&created) }))
}

/// Delete elements (doors and windows of deleted walls go too; pinned elements stay).
#[derive(Debug, Deserialize, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ElementsDelete {
    pub ids: Vec<u64>,
}

impl Command for ElementsDelete {
    const NAME: &'static str = "elements_delete";
    const KIND: CommandKind = CommandKind::Edit;
    fn run(self, s: &mut Session) -> Result<Value> {
        let ids = refs::elements(s.project(), &self.ids)?;
        let (free, pinned) = unpinned(s.project(), ids);
        edit(s, t!("tx-delete"), |e| {
            for id in &free {
                e.remove_element(*id);
            }
            Ok(())
        })?;
        Ok(json!({ "deleted": ids_json(&free), "pinned_skipped": ids_json(&pinned) }))
    }
}

/// Move elements by `delta` (walls take their doors and windows; doors and
/// windows alone slide along their wall).
#[derive(Debug, Deserialize, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ElementsMove {
    pub ids: Vec<u64>,
    /// Displacement `[dx, dy]` in meters.
    pub delta: Point,
}

impl Command for ElementsMove {
    const NAME: &'static str = "elements_move";
    const KIND: CommandKind = CommandKind::Edit;
    fn run(self, s: &mut Session) -> Result<Value> {
        transform(s, &self.ids, PlanTransform::translate(point(self.delta)?), t!("tx-move"))
    }
}

/// Copy elements by `delta`; with `count` > 1, a linear array of that many
/// copies spaced by `delta`.
#[derive(Debug, Deserialize, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ElementsCopy {
    pub ids: Vec<u64>,
    pub delta: Point,
    /// Number of copies (default 1, at most 1000).
    #[serde(default)]
    pub count: Option<u32>,
}

impl Command for ElementsCopy {
    const NAME: &'static str = "elements_copy";
    const KIND: CommandKind = CommandKind::Edit;
    fn run(self, s: &mut Session) -> Result<Value> {
        let delta = point(self.delta)?;
        let count = self.count.unwrap_or(1).clamp(1, 1000);
        let transforms: Vec<PlanTransform> =
            (1..=count).map(|k| PlanTransform::translate(delta * f64::from(k))).collect();
        let tx = if count > 1 { t!("tx-array") } else { t!("tx-copy") };
        copy(s, &self.ids, &transforms, tx)
    }
}

/// Center of the elements' extents in plan.
fn center_of(s: &mut Session, ids: &[ElementId]) -> Option<DVec2> {
    let mut pts: Vec<DVec2> = Vec::new();
    let geom = s.geometry().clone();
    let p = s.project();
    for id in ids {
        for solid in geom.solids_of(*id) {
            if let Some((lo, hi)) = solid.bounds() {
                pts.extend([lo.truncate(), hi.truncate()]);
            }
        }
        let Some(e) = p.element(*id) else { continue };
        if let Some((a, b)) = cc_core::wall_ops::line_of(e) {
            pts.extend([a, b]);
        }
        if let ElementKind::Room(r) = &e.kind {
            pts.push(r.position);
        }
    }
    let (lo, hi) = pts
        .iter()
        .fold(None, |acc: Option<(DVec2, DVec2)>, q| Some(acc.map_or((*q, *q), |(lo, hi)| (lo.min(*q), hi.max(*q)))))?;
    Some((lo + hi) * 0.5)
}

/// Rotate elements by `angle` degrees (counter-clockwise) about `center`
/// (default: the center of their extents); with `copy`, rotated copies.
#[derive(Debug, Deserialize, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ElementsRotate {
    pub ids: Vec<u64>,
    pub angle: f64,
    #[serde(default)]
    pub center: Option<Point>,
    #[serde(default)]
    pub copy: bool,
}

impl Command for ElementsRotate {
    const NAME: &'static str = "elements_rotate";
    const KIND: CommandKind = CommandKind::Edit;
    fn run(self, s: &mut Session) -> Result<Value> {
        let ids = refs::elements(s.project(), &self.ids)?;
        let center = match self.center {
            Some(c) => point(c)?,
            None => center_of(s, &ids).ok_or_else(|| EngineError::invalid("give a center"))?,
        };
        let t = PlanTransform::rotate_about(center, finite(self.angle, "angle")?.to_radians());
        if self.copy { copy(s, &self.ids, &[t], t!("tx-rotate")) } else { transform(s, &self.ids, t, t!("tx-rotate")) }
    }
}

/// Mirror elements across an axis: two points, or the line of a wall, grid,
/// beam or railing (`axis_element`). Copies by default (`copy: false` moves them).
#[derive(Debug, Deserialize, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ElementsMirror {
    pub ids: Vec<u64>,
    /// Two points of the axis.
    #[serde(default)]
    pub axis: Option<[Point; 2]>,
    /// Id of an element whose line is the axis.
    #[serde(default)]
    pub axis_element: Option<u64>,
    /// Keep the originals and mirror copies (default true).
    #[serde(default)]
    pub copy: Option<bool>,
}

impl Command for ElementsMirror {
    const NAME: &'static str = "elements_mirror";
    const KIND: CommandKind = CommandKind::Edit;
    fn run(self, s: &mut Session) -> Result<Value> {
        let p = s.project();
        let (a, b) = match (self.axis, self.axis_element) {
            (Some([a, b]), _) => (point(a)?, point(b)?),
            (None, Some(id)) => {
                let e = p.element(refs::element(p, id)?).ok_or_else(|| EngineError::not_found("element"))?;
                match &e.kind {
                    ElementKind::Wall(w) => p.wall_axis(e, w),
                    _ => {
                        cc_core::wall_ops::line_of(e).ok_or_else(|| EngineError::invalid("the element has no line"))?
                    }
                }
            }
            (None, None) => return Err(EngineError::invalid("give an axis or an axis_element")),
        };
        if a.distance(b) < 1e-9 {
            return Err(EngineError::invalid("the axis has no length"));
        }
        let t = PlanTransform::mirror_across(a, b - a);
        if self.copy.unwrap_or(true) {
            copy(s, &self.ids, &[t], t!("tx-mirror"))
        } else {
            transform(s, &self.ids, t, t!("tx-mirror"))
        }
    }
}

/// Pin elements (they cannot be moved or deleted) or unpin them.
#[derive(Debug, Deserialize, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ElementsPin {
    pub ids: Vec<u64>,
    /// `true` to pin, `false` to unpin.
    pub pinned: bool,
}

impl Command for ElementsPin {
    const NAME: &'static str = "elements_pin";
    const KIND: CommandKind = CommandKind::Edit;
    fn run(self, s: &mut Session) -> Result<Value> {
        let ids = refs::elements(s.project(), &self.ids)?;
        let tx = if self.pinned { t!("tx-pin") } else { t!("tx-unpin") };
        let changed = edit(s, tx, |e| Ok(e.set_pinned(&ids, self.pinned)))?;
        Ok(json!({ "changed": changed }))
    }
}

/// Flip a door or window to the other side of its wall (`facing`) and/or to the other jamb (`hand`).
#[derive(Debug, Deserialize, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ElementFlip {
    pub id: u64,
    #[serde(default)]
    pub facing: bool,
    #[serde(default)]
    pub hand: bool,
}

impl Command for ElementFlip {
    const NAME: &'static str = "element_flip";
    const KIND: CommandKind = CommandKind::Edit;
    fn run(self, s: &mut Session) -> Result<Value> {
        let id = refs::element(s.project(), self.id)?;
        edit(s, t!("tx-modify"), |e| {
            let h = e
                .element_mut(id)
                .and_then(|x| x.as_hosted_mut())
                .ok_or_else(|| EngineError::invalid("not a door or window"))?;
            h.flip_facing ^= self.facing;
            h.flip_hand ^= self.hand;
            Ok(())
        })?;
        Ok(super::created(s, id))
    }
}

/// Allow or disallow joining at one end of a wall.
#[derive(Debug, Deserialize, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WallJoin {
    pub id: u64,
    /// `start` or `end`.
    pub end: String,
    pub joined: bool,
}

fn at_start(end: &str) -> Result<bool> {
    match end {
        "start" => Ok(true),
        "end" => Ok(false),
        other => Err(EngineError::invalid(format!("end must be 'start' or 'end', not '{other}'"))),
    }
}

impl Command for WallJoin {
    const NAME: &'static str = "wall_join";
    const KIND: CommandKind = CommandKind::Edit;
    fn run(self, s: &mut Session) -> Result<Value> {
        let id = refs::element(s.project(), self.id)?;
        let start = at_start(&self.end)?;
        edit(s, t!("tx-wall-join"), |e| match e.element_mut(id).map(|x| &mut x.kind) {
            Some(ElementKind::Wall(w)) => {
                *(if start { &mut w.join_start } else { &mut w.join_end }) = self.joined;
                Ok(())
            }
            _ => Err(EngineError::invalid("not a wall")),
        })?;
        Ok(super::created(s, id))
    }
}

/// Trim or extend two linear elements (walls, beams, grids) to meet at a
/// corner; each pick point marks the part of its element to keep.
#[derive(Debug, Deserialize, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct TrimCorner {
    pub a: u64,
    pub pick_a: Point,
    pub b: u64,
    pub pick_b: Point,
}

impl Command for TrimCorner {
    const NAME: &'static str = "trim_corner";
    const KIND: CommandKind = CommandKind::Edit;
    fn run(self, s: &mut Session) -> Result<Value> {
        let (a, b) = (refs::element(s.project(), self.a)?, refs::element(s.project(), self.b)?);
        let (pa, pb) = (point(self.pick_a)?, point(self.pick_b)?);
        edit(s, t!("tx-trim"), |e| Ok(e.trim_corner(a, pa, b, pb)?))?;
        Ok(json!({ "changed": [a.0, b.0] }))
    }
}

/// Trim or extend a linear element to the line of a reference element; the
/// pick point marks the part to keep.
#[derive(Debug, Deserialize, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct TrimExtend {
    pub id: u64,
    pub pick: Point,
    /// Id of the element whose line is the boundary.
    pub reference: u64,
}

fn line(s: &Session, id: u64) -> Result<(DVec2, DVec2)> {
    let p = s.project();
    let e = p.element(refs::element(p, id)?).ok_or_else(|| EngineError::not_found("element"))?;
    cc_core::wall_ops::line_of(e).ok_or_else(|| EngineError::invalid(format!("element {id} has no line")))
}

impl Command for TrimExtend {
    const NAME: &'static str = "trim_extend";
    const KIND: CommandKind = CommandKind::Edit;
    fn run(self, s: &mut Session) -> Result<Value> {
        let id = refs::element(s.project(), self.id)?;
        let (r0, r1) = line(s, self.reference)?;
        let pick = point(self.pick)?;
        edit(s, t!("tx-trim"), |e| Ok(e.trim_to(id, pick, r0, r1)?))?;
        Ok(super::created(s, id))
    }
}

/// Split a linear element (wall, beam, grid) at a point; returns the new element.
#[derive(Debug, Deserialize, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ElementSplit {
    pub id: u64,
    pub point: Point,
}

impl Command for ElementSplit {
    const NAME: &'static str = "element_split";
    const KIND: CommandKind = CommandKind::Edit;
    fn run(self, s: &mut Session) -> Result<Value> {
        let id = refs::element(s.project(), self.id)?;
        let at = point(self.point)?;
        let new = edit(s, t!("tx-split"), |e| Ok(e.split(id, at)?))?;
        Ok(json!({ "original": id.0, "created": new.0 }))
    }
}

/// Copy a linear element parallel at `distance` (positive: to the left of its direction).
#[derive(Debug, Deserialize, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ElementOffset {
    pub id: u64,
    pub distance: f64,
}

impl Command for ElementOffset {
    const NAME: &'static str = "element_offset";
    const KIND: CommandKind = CommandKind::Edit;
    fn run(self, s: &mut Session) -> Result<Value> {
        let id = refs::element(s.project(), self.id)?;
        let d = finite(self.distance, "distance")?;
        let new = edit(s, t!("tx-offset"), |e| Ok(e.offset_copy(id, d)?))?;
        Ok(super::created(s, new))
    }
}

/// Move a linear element parallel onto the line of a reference element.
#[derive(Debug, Deserialize, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ElementAlign {
    pub id: u64,
    pub reference: u64,
}

impl Command for ElementAlign {
    const NAME: &'static str = "element_align";
    const KIND: CommandKind = CommandKind::Edit;
    fn run(self, s: &mut Session) -> Result<Value> {
        let id = refs::element(s.project(), self.id)?;
        let (r0, r1) = line(s, self.reference)?;
        edit(s, t!("tx-align"), |e| Ok(e.align_to(id, r0, r1)?))?;
        Ok(super::created(s, id))
    }
}

/// Move one end of a wall, beam or grid (joined wall ends follow).
#[derive(Debug, Deserialize, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ElementMoveEnd {
    pub id: u64,
    /// `start` or `end`.
    pub end: String,
    pub point: Point,
}

impl Command for ElementMoveEnd {
    const NAME: &'static str = "element_move_end";
    const KIND: CommandKind = CommandKind::Edit;
    fn run(self, s: &mut Session) -> Result<Value> {
        let id = refs::element(s.project(), self.id)?;
        let start = at_start(&self.end)?;
        let to = point(self.point)?;
        edit(s, t!("tx-modify"), |e| Ok(e.move_endpoint(id, start, to)?))?;
        Ok(super::created(s, id))
    }
}
