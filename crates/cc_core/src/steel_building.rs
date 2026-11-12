//! Parametric steel buildings: a host element that holds the parameters of
//! a steel shed (span, eave height, roof slope and shape, bays, the types of
//! its members, purlins and girts) and owns ordinary columns and beams made
//! from them. Every edit keeps the members in step (see
//! [`crate::EditCtx`]): a changed building rewrites its members in place,
//! keeping their ids while their roles exist; a member the user edited or
//! deleted is detached, and the building leaves its role alone from then on.
//!
//! Local coordinates: x across the span (from the first column line), y
//! along the building (frame `k` at `k * bay_spacing`), mapped to plan
//! through `origin` and `rotation_deg`.

use crate::{
    Beam, Category, Column, ElementId, ElementKind, ElementType, ElementTypeId, PlanTransform, Project, TypeData,
    WallTop,
};
use glam::DVec2;
use serde::{Deserialize, Serialize};
use std::ops::RangeInclusive;

/// Most members one building makes; the layout stops there.
pub const MAX_MEMBERS: usize = 4000;
/// Most purlin lines on a roof plane and girt rows on a side.
const MAX_LINES: usize = 999;
/// The width and depth of a type whose section cannot be read, meters.
pub const MISSING_SECTION: f64 = 0.2;

pub const SPAN: RangeInclusive<f64> = 2.0..=100.0;
pub const EAVE_HEIGHT: RangeInclusive<f64> = 1.0..=50.0;
pub const SLOPE: RangeInclusive<f64> = 0.0..=100.0;
pub const BAYS: RangeInclusive<u32> = 1..=100;
pub const BAY_SPACING: RangeInclusive<f64> = 1.0..=30.0;
/// Purlin and girt spacing.
pub const SPACING: RangeInclusive<f64> = 0.3..=10.0;

pub const DEFAULT_SPAN: f64 = 20.0;
pub const DEFAULT_EAVE_HEIGHT: f64 = 6.0;
pub const DEFAULT_SLOPE: f64 = 10.0;
pub const DEFAULT_BAYS: u32 = 5;
pub const DEFAULT_BAY_SPACING: f64 = 6.0;
pub const DEFAULT_SECONDARY_SPACING: f64 = 1.5;

/// The roof of the frames: two rafters meeting at mid-span, or one rising
/// toward `x = span`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub enum FrameRoof {
    #[default]
    Gable,
    Mono,
}

impl FrameRoof {
    pub const ALL: [FrameRoof; 2] = [FrameRoof::Gable, FrameRoof::Mono];

    pub fn code(self) -> &'static str {
        match self {
            FrameRoof::Gable => "gable",
            FrameRoof::Mono => "mono",
        }
    }

    pub fn from_code(code: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|r| r.code().eq_ignore_ascii_case(code.trim()))
    }
}

/// What a member is in its building.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub enum RoleKind {
    Column,
    Rafter,
    Purlin,
    Girt,
}

/// A member's place: its kind, its frame (columns, rafters) or bay
/// (purlins, girts), and its index there (girts: `side * 1000 + row`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct MemberRole {
    pub kind: RoleKind,
    pub frame: u16,
    pub index: u16,
}

impl MemberRole {
    /// Whether an element of `kind` can fill this role: a column for
    /// columns, a beam for the rest.
    pub fn fits(self, kind: &ElementKind) -> bool {
        match self.kind {
            RoleKind::Column => matches!(kind, ElementKind::Column(_)),
            RoleKind::Rafter | RoleKind::Purlin | RoleKind::Girt => matches!(kind, ElementKind::Beam(_)),
        }
    }
}

/// Purlins or girts: their beam type and the distance between lines
/// (purlins: along the rafter's plan run; girts: up the wall).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Secondary {
    pub type_id: ElementTypeId,
    pub spacing: f64,
}

/// A member the building manages.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct FrameMember {
    pub role: MemberRole,
    pub id: ElementId,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SteelBuilding {
    /// Plan point of the first frame's left column.
    pub origin: DVec2,
    /// Turn of the local x axis (across the span), degrees counter-clockwise.
    #[serde(default)]
    pub rotation_deg: f64,
    /// Column axis to column axis, meters.
    pub span: f64,
    /// Column top above the level (the rafters' underside at the column axis), meters.
    pub eave_height: f64,
    /// Roof slope, percent.
    pub slope_percent: f64,
    #[serde(default)]
    pub roof: FrameRoof,
    /// Bays between frames (frames = bays + 1).
    pub bays: u32,
    /// Frame to frame, meters.
    pub bay_spacing: f64,
    pub column_type: ElementTypeId,
    pub rafter_type: ElementTypeId,
    #[serde(default)]
    pub purlins: Option<Secondary>,
    #[serde(default)]
    pub girts: Option<Secondary>,
    /// The members it manages, by role.
    #[serde(default)]
    pub members: Vec<FrameMember>,
    /// Roles the user took over (edited or deleted members): not made again.
    #[serde(default)]
    pub detached: Vec<MemberRole>,
}

