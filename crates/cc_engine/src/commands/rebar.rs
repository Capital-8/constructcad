//! Reinforcement: sets of bars in beams and columns.

use super::{add_element, created, edit, ids_json};
use crate::error::{EngineError, Result};
use crate::refs::{self, Ref, finite};
use crate::registry::{Command, CommandKind};
use crate::session::Session;
use crate::t;
use cc_core::{
    BarFace, Category, ColumnSection, ElementId, ElementKind, ElementTypeId, Project, Rebar, RebarLayout, TypeData,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

/// Default concrete cover, meters.
const COVER: f64 = 0.025;

/// The bar type nearest `diameter`.
fn bar_type(p: &Project, diameter: f64) -> Result<ElementTypeId> {
    p.types_of(Category::Rebar)
        .filter_map(|t| match t.data {
            TypeData::Rebar { diameter: d, .. } => Some((t.id, (d - diameter).abs())),
            _ => None,
        })
        .min_by(|a, b| a.1.total_cmp(&b.1))
        .map(|(id, _)| id)
        .ok_or_else(|| EngineError::rejected(t!("msg-no-rebar-type")))
}

/// The host of a set: a beam or a column.
fn host(p: &Project, id: u64) -> Result<(ElementId, bool)> {
    let id = refs::element(p, id)?;
    match p.element(id).map(|e| &e.kind) {
        Some(ElementKind::Beam(_)) => Ok((id, true)),
        Some(ElementKind::Column(_)) => Ok((id, false)),
        _ => Err(EngineError::invalid(t!("msg-rebar-host"))),
    }
}

fn rebar_kind(host: ElementId, layout: RebarLayout, cover: f64) -> ElementKind {
    ElementKind::Rebar(Rebar { host, layout, cover })
}

/// Add a set of bars to a beam or column: `layout` is `top` or `bottom`
/// (beams), `perimeter` (columns) with `count` bars, or `stirrups` every
/// `spacing`. The bars are `cover` inside the faces (default 2.5 cm) and
/// follow the host.
#[derive(Debug, Deserialize, Serialize, JsonSchema, Default)]
#[serde(deny_unknown_fields)]
pub struct RebarCreate {
    /// Beam or column id.
    pub host: u64,
    /// `top`, `bottom`, `perimeter` or `stirrups`.
    pub layout: String,
    /// Number of longitudinal bars (default: 2 in beams, 4 in columns).
    #[serde(default)]
    pub count: Option<u32>,
    /// Stirrup spacing, meters (default 0.15).
    #[serde(default)]
    pub spacing: Option<f64>,
    /// Bar type id or name (default: the type nearest 10 mm, or 5 mm for stirrups).
    #[serde(default, rename = "type")]
    pub type_ref: Option<Ref>,
    /// Concrete cover, meters (default 0.025).
    #[serde(default)]
    pub cover: Option<f64>,
}

impl Command for RebarCreate {
    const NAME: &'static str = "rebar_create";
    const KIND: CommandKind = CommandKind::Edit;
    fn run(self, s: &mut Session) -> Result<Value> {
        let p = s.project();
        let (host_id, beam) = host(p, self.host)?;
        let level = p.element(host_id).map(|e| e.level).ok_or_else(|| EngineError::not_found("host"))?;
        let count = self.count.unwrap_or(if beam { 2 } else { 4 });
        if !(1..=200).contains(&count) {
            return Err(EngineError::invalid("count must be between 1 and 200"));
        }
        let longitudinal = |face| RebarLayout::Longitudinal { face, count };
        let layout = match self.layout.as_str() {
            "top" => longitudinal(BarFace::Top),
            "bottom" => longitudinal(BarFace::Bottom),
            "perimeter" => longitudinal(BarFace::Perimeter),
            "stirrups" => {
                let spacing = finite(self.spacing.unwrap_or(0.15), "spacing")?;
                if spacing < 0.02 {
                    return Err(EngineError::invalid("spacing must be at least 0.02"));
                }
                RebarLayout::Stirrups { spacing }
            }
            other => {
                return Err(EngineError::invalid(format!(
                    "layout must be top, bottom, perimeter or stirrups, not '{other}'"
                )));
            }
        };
        let stirrups = matches!(layout, RebarLayout::Stirrups { .. });
        let type_id = match &self.type_ref {
            Some(r) => refs::type_of(p, Category::Rebar, Some(r))?,
            None => bar_type(p, if stirrups { 0.005 } else { 0.010 })?,
        };
        let cover = finite(self.cover.unwrap_or(COVER), "cover")?.max(0.0);
        let kind = rebar_kind(host_id, layout, cover);
        let id = edit(s, t!("tx-create-rebar"), |e| Ok(add_element(e, level, type_id, kind)))?;
        Ok(created(s, id))
    }
}

/// Give beams and columns without reinforcement a typical one: beams two
/// (three when 20 cm wide or more) 10 mm bars at the bottom, two 8 mm at the
/// top and 5 mm stirrups every 15 cm; columns four 10 mm bars (six in round
/// ones) and 5 mm ties every 15 cm. Default: every beam and column of
/// `level` (or of the project).
#[derive(Debug, Deserialize, Serialize, JsonSchema, Default)]
#[serde(deny_unknown_fields)]
pub struct RebarTypical {
    /// Beam and column ids (default: all of `level`, or of the project).
    #[serde(default)]
    pub elements: Vec<u64>,
    /// Level id or name, when no elements are given.
    #[serde(default)]
    pub level: Option<Ref>,
}

impl Command for RebarTypical {
    const NAME: &'static str = "rebar_typical";
    const KIND: CommandKind = CommandKind::Edit;
    fn run(self, s: &mut Session) -> Result<Value> {
        let p = s.project();
        let hosts: Vec<(ElementId, bool)> = if self.elements.is_empty() {
            let level = self.level.as_ref().map(|r| refs::level(p, r)).transpose()?;
            p.elements
                .values()
                .filter(|e| level.is_none_or(|l| e.level == l))
                .filter_map(|e| match e.kind {
                    ElementKind::Beam(_) => Some((e.id, true)),
                    ElementKind::Column(_) => Some((e.id, false)),
                    _ => None,
                })
                .collect()
        } else {
            self.elements.iter().map(|id| host(p, *id)).collect::<Result<_>>()?
        };
        let (main, top, ties) = (bar_type(p, 0.010)?, bar_type(p, 0.008)?, bar_type(p, 0.005)?);
        let mut todo = Vec::new();
        let mut skipped = 0;
        for (id, beam) in hosts {
            let Some(e) = p.element(id) else { continue };
            if p.rebar_of(id).next().is_some() {
                skipped += 1;
                continue;
            }
            let stirrups = RebarLayout::Stirrups { spacing: 0.15 };
            if beam {
                let wide = matches!(p.element_type(e.type_id).map(|t| &t.data), Some(TypeData::Beam { width, .. }) if *width >= 0.2 - 1e-9);
                let bottom = RebarLayout::Longitudinal { face: BarFace::Bottom, count: if wide { 3 } else { 2 } };
                todo.push((e.level, main, rebar_kind(id, bottom, COVER)));
                todo.push((
                    e.level,
                    top,
                    rebar_kind(id, RebarLayout::Longitudinal { face: BarFace::Top, count: 2 }, COVER),
                ));
            } else {
                let round = matches!(
                    p.element_type(e.type_id).map(|t| &t.data),
                    Some(TypeData::Column { section: ColumnSection::Round { .. } | ColumnSection::Pipe { .. }, .. })
                );
                let bars = RebarLayout::Longitudinal { face: BarFace::Perimeter, count: if round { 6 } else { 4 } };
                todo.push((e.level, main, rebar_kind(id, bars, COVER)));
            }
            todo.push((e.level, ties, rebar_kind(id, stirrups, COVER)));
        }
        if todo.is_empty() {
            return Ok(json!({"created": [], "skipped": skipped}));
        }
        let created = edit(s, t!("tx-create-rebar"), |e| {
            Ok(todo.into_iter().map(|(level, ty, kind)| add_element(e, level, ty, kind)).collect::<Vec<_>>())
        })?;
        Ok(json!({"created": ids_json(&created), "skipped": skipped}))
    }
}
