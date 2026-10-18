//! Parametric components: user-defined objects (furniture, fixtures,
//! casework…) whose solids are sized by named parameters.
//!
//! A [`ComponentDef`] lists numeric parameters and solid pieces. Each piece
//! is a box, a cylinder or an extruded profile whose coordinates and sizes
//! are [`crate::expr`] expressions over the parameters, in the component's
//! local frame: origin at the insertion point, X to the right, Y to the
//! back, Z up, meters. Types of the same component share the definition and
//! differ in parameter values; parameters marked `instance` can be
//! overridden per element.

use crate::expr::{self, ExprError};
use glam::DVec2;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// What a component represents (used for IFC mapping and grouping).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub enum ComponentKind {
    Furniture,
    Casework,
    Plumbing,
    Lighting,
    #[default]
    Generic,
}

impl ComponentKind {
    pub const ALL: [ComponentKind; 5] = [
        ComponentKind::Furniture,
        ComponentKind::Casework,
        ComponentKind::Plumbing,
        ComponentKind::Lighting,
        ComponentKind::Generic,
    ];

    pub fn key(self) -> &'static str {
        match self {
            ComponentKind::Furniture => "comp-kind-furniture",
            ComponentKind::Casework => "comp-kind-casework",
            ComponentKind::Plumbing => "comp-kind-plumbing",
            ComponentKind::Lighting => "comp-kind-lighting",
            ComponentKind::Generic => "comp-kind-generic",
        }
    }
}

/// Unit of a parameter value.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub enum ParamKind {
    /// Meters.
    #[default]
    Length,
    /// Degrees.
    Angle,
    Number,
}

impl ParamKind {
    pub const ALL: [ParamKind; 3] = [ParamKind::Length, ParamKind::Angle, ParamKind::Number];

