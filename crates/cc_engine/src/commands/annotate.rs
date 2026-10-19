//! Dimensions: place (associated with what they measure) and drive by value.

use super::{add_element, created, edit};
use crate::error::{EngineError, Result};
use crate::refs::{self, Point, Ref, finite, point};
use crate::registry::{Command, CommandKind};
use crate::session::Session;
use crate::t;
use cc_core::{Category, Dimension, DimensionKind, ElementKind};
use glam::DVec2;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Place a dimension in a 2D view (points in the view's drawing coordinates:
/// plan X/Y in plans; horizontal position and height in elevations and
/// sections). Its ends follow the elements (or level lines) their points
/// lie on, so it keeps measuring them. Aligned dimensions measure from `a`
/// to `b`, linear ones along a horizontal or vertical `direction`, angular
/// ones the angle at `vertex` between the rays through `a` and `b`, radial
/// and diameter ones the arc centered at `a` through `b`.
#[derive(Debug, Deserialize, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DimensionCreate {
    /// View id or name.
    pub view: Ref,
    /// First point (center for radial and diameter).
    pub a: Point,
    /// Second point (a point on the arc for radial and diameter).
    pub b: Point,
    /// `aligned` (default), `linear`, `angular`, `radial` or `diameter`.
    #[serde(default)]
    pub kind: Option<String>,
    /// Distance of the dimension line from `a` (left of a→b positive), or the arc radius for angular ones (default 1).
    #[serde(default)]
    pub offset: Option<f64>,
    /// Vertex of an angular dimension.
    #[serde(default)]
    pub vertex: Option<Point>,
    /// Linear dimensions: `horizontal` or `vertical` (default: the larger extent).
    #[serde(default)]
    pub direction: Option<String>,
    /// Dimension type id or name (default: the default dimension type).
    #[serde(default, rename = "type")]
    pub type_ref: Option<Ref>,
}

impl Command for DimensionCreate {
    const NAME: &'static str = "dimension_create";
    const KIND: CommandKind = CommandKind::Edit;
    fn run(self, s: &mut Session) -> Result<Value> {
        let p = s.project();
        let view_id = refs::view(p, &self.view)?;
        let view = p.view(view_id).ok_or_else(|| EngineError::not_found("view"))?;
        if !(view.is_plan() || view.projection().is_some() || matches!(view.kind, cc_core::ViewKind::Legend { .. })) {
            return Err(EngineError::invalid("dimensions go in plans, elevations, sections and legends"));
        }
        let level = view
            .level()
            .or_else(|| p.levels.first().map(|l| l.id))
            .ok_or_else(|| EngineError::rejected("no levels"))?;
        let (a, b) = (point(self.a)?, point(self.b)?);
        let offset = finite(self.offset.unwrap_or(1.0), "offset")?;
        let kind = match self.kind.as_deref().unwrap_or("aligned") {
            "aligned" => DimensionKind::Aligned,
            "linear" => {
                let horizontal = match self.direction.as_deref() {
                    Some("horizontal") => true,
                    Some("vertical") => false,
                    None => (b - a).x.abs() >= (b - a).y.abs(),
                    Some(other) => return Err(EngineError::invalid(format!("unknown direction '{other}'"))),
                };
                DimensionKind::Linear { dir: if horizontal { DVec2::X } else { DVec2::Y } }
            }
            "angular" => {
                let vertex =
                    point(self.vertex.ok_or_else(|| EngineError::invalid("an angular dimension needs a vertex"))?)?;
                DimensionKind::Angular { vertex, rays: Default::default() }
            }
            "radial" => DimensionKind::Radial,
            "diameter" => DimensionKind::Diameter,
            other => return Err(EngineError::invalid(format!("unknown dimension kind '{other}'"))),
        };
        let dim = Dimension { view: view_id, a, b, offset, kind, refs: Default::default() };
        if dim.value().is_nan() || dim.value() <= 1e-6 {
            return Err(EngineError::invalid(t!("msg-dimension-zero")));
        }
        let type_id = refs::type_of(p, Category::Dimension, self.type_ref.as_ref())?;
        let drawing = s.drawing(view_id).unwrap_or_default();
        let dim = crate::dimensions::associate(s.project(), &drawing, &dim);
        let id = edit(s, t!("tx-create-dimension"), |e| {
            Ok(add_element(e, level, type_id, ElementKind::Dimension(Box::new(dim))))
        })?;
        Ok(created(s, id))
    }
}

/// Give a dimension a new value (meters; degrees for angular dimensions):
/// in plans what its moving end measures moves or stretches; between level
/// lines in elevations and sections the upper level moves. Radial and
/// diameter dimensions are read-only.
#[derive(Debug, Deserialize, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DimensionSetValue {
    /// Dimension element id.
    pub id: u64,
    /// New value: meters, or degrees for angular dimensions.
    pub value: f64,
}

impl Command for DimensionSetValue {
    const NAME: &'static str = "dimension_set_value";
    const KIND: CommandKind = CommandKind::Edit;
    fn run(self, s: &mut Session) -> Result<Value> {
        let id = refs::element(s.project(), self.id)?;
        let angular = matches!(
            s.project().element(id).map(|e| &e.kind),
            Some(ElementKind::Dimension(d)) if matches!(d.kind, DimensionKind::Angular { .. })
        );
        let value = if angular { self.value.to_radians() } else { self.value };
        crate::dimensions::set_value(s, id, value)?;
        Ok(created(s, id))
    }
}