fn within(name: &str, v: f64, range: &RangeInclusive<f64>) -> Result<(), String> {
    if v.is_finite() && range.contains(&v) {
        Ok(())
    } else {
        Err(format!("{name} must be between {} and {}", range.start(), range.end()))
    }
}

/// Reset `v` to `default` when it is out of `range`; returns 1 when it did.
fn reset(v: &mut f64, range: &RangeInclusive<f64>, default: f64) -> usize {
    if v.is_finite() && range.contains(v) {
        0
    } else {
        *v = default;
        1
    }
}

/// The width (across) and depth (up) of a column or beam type's section
/// box, [`MISSING_SECTION`] each when it cannot be read.
pub fn section_box(project: &Project, type_id: ElementTypeId) -> (f64, f64) {
    let size = project.element_type(type_id).and_then(|t| project.type_section(&t.data)).map(|s| {
        let (lo, hi) = s.outline.bbox();
        (hi.x - lo.x, hi.y - lo.y)
    });
    match size {
        Some((w, d)) if w.is_finite() && d.is_finite() && w > 0.0 && d > 0.0 => (w, d),
        _ => (MISSING_SECTION, MISSING_SECTION),
    }
}

/// The type a project gets with its first steel building.
pub fn host_type(id: ElementTypeId, name: String) -> ElementType {
    ElementType { id, name, data: TypeData::SteelBuilding {}, info: Default::default(), uid: Default::default() }
}

/// Members collected by a layout, stopping at [`MAX_MEMBERS`].
struct Members(Vec<(MemberRole, ElementTypeId, ElementKind)>);

impl Members {
    fn full(&self) -> bool {
        self.0.len() >= MAX_MEMBERS
    }

    fn push(&mut self, kind: RoleKind, frame: u32, index: usize, type_id: ElementTypeId, element: ElementKind) {
        let (Ok(frame), Ok(index)) = (u16::try_from(frame), u16::try_from(index)) else { return };
        if !self.full() {
            self.0.push((MemberRole { kind, frame, index }, type_id, element));
        }
    }
}

impl SteelBuilding {
    /// A building with the default parameters at the origin; `secondary`
    /// gives purlins and girts of that beam type.
    pub fn new(column_type: ElementTypeId, rafter_type: ElementTypeId, secondary: Option<ElementTypeId>) -> Self {
        let line = secondary.map(|type_id| Secondary { type_id, spacing: DEFAULT_SECONDARY_SPACING });
        Self {
            origin: DVec2::ZERO,
            rotation_deg: 0.0,
            span: DEFAULT_SPAN,
            eave_height: DEFAULT_EAVE_HEIGHT,
            slope_percent: DEFAULT_SLOPE,
            roof: FrameRoof::Gable,
            bays: DEFAULT_BAYS,
            bay_spacing: DEFAULT_BAY_SPACING,
            column_type,
            rafter_type,
            purlins: line,
            girts: line,
            members: Vec::new(),
            detached: Vec::new(),
        }
    }

    /// Whether every number is in its range (the first one that is not, named).
    pub fn check(&self) -> Result<(), String> {
        if !(self.origin.is_finite() && self.rotation_deg.is_finite()) {
            return Err("origin and rotation must be finite numbers".into());
        }
        within("span", self.span, &SPAN)?;
        within("eave_height", self.eave_height, &EAVE_HEIGHT)?;
        within("slope_percent", self.slope_percent, &SLOPE)?;
        within("bay_spacing", self.bay_spacing, &BAY_SPACING)?;
        if !BAYS.contains(&self.bays) {
            return Err(format!("bays must be between {} and {}", BAYS.start(), BAYS.end()));
        }
        for (name, line) in [("purlin spacing", &self.purlins), ("girt spacing", &self.girts)] {
            if let Some(s) = line {
                within(name, s.spacing, &SPACING)?;
            }
        }
        Ok(())
    }

