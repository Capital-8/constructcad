//! Bar marks and the check of bar lengths (see `cc_core::bar_marks` and
//! `cc_geom::bar_marks`).

use super::edit;
use crate::error::{EngineError, Result};
use crate::refs::finite;
use crate::registry::{Command, CommandKind};
use crate::session::Session;
use crate::t;
use cc_core::bar_marks::{BarMarking, FrozenMark, MAX_FROZEN, MAX_MARK_TEXT, MAX_START};
use cc_core::{ElementId, ElementKind};
use cc_geom::bar_marks::{self as marks, BarMarks, Mark, MarkSource, to_mark_shape};
use cc_geom::bar_shape::ShapeSummary;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

/// A shape as the queries show it: lengths in meters, turns in degrees.
pub(crate) fn shape_json(s: &ShapeSummary) -> Value {
    let legs: Vec<Value> = s
        .legs
        .iter()
        .map(|l| json!({"length": l.length, "radius": l.radius, "sweep": l.sweep.to_degrees(), "bend": l.bend.to_degrees()}))
        .collect();
    json!({ "closed": s.closed, "legs": legs })
}

/// The marks of the model's bars, with the project's units set for texts.
fn current_marks(s: &mut Session) -> BarMarks {
    cc_views::format::set_units(&s.project().units);
    let (p, g) = s.project_and_geometry();
    marks::resolve(p, &g.rebar)
}

fn source_name(source: MarkSource) -> &'static str {
    match source {
        MarkSource::Frozen(_) => "frozen",
        MarkSource::Override => "override",
        MarkSource::Numbered => "numbered",
    }
}

/// A mark as the queries show it.
pub(crate) fn mark_json(m: &Mark) -> Value {
    json!({
        "mark": m.mark,
        "diameter": m.diameter,
        "grade": m.grade,
        "code": cc_views::bending::shape_code(&m.shape),
        "shape": shape_json(&m.shape),
        "unit_length": m.unit_length(),
        "count": m.count,
        "total_length": m.total_length,
        "weight": m.weight,
        "longest": m.longest,
        "sets": m.sets.iter().map(|id| id.0).collect::<Vec<_>>(),
        "source": source_name(m.source),
        "frozen": m.frozen(),
        "overridden": m.source == MarkSource::Override,
    })
}

fn settings_json(m: &BarMarking) -> Value {
    json!({
        "prefix": m.prefix,
        "start": m.start,
        "length_tolerance": m.length_tolerance,
        "stock_length": m.stock_length,
        "frozen": m.frozen.len(),
    })
}

/// Every bar mark of the model, sorted naturally (N2 before N10): its
/// diameter and grade, its shape (`code`, e.g. `L 30-470`, and the legs),
/// the length of one bar (`unit_length`, the mean of its bars), how many
/// bars have it, their total length and weight, the longest bar, the sets
/// using it, and where it comes from (`source`: `frozen`, `override` or
/// `numbered`). Frozen marks no bar uses are listed with a count of 0.
/// Also the marking settings. A mark names one shape: the same diameter,
/// grade, legs and bends within the length tolerance (default 5 mm)
/// and 1 degree, read from either end or mirrored. Lengths in meters,
/// angles in degrees, weights in kilograms.
#[derive(Debug, Deserialize, Serialize, JsonSchema, Default)]
#[serde(deny_unknown_fields)]
pub struct BarMarksList {}

impl Command for BarMarksList {
    const NAME: &'static str = "bar_marks_list";
    const KIND: CommandKind = CommandKind::Query;
    fn run(self, s: &mut Session) -> Result<Value> {
        let marks = current_marks(s);
        Ok(json!({
            "settings": settings_json(&s.project().bar_marking),
            "marks": marks.marks.iter().map(mark_json).collect::<Vec<_>>(),
        }))
    }
}

/// The frozen entry of a numbered mark; none for a shape too long to keep.
fn frozen_entry(m: &Mark) -> Option<FrozenMark> {
    Some(FrozenMark {
        mark: m.mark.clone(),
        diameter: m.diameter,
        grade: m.grade.clone(),
        shape: to_mark_shape(&m.shape)?,
    })
}

/// Freeze the bar marks: the marks numbered for shapes the frozen marks
/// table does not hold yet are added to it, so they stay when the model
/// changes (entries already there are kept, overrides stay on their sets).
/// With `drop_unused`, entries no bar uses any more are dropped. Changes
/// nothing (and makes no undo step) when there is nothing to freeze or drop.
/// `bar_marks_list` shows the result.
#[derive(Debug, Deserialize, Serialize, JsonSchema, Default)]
#[serde(deny_unknown_fields)]
pub struct BarMarksUpdate {
    /// Drop frozen marks that no bar uses (default false).
    #[serde(default)]
    pub drop_unused: bool,
}

