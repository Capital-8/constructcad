//! Painting [`Drawing2D`]s with egui.

use cc_core::ElementId;
use cc_views::{Drawing2D, LineStyle, Prim};
use egui::{Color32, Painter, Pos2, Shape, Stroke};
use glam::DVec2;
use std::collections::BTreeSet;

pub const HIGHLIGHT: Color32 = Color32::from_rgb(20, 120, 255);
pub const PREVIEW: Color32 = Color32::from_rgb(230, 110, 20);

/// Maps drawing coordinates to the screen.
#[derive(Clone, Copy)]
pub struct Xf {
    /// Screen position of `origin`.
    pub screen: Pos2,
    pub origin: DVec2,
    /// Screen points per drawing unit.
    pub scale: f64,
}

impl Xf {
    pub fn to_screen(self, p: DVec2) -> Pos2 {
        let d = (p - self.origin) * self.scale;
        Pos2::new(self.screen.x + d.x as f32, self.screen.y - d.y as f32)
    }

    pub fn to_world(self, s: Pos2) -> DVec2 {
        self.origin + DVec2::new((s.x - self.screen.x) as f64, (self.screen.y - s.y) as f64) / self.scale
    }

    /// Compose with a drawing-to-drawing placement `p ↦ center + (p - from) * k`.
    pub fn then_place(&self, from: DVec2, center: DVec2, k: f64) -> Xf {
        Xf { screen: self.to_screen(center), origin: from, scale: self.scale * k }
    }
}

pub fn stroke_for(style: LineStyle, highlighted: bool) -> Stroke {
    let (width, color) = match style {
        LineStyle::Cut => (2.0, Color32::from_gray(10)),
        LineStyle::Projection => (1.0, Color32::from_gray(45)),
        LineStyle::Thin => (0.6, Color32::from_gray(95)),
        LineStyle::Hidden => (0.8, Color32::from_gray(120)),
        LineStyle::Annotation => (0.9, Color32::from_rgb(30, 90, 170)),
    };
    if highlighted { Stroke::new(width + 0.6, HIGHLIGHT) } else { Stroke::new(width, color) }
}

fn is_convex(pts: &[Pos2]) -> bool {
    let n = pts.len();
    let mut sign = 0.0f32;
    for i in 0..n {
        let (a, b, c) = (pts[i], pts[(i + 1) % n], pts[(i + 2) % n]);
        let cross = (b - a).x * (c - b).y - (b - a).y * (c - b).x;
        if cross.abs() > 1e-3 {
            if sign != 0.0 && cross.signum() != sign {
                return false;
            }
            sign = cross.signum();
        }
    }
    true
}

pub fn fill_polygon(painter: &Painter, pts: Vec<Pos2>, color: Color32) {
    if pts.len() < 3 {
        return;
    }
    if is_convex(&pts) {
        painter.add(Shape::convex_polygon(pts, color, Stroke::NONE));
        return;
    }
    let flat: Vec<DVec2> = pts.iter().map(|p| DVec2::new(p.x as f64, p.y as f64)).collect();
    let mut mesh = egui::Mesh::default();
    for p in &pts {
        mesh.colored_vertex(*p, color);
    }
    for [a, b, c] in cc_geom::polygon::triangulate(&flat) {
        mesh.add_triangle(a as u32, b as u32, c as u32);
    }
    painter.add(Shape::mesh(mesh));
}

fn tint(c: [u8; 4], highlighted: bool) -> Color32 {
    if highlighted {
        let mix = |v: u8, h: u8| ((v as u16 + h as u16 * 2) / 3) as u8;
        Color32::from_rgba_unmultiplied(mix(c[0], 120), mix(c[1], 175), mix(c[2], 255), c[3])
    } else {
        Color32::from_rgba_unmultiplied(c[0], c[1], c[2], c[3])
    }
}

pub struct PaintOptions<'a> {
    pub highlight: &'a BTreeSet<ElementId>,
    /// Screen pixels per paper millimeter, for text.
    pub px_per_mm: f64,
    /// Draw everything in a single color (ghost previews).
    pub mono: Option<Color32>,
    /// Only elements in this set (ghost previews).
    pub only: Option<&'a BTreeSet<ElementId>>,
}

pub fn paint_drawing(painter: &Painter, d: &Drawing2D, xf: &Xf, opts: &PaintOptions) {
    let clip = painter.clip_rect().expand(50.0);
    for prim in &d.prims {
        match prim {
            Prim::Fill { pts, color, element } => {
                if opts.only.is_some_and(|only| !element.is_some_and(|e| only.contains(&e))) {
                    continue;
                }
                let hl = element.is_some_and(|e| opts.highlight.contains(&e));
                let screen: Vec<Pos2> = pts.iter().map(|p| xf.to_screen(*p)).collect();
                if !screen.iter().any(|p| clip.contains(*p)) && !egui::Rect::from_points(&screen).intersects(clip) {
                    continue;
                }
                let c = match opts.mono {
                    Some(m) => m.gamma_multiply(0.25),
                    None => tint(*color, hl),
                };
                fill_polygon(painter, screen, c);
            }
            Prim::Line { a, b, style, element } => {
                if opts.only.is_some_and(|only| !element.is_some_and(|e| only.contains(&e))) {
                    continue;
                }
                let hl = element.is_some_and(|e| opts.highlight.contains(&e));
                let (pa, pb) = (xf.to_screen(*a), xf.to_screen(*b));
                if !egui::Rect::from_two_pos(pa, pb).intersects(clip) {
                    continue;
                }
                let mut stroke = stroke_for(*style, hl);
                if let Some(m) = opts.mono {
                    stroke.color = m;
                }
                if *style == LineStyle::Hidden {
                    painter.extend(Shape::dashed_line(&[pa, pb], stroke, 6.0, 4.0));
                } else {
                    painter.line_segment([pa, pb], stroke);
                }
            }
            Prim::Text { pos, text, size } => {
                if opts.only.is_some() {
                    continue;
                }
                let px = (size * opts.px_per_mm) as f32;
                if px >= 3.5 {
                    painter.text(
                        xf.to_screen(*pos),
                        egui::Align2::LEFT_BOTTOM,
                        text,
                        egui::FontId::proportional(px),
                        Color32::from_gray(20),
                    );
                }
            }
        }
    }
}
