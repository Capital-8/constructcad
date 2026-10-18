//! Line icons drawn with the egui painter, so they scale crisply and follow
//! the theme colors. Each icon is designed on a 16 x 16 grid.

use egui::epaint::{CubicBezierShape, PathStroke};
use egui::{Color32, Painter, Pos2, Rect, Shape, Stroke, StrokeKind};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Icon {
    New,
    Open,
    Save,
    SaveAs,
    Export,
    About,
    Wall,
    Door,
    Window,
    Floor,
    Ceiling,
    Roof,
    Select,
    Copy,
    Delete,
    Undo,
    Redo,
    FloorPlan,
    CeilingPlan,
    View3D,
    Section,
    Elevation,
    Legend,
    Schedule,
    Sheet,
    ZoomFit,
    ProjectInfo,
    Levels,
    Materials,
    ChevronDown,
    ChevronRight,
    Plus,
    Home,
    ArrowLeft,
    CloseProject,
    Close,
    Room,
    Grid,
    Dimension,
}

/// Maps the 16 x 16 design grid into a screen rectangle.
struct Grid {
    origin: Pos2,
    scale: f32,
}

impl Grid {
    fn p(&self, x: f32, y: f32) -> Pos2 {
        self.origin + egui::vec2(x, y) * self.scale
    }

    fn pts(&self, pts: &[(f32, f32)]) -> Vec<Pos2> {
        pts.iter().map(|&(x, y)| self.p(x, y)).collect()
    }

    fn rect(&self, x0: f32, y0: f32, x1: f32, y1: f32) -> Rect {
        Rect::from_two_pos(self.p(x0, y0), self.p(x1, y1))
    }
}