impl Command for BarMarksUpdate {
    const NAME: &'static str = "bar_marks_update";
    const KIND: CommandKind = CommandKind::Edit;
    fn run(self, s: &mut Session) -> Result<Value> {
        let marks = current_marks(s);
        let mut marking = s.project().bar_marking.clone();
        let dropped: Vec<String> = if self.drop_unused {
            let unused: Vec<usize> = marks
                .marks
                .iter()
                .filter_map(|m| match m.source {
                    MarkSource::Frozen(i) if m.count == 0 => Some(i),
                    _ => None,
                })
                .collect();
            let names = unused.iter().filter_map(|i| marking.frozen.get(*i)).map(|f| f.mark.clone()).collect();
            let mut i = 0;
            marking.frozen.retain(|_| {
                i += 1;
                !unused.contains(&(i - 1))
            });
            names
        } else {
            Vec::new()
        };
        let mut added = Vec::new();
        let mut skipped = 0;
        for m in marks.marks.iter().filter(|m| m.source == MarkSource::Numbered) {
            match frozen_entry(m) {
                Some(f) => {
                    added.push(f.mark.clone());
                    marking.frozen.push(f);
                }
                None => skipped += 1,
            }
        }
        if marking.frozen.len() > MAX_FROZEN {
            return Err(EngineError::invalid(format!("the frozen marks table holds at most {MAX_FROZEN} marks")));
        }
        if !added.is_empty() || !dropped.is_empty() {
            edit(s, t!("tx-bar-marks-update"), |e| {
                *e.bar_marking_mut() = marking.clone();
                Ok(())
            })?;
        }
        Ok(json!({
            "frozen": added,
            "dropped": dropped,
            "skipped": skipped,
            "settings": settings_json(&s.project().bar_marking),
        }))
    }
}

/// Number the bar marks afresh: the frozen marks table is cleared, and so
/// are the sets' `bar_mark` overrides unless `keep_overrides`; then every
/// shape is numbered from `start` (default: the current one) with `prefix`
/// (default: the current one) in the stable order (partition, diameter
/// descending, length descending, shape) and the numbering is frozen. One
/// undo step; returns the new marks.
#[derive(Debug, Deserialize, Serialize, JsonSchema, Default)]
#[serde(deny_unknown_fields)]
pub struct BarMarksRenumber {
    /// Text before the numbers (e.g. "N"; at most 40 characters).
    #[serde(default)]
    pub prefix: Option<String>,
    /// First number (0 to 1000000).
    #[serde(default)]
    pub start: Option<u32>,
    /// Keep the marks the sets set themselves (default false: they are cleared).
    #[serde(default)]
    pub keep_overrides: bool,
}

impl Command for BarMarksRenumber {
    const NAME: &'static str = "bar_marks_renumber";
    const KIND: CommandKind = CommandKind::Edit;
    fn run(self, s: &mut Session) -> Result<Value> {
        let mut marking = s.project().bar_marking.clone();
        if let Some(prefix) = &self.prefix {
            if prefix.chars().count() > MAX_MARK_TEXT {
                return Err(EngineError::invalid(format!("the prefix has at most {MAX_MARK_TEXT} characters")));
            }
            marking.prefix = prefix.trim().to_string();
        }
        if let Some(start) = self.start {
            if start > MAX_START {
                return Err(EngineError::invalid(format!("start must be at most {MAX_START}")));
            }
            marking.start = start;
        }
        marking.frozen.clear();
        cc_views::format::set_units(&s.project().units);
        let (p, g) = s.project_and_geometry();
        let fresh = marks::resolve_with(p, &g.rebar, &marking, self.keep_overrides);
        marking.frozen =
            fresh.marks.iter().filter(|m| m.source == MarkSource::Numbered).filter_map(frozen_entry).collect();
        if marking.frozen.len() > MAX_FROZEN {
            return Err(EngineError::invalid(format!("the frozen marks table holds at most {MAX_FROZEN} marks")));
        }
        let overridden: Vec<ElementId> = if self.keep_overrides {
            Vec::new()
        } else {
            p.elements.values().filter(|e| e.rebar_bar_mark().is_some()).map(|e| e.id).collect()
        };
        edit(s, t!("tx-bar-marks-renumber"), |e| {
            *e.bar_marking_mut() = marking.clone();
            for id in &overridden {
                match e.element_mut(*id).map(|x| &mut x.kind) {
                    Some(ElementKind::Rebar(r)) => r.bar_mark = None,
                    Some(ElementKind::ShapedRebar(r)) => r.bar_mark = None,
                    _ => {}
                }
            }
            Ok(())
        })?;
        let marks = current_marks(s);
        Ok(json!({
            "settings": settings_json(&s.project().bar_marking),
            "marks": marks.marks.iter().map(mark_json).collect::<Vec<_>>(),
        }))
    }
}