    /// Whether the column type is a column type and the others beam types.
    pub fn check_types(&self, project: &Project) -> Result<(), String> {
        let is = |t: ElementTypeId, c: Category| project.element_type(t).is_some_and(|x| x.category() == c);
        if !is(self.column_type, Category::Column) {
            return Err(format!("type {} is not a column type", self.column_type.0));
        }
        let beams = [Some(self.rafter_type), self.purlins.map(|s| s.type_id), self.girts.map(|s| s.type_id)];
        match beams.into_iter().flatten().find(|t| !is(*t, Category::Beam)) {
            Some(t) => Err(format!("type {} is not a beam type", t.0)),
            None => Ok(()),
        }
    }

    /// Put every out-of-range number back to its default; returns how many.
    pub fn reset_out_of_range(&mut self) -> usize {
        let mut n = reset(&mut self.span, &SPAN, DEFAULT_SPAN)
            + reset(&mut self.eave_height, &EAVE_HEIGHT, DEFAULT_EAVE_HEIGHT)
            + reset(&mut self.slope_percent, &SLOPE, DEFAULT_SLOPE)
            + reset(&mut self.bay_spacing, &BAY_SPACING, DEFAULT_BAY_SPACING);
        if !BAYS.contains(&self.bays) {
            self.bays = DEFAULT_BAYS;
            n += 1;
        }
        for s in [&mut self.purlins, &mut self.girts].into_iter().flatten() {
            n += reset(&mut s.spacing, &SPACING, DEFAULT_SECONDARY_SPACING);
        }
        n
    }

