//! Pieces of a component drawn as sketches: an outline on the plan, front
//! or side plane extruded along the plane's normal or revolved about an
//! axis in it, solid or void. Coordinates are expressions, so a reference
//! plane's name locks a point (or the start and end) to that plane.

use super::component::{check_expr, edit_def, number, sketch_plane};
use crate::error::{EngineError, Result};
use crate::refs::{self, Ref};
use crate::registry::{Command, CommandKind};
use crate::session::Session;
use cc_core::component::ProfilePoint;
use cc_core::{ComponentItem, ComponentShape, expr};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

/// Most points of a sketch.
const MAX_SKETCH_POINTS: usize = 1024;

/// A number or an expression of the component.
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize, JsonSchema)]
#[serde(untagged)]
pub enum ExprArg {
    Number(f64),
    Text(String),
}

impl ExprArg {
    fn text(&self) -> Result<String> {
        match self {
            ExprArg::Number(v) => Ok(number(refs::finite(*v, "value")?)),
            ExprArg::Text(t) => check_expr(t),
        }
    }
}

/// A point of a sketch: `u`, `v` in its plane (numbers or expressions; a
/// plane's name locks the coordinate to it), and `radius` to make the
/// segment to the next point an arc (positive bulging outwards).
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SketchPointArg {
    pub u: ExprArg,
    pub v: ExprArg,
    #[serde(default)]
    pub radius: Option<ExprArg>,
}

fn profile_points(points: &[SketchPointArg]) -> Result<Vec<ProfilePoint>> {
    if points.len() < 2 || points.len() > MAX_SKETCH_POINTS {
        return Err(EngineError::invalid(format!("a sketch needs 2 to {MAX_SKETCH_POINTS} points")));
    }
    points
        .iter()
        .map(|p| {
            let radius = p.radius.as_ref().map(ExprArg::text).transpose()?.unwrap_or_default();
            Ok(ProfilePoint { u: p.u.text()?, v: p.v.text()?, radius })
        })
        .collect()
}

/// The depth from `start` to `end`, written plainly when both are simple.
fn depth_expr(start: &str, end: &str) -> String {
    let simple = |e: &str| e.chars().all(|c| expr::is_ident_char(c) || c == '.');
    let wrap = |e: &str| if simple(e) { e.to_string() } else { format!("({e})") };
    if start.trim() == "0" { end.to_string() } else { format!("{} - {}", wrap(end), wrap(start)) }
}

/// Add a piece drawn as a sketch on the `plan`, `front` or `side` plane: an
/// `extrusion` of the closed outline from `start` to `end` along the plane's
/// normal (Z for the plan, Y for the front, X for the side), or a
/// `revolution` about the line u = `axis` of the plane, `angle` degrees
/// (default 360). Coordinates are numbers or expressions; a reference
/// plane's name locks a coordinate (or `start`, `end`, `axis`) to that
/// plane, so the piece follows it. With `void`, the piece is cut out of the
/// solid pieces it overlaps.
#[derive(Debug, Deserialize, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ComponentPieceAdd {
    /// A type of the component: id or name.
    #[serde(rename = "type")]
    pub type_ref: Ref,
    /// `extrusion` (default) or `revolution`.
    #[serde(default)]
    pub kind: Option<String>,
    /// `plan`, `front` or `side`.
    pub plane: String,
    /// The closed outline, in order.
    pub points: Vec<SketchPointArg>,
    /// Where the sketch plane stands along its normal (default 0).
    #[serde(default)]
    pub start: Option<ExprArg>,
    /// Extrusions: where they end along the normal (default 0.1 past `start`).
    #[serde(default)]
    pub end: Option<ExprArg>,
    /// Revolutions: the axis, u = `axis` in the sketch plane.
    #[serde(default)]
    pub axis: Option<ExprArg>,
    /// Revolutions: degrees turned (default 360).
    #[serde(default)]
    pub angle: Option<ExprArg>,
    /// Cut out of the solid pieces it overlaps.
    #[serde(default)]
    pub void: bool,
    /// Name (default `Piece1`, `Piece2`…).
    #[serde(default)]
    pub name: Option<String>,
    /// Project material name.
    #[serde(default)]
    pub material: Option<String>,
}