/// Change how bars are marked: the `prefix` and `start` of numbered marks
/// (used by marks numbered from now on; `bar_marks_renumber` renumbers the
/// frozen ones), the length tolerance within which bars are one shape, and
/// the stock length bars are bought in (`bar_length_check` and bending
/// schedules flag longer bars). One undo step.
#[derive(Debug, Deserialize, Serialize, JsonSchema, Default)]
#[serde(deny_unknown_fields)]
pub struct BarMarkingSet {
    /// Text before the numbers (at most 40 characters).
    #[serde(default)]
    pub prefix: Option<String>,
    /// First number (0 to 1000000).
    #[serde(default)]
    pub start: Option<u32>,
    /// Legs of bars of one mark differ by at most this, meters (0.0001 to 0.05; default 0.005).
    #[serde(default)]
    pub length_tolerance: Option<f64>,
    /// Length bars are bought in, meters (1 to 100; default 12).
    #[serde(default)]
    pub stock_length: Option<f64>,
}

impl Command for BarMarkingSet {
    const NAME: &'static str = "bar_marking_set";
    const KIND: CommandKind = CommandKind::Edit;
    fn run(self, s: &mut Session) -> Result<Value> {
        let mut marking = s.project().bar_marking.clone();
        if let Some(prefix) = &self.prefix {
            marking.prefix = prefix.trim().to_string();
        }
        if let Some(start) = self.start {
            marking.start = start;
        }
        if let Some(t) = self.length_tolerance {
            marking.length_tolerance = finite(t, "length_tolerance")?;
        }
        if let Some(l) = self.stock_length {
            marking.stock_length = finite(l, "stock_length")?;
        }
        if let Some(message) = marking.check() {
            return Err(EngineError::invalid(message));
        }
        edit(s, t!("tx-bar-marking-set"), |e| {
            *e.bar_marking_mut() = marking.clone();
            Ok(())
        })?;
        Ok(settings_json(&s.project().bar_marking))
    }
}

/// Bars longer than `max_length` (default: the project's stock length, 12 m
/// unless `bar_marking_set` changed it): the bars that need splices, listed
/// by set and mark with how many there are and the longest. A set with a
/// `max_length` of its own cuts its bars into lapped pieces, which then pass.
#[derive(Debug, Deserialize, Serialize, JsonSchema, Default)]
#[serde(deny_unknown_fields)]
pub struct BarLengthCheck {
    /// Longest bar allowed, meters (default: the stock length).
    #[serde(default)]
    pub max_length: Option<f64>,
}

impl Command for BarLengthCheck {
    const NAME: &'static str = "bar_length_check";
    const KIND: CommandKind = CommandKind::Query;
    fn run(self, s: &mut Session) -> Result<Value> {
        let max = match self.max_length {
            Some(m) => {
                let m = finite(m, "max_length")?;
                if m <= 0.0 {
                    return Err(EngineError::invalid("max_length must be above 0"));
                }
                m
            }
            None => s.project().bar_marking.stock_length,
        };
        let marks = current_marks(s);
        let (p, g) = s.project_and_geometry();
        let mut out = Vec::new();
        let mut total = 0;
        for (id, set) in &g.rebar {
            // Per mark: the bars over and the longest.
            let mut over: Vec<(String, usize, f64)> = Vec::new();
            for (i, bar) in set.bars.iter().enumerate() {
                let length = bar.length();
                if length <= max + 1e-9 {
                    continue;
                }
                let mark = marks.of_bar(*id, i).map_or(String::new(), |m| m.mark.clone());
                match over.iter_mut().find(|(m, ..)| *m == mark) {
                    Some(o) => {
                        o.1 += 1;
                        o.2 = o.2.max(length);
                    }
                    None => over.push((mark, 1, length)),
                }
            }
            let e = p.element(*id);
            for (mark, count, longest) in over {
                total += count;
                out.push(json!({
                    "set": id.0,
                    "host": set.host.map(|h| h.0),
                    "partition": e.and_then(|e| e.rebar_partition()).unwrap_or(""),
                    "mark": mark,
                    "count": count,
                    "longest": longest,
                }));
            }
        }
        Ok(json!({ "max_length": max, "count": total, "bars": out }))
    }
}