    /// The member types it uses.
    pub fn type_ids(&self) -> impl Iterator<Item = ElementTypeId> + '_ {
        [Some(self.column_type), Some(self.rafter_type), self.purlins.map(|s| s.type_id), self.girts.map(|s| s.type_id)]
            .into_iter()
            .flatten()
    }

    /// Frame line to frame line, meters.
    pub fn length(&self) -> f64 {
        f64::from(self.bays) * self.bay_spacing
    }

    fn axes(&self) -> (DVec2, DVec2) {
        let x = DVec2::from_angle(self.rotation_deg.to_radians());
        (x, x.perp())
    }

    /// A local point in plan.
    pub fn to_plan(&self, local: DVec2) -> DVec2 {
        let (x, y) = self.axes();
        self.origin + x * local.x + y * local.y
    }

    /// The column lines' rectangle in plan, counter-clockwise from the origin.
    pub fn footprint(&self) -> [DVec2; 4] {
        let (w, l) = (self.span, self.length());
        [DVec2::ZERO, DVec2::new(w, 0.0), DVec2::new(w, l), DVec2::new(0.0, l)].map(|p| self.to_plan(p))
    }

    /// Move, turn or mirror it. A mirror flips the local frame's handedness,
    /// so the far gable's corner becomes the origin (the frames run the
    /// other way and the footprint stays where the mirror puts it).
    pub fn transform(&mut self, t: &PlanTransform) {
        let corner = if t.mirror { DVec2::new(0.0, self.length()) } else { DVec2::ZERO };
        self.origin = t.apply(self.to_plan(corner));
        self.rotation_deg = t.apply_angle_deg(self.rotation_deg);
    }

    fn slope(&self) -> f64 {
        self.slope_percent / 100.0
    }

    /// The rafters' top above the level at local `x` (the location line).
    fn rafter_top(&self, rafter_depth: f64, x: f64) -> f64 {
        let run = match self.roof {
            FrameRoof::Gable => x.min(self.span - x),
            FrameRoof::Mono => x,
        };
        self.eave_height + rafter_depth + self.slope() * run.clamp(0.0, self.span)
    }

    /// The local x of the purlin lines: on each roof plane from the column
    /// face every `spacing`, the last at the ridge (gable: half a purlin
    /// short of mid-span, mirrored on the other half) or at the far column
    /// face (mono); a line closer than a purlin's width to the last is left out.
    fn purlin_lines(&self, half_column: f64, width: f64, spacing: f64) -> Vec<f64> {
        let plane = |from: f64, to: f64| {
            let mut xs = Vec::new();
            let mut x = from;
            while x < to - width - 1e-9 && xs.len() < MAX_LINES {
                xs.push(x);
                x += spacing;
            }
            xs.push(to.max(from));
            xs
        };
        match self.roof {
            FrameRoof::Gable => {
                let left = plane(half_column, self.span / 2.0 - width / 2.0);
                let right: Vec<f64> = left.iter().map(|x| self.span - x).collect();
                left.into_iter().chain(right).collect()
            }
            FrameRoof::Mono => plane(half_column, self.span - half_column),
        }
    }

    /// The girt rows' tops above the level: every `spacing` from `spacing`
    /// up, below the eave.
    fn girt_rows(&self, spacing: f64) -> Vec<f64> {
        (1..=MAX_LINES).map(|r| r as f64 * spacing).take_while(|z| *z < self.eave_height - 1e-9).collect()
    }

    /// Its members: role, type and element, in a fixed order (columns,
    /// rafters, purlins, girts), at most [`MAX_MEMBERS`]. Empty when a
    /// number is out of range.
    pub fn layout(&self, project: &Project) -> Vec<(MemberRole, ElementTypeId, ElementKind)> {
        let mut out = Members(Vec::new());
        if self.check().is_err() {
            return out.0;
        }
        let (_, column_depth) = section_box(project, self.column_type);
        let (_, rafter_depth) = section_box(project, self.rafter_type);
        let half_column = column_depth / 2.0;
        let plan = |x: f64, y: f64| self.to_plan(DVec2::new(x, y));
        let rafters = match self.roof {
            FrameRoof::Gable => vec![(0.0, self.span / 2.0), (self.span, self.span / 2.0)],
            FrameRoof::Mono => vec![(0.0, self.span)],
        };
        for k in 0..=self.bays {
            let y = f64::from(k) * self.bay_spacing;
            for (i, x) in [0.0, self.span].into_iter().enumerate() {
                // Turned a quarter so the section's depth runs across the span,
                // up to the rafters' top so they frame into it.
                let column = Column {
                    position: plan(x, y),
                    rotation_deg: self.rotation_deg + 90.0,
                    base_offset: 0.0,
                    height: self.eave_height + rafter_depth,
                    top: WallTop::Unconnected,
                    structural: true,
                };
                out.push(RoleKind::Column, k, i, self.column_type, ElementKind::Column(column));
            }
            for (i, &(a, b)) in rafters.iter().enumerate() {
                let mut beam = Beam::new(plan(a, y), plan(b, y), self.rafter_top(rafter_depth, a));
                let end = self.rafter_top(rafter_depth, b);
                beam.end_offset = ((end - beam.top_offset).abs() > 1e-9).then_some(end);
                out.push(RoleKind::Rafter, k, i, self.rafter_type, ElementKind::Beam(beam));
            }
        }
        if let Some(s) = self.purlins {
            let (width, depth) = section_box(project, s.type_id);
            let lines = self.purlin_lines(half_column, width, s.spacing);
            for k in 0..self.bays {
                let (y0, y1) = (f64::from(k) * self.bay_spacing, f64::from(k + 1) * self.bay_spacing);
                for (i, &x) in lines.iter().enumerate() {
                    if out.full() {
                        break;
                    }
                    // It bears on the rafters: its bottom on their top.
                    let beam = Beam::new(plan(x, y0), plan(x, y1), self.rafter_top(rafter_depth, x) + depth);
                    out.push(RoleKind::Purlin, k, i, s.type_id, ElementKind::Beam(beam));
                }
            }
        }
        if let Some(s) = self.girts {
            let (width, _) = section_box(project, s.type_id);
            let off = half_column + width / 2.0;
            let rows = self.girt_rows(s.spacing);
            for k in 0..self.bays {
                let (y0, y1) = (f64::from(k) * self.bay_spacing, f64::from(k + 1) * self.bay_spacing);
                for (side, x) in [(0, -off), (1, self.span + off)] {
                    for (r, &z) in rows.iter().enumerate() {
                        if out.full() {
                            break;
                        }
                        let beam = Beam::new(plan(x, y0), plan(x, y1), z);
                        out.push(RoleKind::Girt, k, side * 1000 + r, s.type_id, ElementKind::Beam(beam));
                    }
                }
            }
        }
        out.0
    }
}