impl Command for ComponentPieceAdd {
    const NAME: &'static str = "component_piece_add";
    const KIND: CommandKind = CommandKind::Edit;
    fn run(self, s: &mut Session) -> Result<Value> {
        let plane = sketch_plane(&self.plane)?;
        let points = profile_points(&self.points)?;
        let start = self.start.as_ref().map(ExprArg::text).transpose()?.unwrap_or_else(|| "0".into());
        let shape = match self.kind.as_deref().unwrap_or("extrusion") {
            "extrusion" => {
                let depth = match &self.end {
                    Some(end) => depth_expr(&start, &end.text()?),
                    None => "0.1".into(),
                };
                ComponentShape::Profile { plane, points, offset: start, depth }
            }
            "revolution" => {
                let axis = self.axis.as_ref().ok_or_else(|| EngineError::invalid("a revolution needs an axis"))?;
                let angle = self.angle.as_ref().map(ExprArg::text).transpose()?.unwrap_or_else(|| "360".into());
                ComponentShape::Revolution { plane, points, offset: start, axis: axis.text()?, angle }
            }
            other => {
                return Err(EngineError::invalid(format!("unknown piece kind '{other}' (extrusion or revolution)")));
            }
        };
        let index = edit_def(s, &self.type_ref, &[], |def| {
            let name = match self.name.as_deref().map(str::trim) {
                Some(n) if !n.is_empty() => n.to_string(),
                _ => (1..)
                    .map(|i| format!("Piece{i}"))
                    .find(|n| !def.items.iter().any(|it| it.name == *n))
                    .unwrap_or_default(),
            };
            def.items.push(ComponentItem {
                name,
                shape,
                material: self.material.clone().unwrap_or_default(),
                in_plan: true,
                void: self.void,
                plan_only: false,
            });
            Ok(def.items.len() - 1)
        })?;
        Ok(json!({ "index": index }))
    }
}

/// Change a piece of a component: its name, material, whether it is a void
/// or drawn in plans, and for sketched pieces their outline, `start`, `end`
/// (extrusions), `axis` and `angle` (revolutions).
#[derive(Debug, Deserialize, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ComponentPieceSet {
    /// A type of the component: id or name.
    #[serde(rename = "type")]
    pub type_ref: Ref,
    /// The piece's index (from `component_get`).
    pub index: usize,
    /// New name.
    #[serde(default)]
    pub name: Option<String>,
    /// Project material name (empty for the default color).
    #[serde(default)]
    pub material: Option<String>,
    /// Cut out of the solid pieces it overlaps.
    #[serde(default)]
    pub void: Option<bool>,
    /// Drawn in plans.
    #[serde(default)]
    pub in_plan: Option<bool>,
    /// New outline.
    #[serde(default)]
    pub points: Option<Vec<SketchPointArg>>,
    /// Where the sketch plane stands along its normal.
    #[serde(default)]
    pub start: Option<ExprArg>,
    /// Extrusions: where they end along the normal.
    #[serde(default)]
    pub end: Option<ExprArg>,
    /// Revolutions: the axis, u = `axis` in the sketch plane.
    #[serde(default)]
    pub axis: Option<ExprArg>,
    /// Revolutions: degrees turned.
    #[serde(default)]
    pub angle: Option<ExprArg>,
}