/// Paint `icon` centered in `rect` with stroke color `color`.
pub fn paint(painter: &Painter, rect: Rect, icon: Icon, color: Color32) {
    let size = rect.width().min(rect.height());
    let scale = size / 16.0;
    let g = Grid { origin: rect.center() - egui::vec2(8.0, 8.0) * scale, scale };
    let w = (1.35 * scale).max(1.0);
    let s = Stroke::new(w, color);
    let thin = Stroke::new((0.9 * scale).max(0.8), color);
    let fill = color.gamma_multiply(0.18);
    let line = |pts: &[(f32, f32)]| {
        painter.add(Shape::line(g.pts(pts), PathStroke::from(s)));
    };
    let closed = |pts: &[(f32, f32)]| {
        painter.add(Shape::closed_line(g.pts(pts), PathStroke::from(s)));
    };
    let poly = |pts: &[(f32, f32)], fill: Color32| {
        painter.add(Shape::convex_polygon(g.pts(pts), fill, s));
    };
    let seg = |a: (f32, f32), b: (f32, f32), st: Stroke| {
        painter.line_segment([g.p(a.0, a.1), g.p(b.0, b.1)], st);
    };
    let rect_s = |x0: f32, y0: f32, x1: f32, y1: f32, r: f32| {
        painter.rect_stroke(g.rect(x0, y0, x1, y1), r * scale, s, StrokeKind::Middle);
    };
    let arc = |cx: f32, cy: f32, r: f32, a0: f32, a1: f32| {
        let n = 16;
        let pts: Vec<Pos2> = (0..=n)
            .map(|i| {
                let a = (a0 + (a1 - a0) * i as f32 / n as f32).to_radians();
                g.p(cx + r * a.cos(), cy + r * a.sin())
            })
            .collect();
        painter.add(Shape::line(pts, PathStroke::from(s)));
    };

    match icon {
        Icon::New => {
            closed(&[(3.5, 1.5), (9.5, 1.5), (12.5, 4.5), (12.5, 14.5), (3.5, 14.5)]);
            line(&[(9.5, 1.5), (9.5, 4.5), (12.5, 4.5)]);
            seg((8.0, 7.5), (8.0, 11.5), s);
            seg((6.0, 9.5), (10.0, 9.5), s);
        }
        Icon::Open => {
            closed(&[(1.5, 3.5), (6.0, 3.5), (7.5, 5.0), (13.5, 5.0), (13.5, 13.5), (1.5, 13.5)]);
            poly(&[(3.5, 7.5), (15.0, 7.5), (13.5, 13.5), (1.5, 13.5)], fill);
        }
        Icon::Save | Icon::SaveAs => {
            closed(&[(2.0, 2.0), (11.5, 2.0), (14.0, 4.5), (14.0, 14.0), (2.0, 14.0)]);
            rect_s(4.5, 2.0, 10.5, 6.0, 0.0);
            rect_s(4.5, 9.0, 11.5, 14.0, 0.5);
            if icon == Icon::SaveAs {
                painter.circle_filled(g.p(13.0, 13.0), 3.6 * scale, painter_bg(painter));
                seg((10.8, 15.2), (15.2, 10.8), s);
                seg((10.2, 15.8), (10.8, 15.2), s);
            }
        }
        Icon::Export => {
            line(&[(6.0, 2.5), (2.5, 2.5), (2.5, 13.5), (13.5, 13.5), (13.5, 10.0)]);
            seg((7.5, 8.5), (14.0, 2.0), s);
            line(&[(9.5, 2.0), (14.0, 2.0), (14.0, 6.5)]);
        }
        Icon::About => {
            painter.circle_stroke(g.p(8.0, 8.0), 6.5 * scale, s);
            seg((8.0, 7.0), (8.0, 11.5), s);
            painter.circle_filled(g.p(8.0, 4.8), 0.9 * scale, color);
        }
        Icon::Wall => {
            poly(&[(1.5, 5.0), (14.5, 5.0), (14.5, 11.0), (1.5, 11.0)], fill);
            for i in 0..5 {
                let x = 2.5 + i as f32 * 3.0;
                seg((x, 11.0), (x + 3.0, 5.0), thin);
            }
        }
        Icon::Door => {
            seg((0.5, 14.0), (3.0, 14.0), s);
            seg((13.0, 14.0), (15.5, 14.0), s);
            seg((3.0, 14.0), (3.0, 4.0), s);
            arc(3.0, 14.0, 10.0, -90.0, 0.0);
        }
        Icon::Window => {
            rect_s(2.0, 2.0, 14.0, 14.0, 0.5);
            seg((8.0, 2.0), (8.0, 14.0), thin);
            seg((2.0, 8.0), (14.0, 8.0), thin);
            poly(&[(2.0, 2.0), (8.0, 2.0), (8.0, 8.0), (2.0, 8.0)], fill);
        }
        Icon::Floor => {
            poly(&[(1.0, 9.0), (6.0, 6.0), (15.0, 6.0), (10.0, 9.0)], fill);
            line(&[(1.0, 9.0), (1.0, 11.0), (10.0, 11.0), (15.0, 8.0), (15.0, 6.0)]);
            seg((10.0, 9.0), (10.0, 11.0), s);
        }
        Icon::Ceiling => {
            poly(&[(1.0, 5.0), (6.0, 2.0), (15.0, 2.0), (10.0, 5.0)], fill);
            for x in [4.0, 8.0, 12.0] {
                seg((x, 6.5), (x, 12.5), thin);
                line(&[(x - 1.5, 11.0), (x, 12.5), (x + 1.5, 11.0)]);
            }
        }
        Icon::Roof => {
            poly(&[(8.0, 2.0), (15.0, 8.5), (1.0, 8.5)], fill);
            line(&[(3.0, 7.0), (3.0, 14.0), (13.0, 14.0), (13.0, 7.0)]);
            rect_s(6.5, 10.0, 9.5, 14.0, 0.0);
        }
        Icon::Select => {
            let pts = [(4.0, 1.5), (4.0, 13.0), (7.0, 10.2), (9.2, 14.8), (11.0, 14.0), (8.8, 9.4), (12.8, 9.4)];
            painter.add(Shape::closed_line(g.pts(&pts), PathStroke::from(s)));
        }
        Icon::Copy => {
            rect_s(5.5, 5.5, 14.0, 14.0, 1.5);
            line(&[(10.5, 5.5), (10.5, 2.0), (2.0, 2.0), (2.0, 10.5), (5.5, 10.5)]);
        }
        Icon::Delete => {
            seg((2.0, 4.0), (14.0, 4.0), s);
            line(&[(6.0, 4.0), (6.0, 2.0), (10.0, 2.0), (10.0, 4.0)]);
            line(&[(3.5, 4.0), (4.5, 14.5), (11.5, 14.5), (12.5, 4.0)]);
            seg((6.5, 7.0), (6.5, 12.0), thin);
            seg((9.5, 7.0), (9.5, 12.0), thin);
        }
        Icon::Undo | Icon::Redo => {
            let flip = |x: f32| if icon == Icon::Redo { 16.0 - x } else { x };
            let curve = CubicBezierShape::from_points_stroke(
                [g.p(flip(3.0), 6.0), g.p(flip(11.0), 6.0), g.p(flip(15.0), 12.0), g.p(flip(9.0), 13.5)],
                false,
                Color32::TRANSPARENT,
                s,
            );
            painter.add(curve);
            line(&[(flip(6.0), 2.5), (flip(2.5), 6.0), (flip(6.0), 9.5)]);
        }
        Icon::FloorPlan => {
            rect_s(1.5, 2.5, 14.5, 13.5, 0.5);
            line(&[(8.0, 2.5), (8.0, 7.0)]);
            line(&[(8.0, 9.5), (8.0, 13.5)]);
            seg((8.0, 8.0), (14.5, 8.0), thin);
        }
        Icon::CeilingPlan => {
            rect_s(1.5, 2.5, 14.5, 13.5, 0.5);
            for x in [5.75, 10.25] {
                seg((x, 2.5), (x, 13.5), thin);
            }
            seg((1.5, 8.0), (14.5, 8.0), thin);
        }
        Icon::View3D => {
            poly(&[(8.0, 1.5), (14.0, 4.8), (8.0, 8.1), (2.0, 4.8)], fill);
            line(&[(2.0, 4.8), (2.0, 11.2), (8.0, 14.5), (14.0, 11.2), (14.0, 4.8)]);
            seg((8.0, 8.1), (8.0, 14.5), s);
        }
        Icon::Section => {
            seg((8.0, 1.5), (8.0, 14.5), Stroke::new(w, color));
            poly(&[(8.0, 1.5), (13.5, 1.5), (8.0, 5.5)], color);
            poly(&[(8.0, 14.5), (13.5, 14.5), (8.0, 10.5)], color);
            seg((2.0, 8.0), (6.0, 8.0), thin);
        }
        Icon::Elevation => {
            painter.circle_stroke(g.p(8.0, 9.0), 4.5 * scale, s);
            poly(&[(8.0, 1.0), (12.5, 6.0), (3.5, 6.0)], color);
            seg((3.5, 9.0), (12.5, 9.0), thin);
        }
        Icon::Legend => {
            for (i, y) in [3.0, 8.0, 13.0].into_iter().enumerate() {
                let sw = g.rect(1.5, y - 1.8, 5.1, y + 1.8);
                painter.rect_filled(sw, 0.5 * scale, if i == 0 { color } else { fill });
                painter.rect_stroke(sw, 0.5 * scale, thin, StrokeKind::Middle);
                seg((7.5, y), (14.5, y), s);
            }
        }
        Icon::Schedule => {
            rect_s(1.5, 2.5, 14.5, 13.5, 1.0);
            painter.rect_filled(g.rect(1.5, 2.5, 14.5, 5.5), 0.0, fill);
            for y in [5.5, 8.2, 10.9] {
                seg((1.5, y), (14.5, y), thin);
            }
            seg((6.0, 2.5), (6.0, 13.5), thin);
        }
        Icon::Sheet => {
            rect_s(1.5, 3.0, 14.5, 13.0, 0.5);
            rect_s(10.0, 9.5, 14.5, 13.0, 0.0);
            rect_s(3.0, 4.5, 8.5, 11.5, 0.0);
        }
        Icon::ZoomFit => {
            for (cx, cy, dx, dy) in
                [(1.5, 1.5, 1.0, 1.0), (14.5, 1.5, -1.0, 1.0), (1.5, 14.5, 1.0, -1.0), (14.5, 14.5, -1.0, -1.0)]
            {
                line(&[(cx + 4.0 * dx, cy), (cx, cy), (cx, cy + 4.0 * dy)]);
            }
            rect_s(5.0, 5.0, 11.0, 11.0, 0.5);
        }
        Icon::ProjectInfo => {
            closed(&[(3.0, 1.5), (13.0, 1.5), (13.0, 14.5), (3.0, 14.5)]);
            for y in [5.0, 8.0, 11.0] {
                seg((5.5, y), (10.5, y), thin);
            }
        }
        Icon::Levels => {
            for y in [3.5, 8.0, 12.5] {
                seg((5.5, y), (15.0, y), s);
                poly(&[(1.0, y - 2.0), (4.5, y - 2.0), (2.75, y)], color);
            }
        }
        Icon::Materials => {
            painter.circle_filled(g.p(5.5, 6.0), 3.5 * scale, color);
            painter.circle_stroke(g.p(10.5, 6.0), 3.5 * scale, s);
            painter.circle_filled(g.p(8.0, 10.5), 3.5 * scale, fill);
            painter.circle_stroke(g.p(8.0, 10.5), 3.5 * scale, s);
        }
        Icon::ChevronDown => {
            line(&[(4.0, 6.0), (8.0, 10.0), (12.0, 6.0)]);
        }
        Icon::ChevronRight => {
            line(&[(6.0, 4.0), (10.0, 8.0), (6.0, 12.0)]);
        }
        Icon::Home => {
            closed(&[(8.0, 1.8), (14.5, 7.5), (12.8, 7.5), (12.8, 14.2), (3.2, 14.2), (3.2, 7.5), (1.5, 7.5)]);
            rect_s(6.6, 9.6, 9.4, 14.2, 0.0);
        }
        Icon::CloseProject => {
            closed(&[(3.5, 1.5), (9.5, 1.5), (12.5, 4.5), (12.5, 14.5), (3.5, 14.5)]);
            line(&[(9.5, 1.5), (9.5, 4.5), (12.5, 4.5)]);
            seg((6.0, 7.5), (10.0, 11.5), s);
            seg((10.0, 7.5), (6.0, 11.5), s);
        }
        Icon::Room => {
            rect_s(1.5, 2.5, 14.5, 13.5, 0.5);
            poly(&[(1.5, 2.5), (14.5, 2.5), (14.5, 13.5), (1.5, 13.5)], fill);
            seg((5.0, 7.0), (11.0, 7.0), thin);
            seg((6.0, 9.5), (10.0, 9.5), thin);
        }
        Icon::Grid => {
            painter.circle_stroke(g.p(8.0, 3.5), 2.8 * scale, s);
            seg((8.0, 6.3), (8.0, 8.5), s);
            seg((8.0, 10.0), (8.0, 11.0), s);
            seg((8.0, 12.5), (8.0, 15.0), s);
        }
        Icon::Dimension => {
            seg((1.5, 4.0), (1.5, 12.0), thin);
            seg((14.5, 4.0), (14.5, 12.0), thin);
            seg((1.5, 9.0), (14.5, 9.0), s);
            seg((0.5, 10.0), (2.5, 8.0), s);
            seg((13.5, 10.0), (15.5, 8.0), s);
            seg((5.5, 6.0), (10.5, 6.0), thin);
        }
        Icon::Close => {
            seg((3.0, 3.0), (13.0, 13.0), s);
            seg((13.0, 3.0), (3.0, 13.0), s);
        }
        Icon::ArrowLeft => {
            seg((13.5, 8.0), (2.5, 8.0), s);
            line(&[(7.0, 3.5), (2.5, 8.0), (7.0, 12.5)]);
        }
        Icon::Plus => {
            seg((8.0, 2.5), (8.0, 13.5), s);
            seg((2.5, 8.0), (13.5, 8.0), s);
        }
    }
}

/// Background color used to knock out icon parts (matches panels).
fn painter_bg(_painter: &Painter) -> Color32 {
    crate::theme::PALETTE.panel
}