impl Project {
    /// The members a steel building manages (none for other elements).
    pub fn steel_members_of(&self, host: ElementId) -> Vec<ElementId> {
        match self.element(host).map(|e| &e.kind) {
            Some(ElementKind::SteelBuilding(b)) => b.members.iter().map(|m| m.id).collect(),
            _ => Vec::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ColumnSection, Document, Element, LevelId};

    fn close(a: DVec2, b: DVec2) -> bool {
        a.distance(b) < 1e-9
    }

    fn add_type(e: &mut crate::EditCtx, data: TypeData) -> ElementTypeId {
        let id = ElementTypeId(e.alloc_id());
        e.add_type(ElementType {
            id,
            name: format!("T{}", id.0),
            data,
            info: Default::default(),
            uid: Default::default(),
        })
    }

    pub(super) fn element(id: ElementId, level: LevelId, type_id: ElementTypeId, kind: ElementKind) -> Element {
        Element {
            id,
            level,
            type_id,
            mark: String::new(),
            comments: String::new(),
            uid: Default::default(),
            kind,
            phase_created: Default::default(),
            phase_demolished: None,
            design_option: None,
            paints: Vec::new(),
        }
    }

    /// A document with a default gable building on the first level: columns
    /// of a 0.2 x 0.3 type, rafters, purlins and girts of a 0.1 x 0.2 beam type.
    pub(super) fn site() -> (Document, ElementId) {
        let mut doc = Document::new(Project::from_template());
        let host = doc.edit("building", |e| {
            let level = e.project().levels[0].id;
            let material = e.project().materials[0].name.clone();
            let column = add_type(
                e,
                TypeData::Column {
                    section: ColumnSection::Rectangular { width: 0.2, depth: 0.3 },
                    material: material.clone(),
                    profile: None,
                    profile_rotation_deg: 0.0,
                    profile_mirror: false,
                },
            );
            let beam = add_type(
                e,
                TypeData::Beam {
                    width: 0.1,
                    height: 0.2,
                    material,
                    shape: Default::default(),
                    profile: None,
                    profile_rotation_deg: 0.0,
                    profile_mirror: false,
                },
            );
            let ty = add_type(e, TypeData::SteelBuilding {});
            let id = ElementId(e.alloc_id());
            let b = SteelBuilding::new(column, beam, Some(beam));
            e.add_element(element(id, level, ty, ElementKind::SteelBuilding(Box::new(b))))
        });
        (doc, host)
    }

    pub(super) fn building(doc: &Document, host: ElementId) -> SteelBuilding {
        match doc.project.element(host).map(|e| &e.kind) {
            Some(ElementKind::SteelBuilding(b)) => (**b).clone(),
            _ => panic!("not a steel building"),
        }
    }

    fn count(layout: &[(MemberRole, ElementTypeId, ElementKind)], kind: RoleKind) -> usize {
        layout.iter().filter(|(r, _, _)| r.kind == kind).count()
    }

    fn find(
        layout: &[(MemberRole, ElementTypeId, ElementKind)],
        kind: RoleKind,
        frame: u16,
        index: u16,
    ) -> ElementKind {
        let role = MemberRole { kind, frame, index };
        layout.iter().find(|(r, _, _)| *r == role).map(|(_, _, k)| k.clone()).expect("role")
    }

    #[test]
    fn layout_counts_members() {
        let (doc, host) = site();
        let mut b = building(&doc, host);
        let p = &doc.project;
        let kinds = |l: &[(MemberRole, ElementTypeId, ElementKind)]| {
            [
                count(l, RoleKind::Column),
                count(l, RoleKind::Rafter),
                count(l, RoleKind::Purlin),
                count(l, RoleKind::Girt),
            ]
        };
        // Gable, 5 bays: 6 frames of 2 columns and 2 rafters; per bay 16 purlin
        // lines (7 every 1.5 m from the column face plus the ridge, each half)
        // and 3 girt rows (1.5, 3, 4.5 m) on each side.
        assert_eq!(kinds(&b.layout(p)), [12, 12, 80, 30]);
        // Mono: 1 rafter per frame, 15 purlin lines (14 plus the far column face).
        b.roof = FrameRoof::Mono;
        assert_eq!(kinds(&b.layout(p)), [12, 6, 75, 30]);
        (b.purlins, b.girts) = (None, None);
        assert_eq!(b.layout(p).len(), 18);
        b.roof = FrameRoof::Gable;
        assert_eq!(b.layout(p).len(), 24);
    }

    #[test]
    fn rafters_rise_from_the_columns() {
        let (doc, host) = site();
        let l = building(&doc, host).layout(&doc.project);
        // Rafter depth 0.2: the top starts 0.2 over the 6 m eave and rises
        // 10 % over half the 20 m span.
        let ElementKind::Beam(left) = find(&l, RoleKind::Rafter, 0, 0) else { panic!("a beam") };
        assert!(close(left.start, DVec2::ZERO) && close(left.end, DVec2::new(10.0, 0.0)));
        assert!((left.top_offset - 6.2).abs() < 1e-9);
        assert!((left.end_offset.unwrap_or(0.0) - 7.2).abs() < 1e-9);
        let ElementKind::Beam(right) = find(&l, RoleKind::Rafter, 1, 1) else { panic!("a beam") };
        assert!(close(right.start, DVec2::new(20.0, 6.0)) && close(right.end, DVec2::new(10.0, 6.0)));
        // Columns rise to the rafters' top, turned so their depth runs across the span.
        let ElementKind::Column(c) = find(&l, RoleKind::Column, 0, 1) else { panic!("a column") };
        assert!(close(c.position, DVec2::new(20.0, 0.0)));
        assert!((c.height - 6.2).abs() < 1e-9 && (c.rotation_deg - 90.0).abs() < 1e-9);
        // The first purlin starts at the column face (0.15 m in) and bears on the rafter.
        let ElementKind::Beam(p0) = find(&l, RoleKind::Purlin, 0, 0) else { panic!("a beam") };
        assert!(close(p0.start, DVec2::new(0.15, 0.0)) && close(p0.end, DVec2::new(0.15, 6.0)));
        assert!((p0.top_offset - (6.2 + 0.015 + 0.2)).abs() < 1e-9);
        // The ridge purlins sit half a purlin off mid-span.
        let ElementKind::Beam(ridge) = find(&l, RoleKind::Purlin, 0, 7) else { panic!("a beam") };
        assert!((ridge.start.x - 9.95).abs() < 1e-9);
        // Girts: outside the column face, rows every 1.5 m below the eave.
        let ElementKind::Beam(g) = find(&l, RoleKind::Girt, 0, 1002) else { panic!("a beam") };
        assert!((g.start.x - 20.2).abs() < 1e-9 && (g.top_offset - 4.5).abs() < 1e-9);
    }

    #[test]
    fn origin_and_rotation_place_the_frames() {
        let (doc, host) = site();
        let mut b = building(&doc, host);
        (b.origin, b.rotation_deg) = (DVec2::new(5.0, 5.0), 90.0);
        let l = b.layout(&doc.project);
        let at = |frame, index| match find(&l, RoleKind::Column, frame, index) {
            ElementKind::Column(c) => c.position,
            _ => panic!("a column"),
        };
        assert!(close(at(0, 1), DVec2::new(5.0, 25.0)), "the span runs along +y");
        assert!(close(at(1, 0), DVec2::new(-1.0, 5.0)), "the frames along -x");
        assert!(close(b.footprint()[2], DVec2::new(-25.0, 25.0)));
    }

    #[test]
    fn a_mirror_keeps_the_footprint() {
        let (doc, host) = site();
        let mut b = building(&doc, host);
        b.rotation_deg = 30.0;
        let t = PlanTransform::mirror_across(DVec2::new(3.0, 0.0), DVec2::new(0.0, 1.0));
        let want: Vec<DVec2> = b.footprint().iter().map(|p| t.apply(*p)).collect();
        b.transform(&t);
        for p in b.footprint() {
            assert!(want.iter().any(|q| close(*q, p)), "{p}");
        }
    }

    #[test]
    fn layouts_stop_at_the_limit_and_need_valid_numbers() {
        let (doc, host) = site();
        let mut b = building(&doc, host);
        (b.span, b.bays, b.eave_height) = (100.0, 100, 50.0);
        for s in [&mut b.purlins, &mut b.girts].into_iter().flatten() {
            s.spacing = 0.3;
        }
        assert_eq!(b.layout(&doc.project).len(), MAX_MEMBERS);
        b.span = 150.0;
        assert!(b.check().is_err());
        assert!(b.layout(&doc.project).is_empty());
        assert_eq!(b.reset_out_of_range(), 1);
        assert_eq!(b.span, DEFAULT_SPAN);
        b.column_type = b.rafter_type;
        assert!(b.check_types(&doc.project).is_err());
    }

    #[test]
    fn roof_codes_round_trip() {
        for r in FrameRoof::ALL {
            assert_eq!(FrameRoof::from_code(r.code()), Some(r));
        }
        assert_eq!(FrameRoof::from_code(" MONO "), Some(FrameRoof::Mono));
        assert_eq!(FrameRoof::from_code("dome"), None);
    }
}