impl Command for ComponentPieceSet {
    const NAME: &'static str = "component_piece_set";
    const KIND: CommandKind = CommandKind::Edit;
    fn run(self, s: &mut Session) -> Result<Value> {
        let points = self.points.as_deref().map(profile_points).transpose()?;
        let text = |e: &Option<ExprArg>| e.as_ref().map(ExprArg::text).transpose();
        let (start, end, axis, angle) = (text(&self.start)?, text(&self.end)?, text(&self.axis)?, text(&self.angle)?);
        let sketched = points.is_some() || start.is_some() || end.is_some() || axis.is_some() || angle.is_some();
        edit_def(s, &self.type_ref, &[], |def| {
            let Some(item) = def.items.get_mut(self.index) else {
                return Err(EngineError::not_found(format!("no piece {}", self.index)));
            };
            if let Some(n) = self.name.as_deref().map(str::trim).filter(|n| !n.is_empty()) {
                item.name = n.to_string();
            }
            if let Some(m) = &self.material {
                item.material = m.clone();
            }
            if let Some(v) = self.void {
                item.void = v;
            }
            if let Some(p) = self.in_plan {
                item.in_plan = p;
            }
            match &mut item.shape {
                ComponentShape::Profile { points: pts, offset, depth, .. } => {
                    if axis.is_some() || angle.is_some() {
                        return Err(EngineError::invalid("an extrusion has no axis or angle"));
                    }
                    if let Some(p) = points {
                        *pts = p;
                    }
                    if start.is_some() || end.is_some() {
                        // The other end stays where it was.
                        let old_end = depth_end(offset, depth);
                        let new_start = start.unwrap_or_else(|| offset.clone());
                        *depth = depth_expr(&new_start, &end.unwrap_or(old_end));
                        *offset = new_start;
                    }
                }
                ComponentShape::Revolution { points: pts, offset, axis: ax, angle: an, .. } => {
                    if end.is_some() {
                        return Err(EngineError::invalid("a revolution has no end"));
                    }
                    if let Some(p) = points {
                        *pts = p;
                    }
                    for (field, value) in [(offset, start), (ax, axis), (an, angle)] {
                        if let Some(v) = value {
                            *field = v;
                        }
                    }
                }
                _ if sketched => return Err(EngineError::invalid("this piece is not a sketch")),
                _ => {}
            }
            Ok(())
        })?;
        Ok(json!({ "index": self.index }))
    }
}

/// Where an extrusion ends, as an expression: `start + depth`, read back
/// plainly when the depth was written as `end - start`.
pub fn depth_end(start: &str, depth: &str) -> String {
    if start.trim() == "0" {
        return depth.to_string();
    }
    let suffix = format!(" - {start}");
    let wrapped = format!(" - ({start})");
    if let Some(end) = depth.strip_suffix(&suffix).or_else(|| depth.strip_suffix(&wrapped)) {
        let end = end.trim();
        return end.strip_prefix('(').and_then(|e| e.strip_suffix(')')).unwrap_or(end).to_string();
    }
    format!("({start}) + ({depth})")
}

/// Remove a piece of a component (a component keeps at least one).
#[derive(Debug, Deserialize, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ComponentPieceDelete {
    /// A type of the component: id or name.
    #[serde(rename = "type")]
    pub type_ref: Ref,
    /// The piece's index.
    pub index: usize,
}

impl Command for ComponentPieceDelete {
    const NAME: &'static str = "component_piece_delete";
    const KIND: CommandKind = CommandKind::Edit;
    fn run(self, s: &mut Session) -> Result<Value> {
        edit_def(s, &self.type_ref, &[], |def| {
            if self.index >= def.items.len() {
                return Err(EngineError::not_found(format!("no piece {}", self.index)));
            }
            def.items.remove(self.index);
            Ok(())
        })?;
        Ok(json!({ "deleted": self.index }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn depths_read_back_their_end() {
        assert_eq!(depth_expr("0", "Top"), "Top");
        assert_eq!(depth_expr("Base", "Top"), "Top - Base");
        assert_eq!(depth_expr("Base + 0.1", "Top"), "Top - (Base + 0.1)");
        assert_eq!(depth_end("Base", "Top - Base"), "Top");
        assert_eq!(depth_end("Base + 0.1", "Top - (Base + 0.1)"), "Top");
        assert_eq!(depth_end("0", "0.5"), "0.5");
        assert_eq!(depth_end("0.2", "0.5"), "(0.2) + (0.5)");
    }
}