    pub fn key(self) -> &'static str {
        match self {
            ParamKind::Length => "param-kind-length",
            ParamKind::Angle => "param-kind-angle",
            ParamKind::Number => "param-kind-number",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ComponentParam {
    /// Name used in expressions (see [`expr::is_valid_name`]).
    pub name: String,
    /// Value of this type.
    pub value: f64,
    #[serde(default)]
    pub kind: ParamKind,
    /// Elements may override the value.
    #[serde(default)]
    pub instance: bool,
}

/// Geometry of a piece; every field is an expression.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ComponentShape {
    /// Box from the corner (`x`, `y`, `z`) with the given sizes.
    Box { x: String, y: String, z: String, width: String, depth: String, height: String },
    /// Vertical cylinder with its base centered at (`x`, `y`, `z`).
    Cylinder { x: String, y: String, z: String, radius: String, height: String },
    /// Closed profile in plan extruded upwards from `z`.
    Extrusion { points: Vec<(String, String)>, z: String, height: String },
}

impl ComponentShape {
    /// Every expression of the shape, with a field label.
    pub fn expressions(&self) -> Vec<(&'static str, &String)> {
        match self {
            ComponentShape::Box { x, y, z, width, depth, height } => {
                vec![("x", x), ("y", y), ("z", z), ("width", width), ("depth", depth), ("height", height)]
            }
            ComponentShape::Cylinder { x, y, z, radius, height } => {
                vec![("x", x), ("y", y), ("z", z), ("radius", radius), ("height", height)]
            }
            ComponentShape::Extrusion { points, z, height } => {
                let mut out: Vec<(&'static str, &String)> =
                    points.iter().flat_map(|(x, y)| [("x", x), ("y", y)]).collect();
                out.push(("z", z));
                out.push(("height", height));
                out
            }
        }
    }

    pub fn expressions_mut(&mut self) -> Vec<&mut String> {
        match self {
            ComponentShape::Box { x, y, z, width, depth, height } => vec![x, y, z, width, depth, height],
            ComponentShape::Cylinder { x, y, z, radius, height } => vec![x, y, z, radius, height],
            ComponentShape::Extrusion { points, z, height } => {
                let mut out: Vec<&mut String> = points.iter_mut().flat_map(|(x, y)| [x, y]).collect();
                out.push(z);
                out.push(height);
                out
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ComponentItem {
    pub name: String,
    pub shape: ComponentShape,
    /// Project material; empty for the default color.
    #[serde(default)]
    pub material: String,
    /// The piece's outline is drawn in plans.
    #[serde(default = "yes")]
    pub in_plan: bool,
}

fn yes() -> bool {
    true
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct ComponentDef {
    #[serde(default)]
    pub kind: ComponentKind,
    #[serde(default)]
    pub params: Vec<ComponentParam>,
    #[serde(default)]
    pub items: Vec<ComponentItem>,
}

/// A resolved piece: a prism of `footprint` between `z0` and `z1`, in the
/// component's local frame.
#[derive(Debug, Clone, PartialEq)]
pub struct ComponentPart {
    pub item: usize,
    pub footprint: Vec<DVec2>,
    pub z0: f64,
    pub z1: f64,
    pub material: String,
    pub in_plan: bool,
}

/// Why a piece could not be resolved.
#[derive(Debug, Clone, PartialEq)]
pub enum PartError {
    Expr {
        field: &'static str,
        error: ExprError,
    },
    /// A size is zero or negative, or a profile has fewer than 3 points.
    Degenerate,
}

impl std::fmt::Display for PartError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PartError::Expr { field, error } => write!(f, "{field}: {error}"),
            PartError::Degenerate => write!(f, "zero or negative size"),
        }
    }
}

/// Segments of a cylinder's polygon.
const CYLINDER_SEGMENTS: usize = 24;

impl ComponentDef {
    pub fn param(&self, name: &str) -> Option<&ComponentParam> {
        self.params.iter().find(|p| p.name == name)
    }

    /// Parameter values with instance `overrides` applied (only to instance
    /// parameters; non-finite overrides are ignored).
    pub fn values(&self, overrides: &BTreeMap<String, f64>) -> BTreeMap<String, f64> {
        self.params
            .iter()
            .map(|p| {
                let v = overrides.get(&p.name).copied().filter(|v| p.instance && v.is_finite()).unwrap_or(p.value);
                (p.name.clone(), v)
            })
            .collect()
    }

    /// Resolve every piece; pieces that fail are reported with their index.
    pub fn resolve(&self, overrides: &BTreeMap<String, f64>) -> (Vec<ComponentPart>, Vec<(usize, PartError)>) {
        let values = self.values(overrides);
        let lookup = |name: &str| values.get(name).copied();
        let mut parts = Vec::new();
        let mut errors = Vec::new();
        for (i, item) in self.items.iter().enumerate() {
            match resolve_shape(&item.shape, &lookup) {
                Ok((footprint, z0, z1)) => parts.push(ComponentPart {
                    item: i,
                    footprint,
                    z0,
                    z1,
                    material: item.material.clone(),
                    in_plan: item.in_plan,
                }),
                Err(e) => errors.push((i, e)),
            }
        }
        (parts, errors)
    }

    /// Material of the first piece (for coloring and type legends).
    pub fn main_material(&self) -> &str {
        self.items.first().map_or("", |i| i.material.as_str())
    }

    /// Rename a parameter and every reference to it in the pieces.
    pub fn rename_param(&mut self, old: &str, new: &str) {
        for p in &mut self.params {
            if p.name == old {
                p.name = new.to_string();
            }
        }
        for item in &mut self.items {
            for e in item.shape.expressions_mut() {
                *e = replace_name(e, old, new);
            }
        }
    }
}

/// Replace whole-identifier occurrences of `old` in `text` (numbers such as
/// `2e3` are left alone).
fn replace_name(text: &str, old: &str, new: &str) -> String {
    let chars: Vec<char> = text.chars().collect();
    let mut out = String::with_capacity(text.len());
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        let start = i;
        if c.is_ascii_digit() || c == '.' {
            while i < chars.len() && (expr::is_ident_char(chars[i]) || chars[i] == '.') {
                i += 1;
            }
            out.extend(&chars[start..i]);
        } else if expr::is_ident_char(c) {
            while i < chars.len() && expr::is_ident_char(chars[i]) {
                i += 1;
            }
            let ident: String = chars[start..i].iter().collect();
            out.push_str(if ident == old { new } else { &ident });
        } else {
            out.push(c);
            i += 1;
        }
    }
    out
}

fn resolve_shape(
    shape: &ComponentShape,
    lookup: &dyn Fn(&str) -> Option<f64>,
) -> Result<(Vec<DVec2>, f64, f64), PartError> {
    let ev =
        |field: &'static str, text: &str| expr::eval(text, lookup).map_err(|error| PartError::Expr { field, error });
    let positive = |v: f64| if v > 1e-6 { Ok(v) } else { Err(PartError::Degenerate) };
    match shape {
        ComponentShape::Box { x, y, z, width, depth, height } => {
            let (x, y, z) = (ev("x", x)?, ev("y", y)?, ev("z", z)?);
            let w = positive(ev("width", width)?)?;
            let d = positive(ev("depth", depth)?)?;
            let h = positive(ev("height", height)?)?;
            let footprint =
                vec![DVec2::new(x, y), DVec2::new(x + w, y), DVec2::new(x + w, y + d), DVec2::new(x, y + d)];
            Ok((footprint, z, z + h))
        }
        ComponentShape::Cylinder { x, y, z, radius, height } => {
            let c = DVec2::new(ev("x", x)?, ev("y", y)?);
            let z = ev("z", z)?;
            let r = positive(ev("radius", radius)?)?;
            let h = positive(ev("height", height)?)?;
            let footprint = (0..CYLINDER_SEGMENTS)
                .map(|i| c + DVec2::from_angle(i as f64 / CYLINDER_SEGMENTS as f64 * std::f64::consts::TAU) * r)
                .collect();
            Ok((footprint, z, z + h))
        }
        ComponentShape::Extrusion { points, z, height } => {
            let mut footprint = Vec::with_capacity(points.len());
            for (x, y) in points {
                footprint.push(DVec2::new(ev("x", x)?, ev("y", y)?));
            }
            let z = ev("z", z)?;
            let h = positive(ev("height", height)?)?;
            footprint.dedup_by(|a, b| a.distance(*b) < 1e-9);
            let area: f64 =
                (0..footprint.len()).map(|i| footprint[i].perp_dot(footprint[(i + 1) % footprint.len()])).sum::<f64>()
                    / 2.0;
            if footprint.len() < 3 || area.abs() < 1e-9 {
                return Err(PartError::Degenerate);
            }
            Ok((footprint, z, z + h))
        }
    }
}

/// The default component library: (component name, type name, definition).
/// Parameter names come from `n` (keys `tpl-param-*`), so expressions use
/// the translated names; materials are the template's.
pub fn library(n: &dyn Fn(&str) -> String) -> Vec<(String, String, ComponentDef)> {
    let (w, d, h) = (n("tpl-param-width"), n("tpl-param-depth"), n("tpl-param-height"));
    let wood = n("tpl-mat-wood");
    let fabric = n("tpl-mat-fabric");
    let porcelain = n("tpl-mat-porcelain");
    let length = |name: &str, value: f64| ComponentParam {
        name: name.to_string(),
        value,
        kind: ParamKind::Length,
        instance: false,
    };
    let bx = |name: &str,
              x: String,
              y: String,
              z: String,
              sw: String,
              sd: String,
              sh: String,
              material: &str,
              plan: bool| {
        ComponentItem {
            name: name.to_string(),
            shape: ComponentShape::Box { x, y, z, width: sw, depth: sd, height: sh },
            material: material.to_string(),
            in_plan: plan,
        }
    };
    let s = |v: &str| v.to_string();
    // Four legs of `t` inset at the corners of a W x D footprint centered on the origin.
    let legs = |t: &str, top: String, material: &str| -> Vec<ComponentItem> {
        [("-", "-"), ("+", "-"), ("+", "+"), ("-", "+")]
            .iter()
            .enumerate()
            .map(|(i, (sx, sy))| {
                let x = if *sx == "-" { format!("-{w} / 2") } else { format!("{w} / 2 - {t}") };
                let y = if *sy == "-" { format!("-{d} / 2") } else { format!("{d} / 2 - {t}") };
                bx(&format!("{} {}", n("tpl-part-leg"), i + 1), x, y, s("0"), s(t), s(t), top.clone(), material, false)
            })
            .collect()
    };

    let mut out = Vec::new();

    // Table: top and four legs.
    let mut items = vec![bx(
        &n("tpl-part-top"),
        format!("-{w} / 2"),
        format!("-{d} / 2"),
        format!("{h} - 0.03"),
        s(&w),
        s(&d),
        s("0.03"),
        &wood,
        true,
    )];
    items.extend(legs("0.05", format!("{h} - 0.03"), &wood));
    let table = ComponentDef {
        kind: ComponentKind::Furniture,
        params: vec![length(&w, 1.2), length(&d, 0.8), length(&h, 0.75)],
        items,
    };
    out.push((n("tpl-comp-table"), n("tpl-type-table"), table));

    // Chair: seat, legs and backrest.
    let seat = n("tpl-param-seat-height");
    let mut items = vec![bx(
        &n("tpl-part-seat"),
        format!("-{w} / 2"),
        format!("-{d} / 2"),
        format!("{seat} - 0.04"),
        s(&w),
        s(&d),
        s("0.04"),
        &wood,
        true,
    )];
    items.extend(legs("0.04", format!("{seat} - 0.04"), &wood));
    items.push(bx(
        &n("tpl-part-back"),
        format!("-{w} / 2"),
        format!("{d} / 2 - 0.03"),
        s(&seat),
        s(&w),
        s("0.03"),
        format!("{h} - {seat}"),
        &wood,
        true,
    ));
    let chair = ComponentDef {
        kind: ComponentKind::Furniture,
        params: vec![length(&w, 0.45), length(&d, 0.5), length(&h, 0.9), length(&seat, 0.45)],
        items,
    };
    out.push((n("tpl-comp-chair"), n("tpl-type-chair"), chair));

    // Bed: mattress base and headboard at the back.
    let bed = ComponentDef {
        kind: ComponentKind::Furniture,
        params: vec![length(&w, 1.4), length(&d, 1.9), length(&h, 0.5)],
        items: vec![
            bx(
                &n("tpl-part-mattress"),
                format!("-{w} / 2"),
                format!("-{d} / 2"),
                s("0"),
                s(&w),
                format!("{d} - 0.05"),
                s(&h),
                &fabric,
                true,
            ),
            bx(
                &n("tpl-part-headboard"),
                format!("-{w} / 2"),
                format!("{d} / 2 - 0.05"),
                s("0"),
                s(&w),
                s("0.05"),
                format!("{h} + 0.5"),
                &wood,
                true,
            ),
        ],
    };
    out.push((n("tpl-comp-bed"), n("tpl-type-bed"), bed));

    // Sofa: seat, back and two arms.
    let arm = "0.15";
    let sofa = ComponentDef {
        kind: ComponentKind::Furniture,
        params: vec![length(&w, 2.0), length(&d, 0.9), length(&h, 0.8), length(&seat, 0.42)],
        items: vec![
            bx(
                &n("tpl-part-seat"),
                format!("-{w} / 2 + {arm}"),
                format!("-{d} / 2"),
                s("0"),
                format!("{w} - 2 * {arm}"),
                format!("{d} - 0.2"),
                s(&seat),
                &fabric,
                true,
            ),
            bx(
                &n("tpl-part-back"),
                format!("-{w} / 2"),
                format!("{d} / 2 - 0.2"),
                s("0"),
                s(&w),
                s("0.2"),
                s(&h),
                &fabric,
                true,
            ),
            bx(
                &format!("{} 1", n("tpl-part-arm")),
                format!("-{w} / 2"),
                format!("-{d} / 2"),
                s("0"),
                s(arm),
                format!("{d} - 0.2"),
                format!("{seat} + 0.2"),
                &fabric,
                true,
            ),
            bx(
                &format!("{} 2", n("tpl-part-arm")),
                format!("{w} / 2 - {arm}"),
                format!("-{d} / 2"),
                s("0"),
                s(arm),
                format!("{d} - 0.2"),
                format!("{seat} + 0.2"),
                &fabric,
                true,
            ),
        ],
    };
    out.push((n("tpl-comp-sofa"), n("tpl-type-sofa"), sofa));

    // Toilet: bowl in front of the tank, against the wall at the back.
    let toilet = ComponentDef {
        kind: ComponentKind::Plumbing,
        params: vec![length(&w, 0.38), length(&d, 0.65), length(&h, 0.78)],
        items: vec![
            ComponentItem {
                name: n("tpl-part-bowl"),
                shape: ComponentShape::Cylinder {
                    x: s("0"),
                    y: format!("-{d} / 2 + {w} / 2"),
                    z: s("0"),
                    radius: format!("{w} / 2"),
                    height: s("0.4"),
                },
                material: porcelain.clone(),
                in_plan: true,
            },
            bx(
                &n("tpl-part-tank"),
                format!("-{w} / 2"),
                format!("{d} / 2 - 0.18"),
                s("0"),
                s(&w),
                s("0.18"),
                s(&h),
                &porcelain,
                true,
            ),
        ],
    };
    out.push((n("tpl-comp-toilet"), n("tpl-type-toilet"), toilet));

    // Washbasin: basin slab on a pedestal.
    let basin = ComponentDef {
        kind: ComponentKind::Plumbing,
        params: vec![length(&w, 0.55), length(&d, 0.45), length(&h, 0.85)],
        items: vec![
            bx(
                &n("tpl-part-basin"),
                format!("-{w} / 2"),
                format!("-{d} / 2"),
                format!("{h} - 0.15"),
                s(&w),
                s(&d),
                s("0.15"),
                &porcelain,
                true,
            ),
            ComponentItem {
                name: n("tpl-part-pedestal"),
                shape: ComponentShape::Cylinder {
                    x: s("0"),
                    y: format!("{d} / 2 - 0.12"),
                    z: s("0"),
                    radius: s("0.09"),
                    height: format!("{h} - 0.15"),
                },
                material: porcelain.clone(),
                in_plan: false,
            },
        ],
    };
    out.push((n("tpl-comp-washbasin"), n("tpl-type-washbasin"), basin));

    // Wardrobe: one box.
    let wardrobe = ComponentDef {
        kind: ComponentKind::Casework,
        params: vec![length(&w, 1.6), length(&d, 0.6), length(&h, 2.2)],
        items: vec![bx(
            &n("tpl-part-body"),
            format!("-{w} / 2"),
            format!("-{d} / 2"),
            s("0"),
            s(&w),
            s(&d),
            s(&h),
            &wood,
            true,
        )],
    };
    out.push((n("tpl-comp-wardrobe"), n("tpl-type-wardrobe"), wardrobe));
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pt(n: &str) -> String {
        match n {
            "tpl-param-width" => "Largura".into(),
            "tpl-param-depth" => "Profundidade".into(),
            "tpl-param-height" => "Altura".into(),
            "tpl-param-seat-height" => "AlturaAssento".into(),
            other => other.into(),
        }
    }

    #[test]
    fn library_resolves_without_errors() {
        let lib = library(&pt);
        assert_eq!(lib.len(), 7);
        for (name, _, def) in &lib {
            let (parts, errors) = def.resolve(&BTreeMap::new());
            assert!(errors.is_empty(), "{name}: {errors:?}");
            assert_eq!(parts.len(), def.items.len());
            for p in &parts {
                assert!(p.z1 > p.z0);
            }
        }
        // The table top sits at the table height.
        let (_, _, table) = &lib[0];
        let (parts, _) = table.resolve(&BTreeMap::new());
        assert!((parts[0].z1 - 0.75).abs() < 1e-12);
        let (lo, hi) = parts
            .iter()
            .flat_map(|p| p.footprint.iter())
            .fold((DVec2::splat(f64::INFINITY), DVec2::splat(f64::NEG_INFINITY)), |(lo, hi), p| {
                (lo.min(*p), hi.max(*p))
            });
        assert!((hi - lo - DVec2::new(1.2, 0.8)).abs().max_element() < 1e-12);
    }

    #[test]
    fn instance_overrides_apply_only_to_instance_params() {
        let mut def = library(&pt).remove(0).2;
        let mut overrides = BTreeMap::from([("Largura".to_string(), 2.0), ("Altura".to_string(), 1.0)]);
        def.params[0].instance = true;
        let values = def.values(&overrides);
        assert_eq!(values["Largura"], 2.0);
        assert_eq!(values["Altura"], 0.75);
        overrides.insert("Largura".into(), f64::NAN);
        assert_eq!(def.values(&overrides)["Largura"], 1.2);
    }

    #[test]
    fn errors_name_the_piece() {
        let mut def = library(&pt).remove(0).2;
        if let ComponentShape::Box { width, .. } = &mut def.items[1].shape {
            *width = "Largur * 2".into();
        }
        if let ComponentShape::Box { height, .. } = &mut def.items[2].shape {
            *height = "0 - 1".into();
        }
        let (parts, errors) = def.resolve(&BTreeMap::new());
        assert_eq!(parts.len(), def.items.len() - 2);
        assert_eq!(errors.len(), 2);
        assert_eq!(errors[0].0, 1);
        assert!(
            matches!(&errors[0].1, PartError::Expr { field: "width", error: ExprError::UnknownName(n) } if n == "Largur")
        );
        assert_eq!(errors[1], (2, PartError::Degenerate));
    }

    #[test]
    fn rename_updates_expressions() {
        let mut def = library(&pt).remove(0).2;
        def.rename_param("Largura", "W");
        assert_eq!(def.params[0].name, "W");
        let all: Vec<String> = def.items.iter().flat_map(|i| i.shape.expressions()).map(|(_, e)| e.clone()).collect();
        assert!(all.iter().any(|e| e.contains("W")));
        assert!(!all.iter().any(|e| e.contains("Largura")));
        assert_eq!(replace_name("Largura2 + Largura * 2e3 - a", "Largura", "W"), "Largura2 + W * 2e3 - a");
        assert_eq!(replace_name("x1 + 1x", "x", "y"), "x1 + 1x");
        assert!(def.resolve(&BTreeMap::new()).1.is_empty());
    }
}
