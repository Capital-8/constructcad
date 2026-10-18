//! View generation for ConstructCAD.
//!
//! Each [`cc_core::View`] is a query over the model. [`generate`] produces a
//! resolution-independent [`Drawing2D`] for graphical views; [`schedule`]
//! computes tables.

mod annotate;
mod drawing;
pub mod format;
mod legend;
mod plan;
mod projected;
mod schedule;

pub use annotate::HIGHLIGHT_ONLY;
pub use drawing::{Anchor, Drawing2D, LineStyle, Prim};
pub use legend::legend;
pub use plan::plan;
pub use projected::{Frame, axonometric, elevation, section};
pub use schedule::{Row, Table, schedule};

use cc_core::{Project, View, ViewKind};
use cc_geom::ModelGeometry;

/// Drawing for any graphical view (`None` for schedules and sheets).
pub fn generate(project: &Project, geom: &ModelGeometry, view: &View) -> Option<Drawing2D> {
    let mut d = match &view.kind {
        ViewKind::FloorPlan { level, cut_height } | ViewKind::CeilingPlan { level, cut_height } => {
            let ceiling = matches!(view.kind, ViewKind::CeilingPlan { .. });
            let mut d = plan(project, geom, *level, *cut_height, ceiling);
            annotate::plan_annotations(&mut d, project, geom, *level, ceiling, view.scale);
            d
        }
        ViewKind::Elevation { direction } => elevation(project, geom, *direction),
        ViewKind::Section { start, end, depth } => section(project, geom, *start, *end, *depth),
        ViewKind::ThreeD { camera } => axonometric(project, geom, camera),
        ViewKind::Legend { types } => legend(project, types, view.scale),
        ViewKind::Schedule { .. } | ViewKind::Sheet { .. } => return None,
    };
    annotate::dimensions(&mut d, project, view.id, view.scale);
    Some(d)
}

#[cfg(test)]
mod tests {
    use super::*;
    use cc_core::{Category, ViewCategory};

    fn sample() -> (Project, ModelGeometry) {
        let project = cc_core::from_ron(include_str!("../../../examples/casa_simples.ccad")).unwrap();
        let geom = ModelGeometry::build(&project);
        (project, geom)
    }

    #[test]
    fn ground_plan_shows_rooms_grids_and_dimensions() {
        let (project, geom) = sample();
        let level = project.levels[0].id;
        let view = project.views_of(ViewCategory::FloorPlan).find(|v| v.level() == Some(level)).unwrap();
        let d = generate(&project, &geom, view).unwrap();
        let texts: Vec<&str> = d
            .prims
            .iter()
            .filter_map(|p| match p {
                Prim::Text { text, .. } => Some(text.as_str()),
                _ => None,
            })
            .collect();
        for expected in ["Sala", "Quarto", "1", "A", "B"] {
            assert!(texts.contains(&expected), "missing {expected:?} in {texts:?}");
        }
        // The overall dimension of the south wall.
        assert!(texts.contains(&format::number(9.0, 2).as_str()), "{texts:?}");
        assert!(d.prims.iter().any(|p| matches!(p, Prim::Line { style: LineStyle::Center, .. })));
        // Door and window marks are tagged.
        assert!(texts.iter().filter(|t| **t == "2").count() >= 2);

        // The upper plan has the grids but neither the rooms nor the dimensions.
        let upper =
            project.views_of(ViewCategory::FloorPlan).find(|v| v.level() == Some(project.levels[1].id)).unwrap();
        let d = generate(&project, &geom, upper).unwrap();
        let has = |s: &str| d.prims.iter().any(|p| matches!(p, Prim::Text { text, .. } if text == s));
        assert!(has("A") && !has("Sala") && !has(&format::number(9.0, 2)));
    }

    #[test]
    fn room_schedule_lists_enclosed_rooms() {
        let (project, geom) = sample();
        let table = schedule(&project, &geom, Category::Room);
        assert_eq!(table.rows.len(), 2);
        // Interior of the left room: 5 m minus half walls, by 6 m minus half walls.
        let area = (5.0 - 0.075 - 0.075) * (6.0 - 0.15);
        assert_eq!(table.rows[0].cells[3], format::number(area, 2));
    }
}
