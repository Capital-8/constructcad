use crate::format;
use cc_core::{Category, Element, ElementKind, Project};
use cc_geom::{ModelGeometry, polygon};

/// A computed schedule. Headers and the footer label are translation keys.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Table {
    pub headers: Vec<&'static str>,
    pub rows: Vec<Row>,
    /// Totals row aligned with `headers` (empty cells are blank).
    pub totals: Vec<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Row {
    pub element: cc_core::ElementId,
    pub cells: Vec<String>,
}

fn mark_order(a: &Element, b: &Element) -> std::cmp::Ordering {
    match (a.mark.parse::<u64>(), b.mark.parse::<u64>()) {
        (Ok(x), Ok(y)) => x.cmp(&y),
        _ => a.mark.cmp(&b.mark).then(a.id.cmp(&b.id)),
    }
}

pub fn schedule(project: &Project, geom: &ModelGeometry, category: Category) -> Table {
    let mut elements: Vec<&Element> = project.elements_of(category).collect();
    elements.sort_by(|a, b| mark_order(a, b));
    let count = elements.len().to_string();
    let ty = |e: &Element| project.type_name(e.type_id).to_string();
    let level = |e: &Element| project.level_name(e.level).to_string();

    match category {
        Category::Door | Category::Window => {
            let mut headers = vec!["col-mark", "col-type", "col-width", "col-height"];
            if category == Category::Window {
                headers.push("col-sill");
            }
            headers.extend(["col-level", "col-host", "col-comments"]);
            let rows = elements
                .iter()
                .map(|e| {
                    let (w, h) = project.element_type(e.type_id).and_then(|t| t.opening_size()).unwrap_or_default();
                    let hosted = e.as_hosted();
                    let mut cells = vec![e.mark.clone(), ty(e), format::meters(w), format::meters(h)];
                    if category == Category::Window {
                        cells.push(format::meters(hosted.map_or(0.0, |h| h.sill_height)));
                    }
                    let host = hosted
                        .and_then(|h| project.element(h.host))
                        .map(|w| if w.mark.is_empty() { w.id.to_string() } else { w.mark.clone() })
                        .unwrap_or_default();
                    cells.extend([level(e), host, e.comments.clone()]);
                    Row { element: e.id, cells }
                })
                .collect();
            let mut totals = vec![String::new(); headers.len()];
            totals[0] = count;
            Table { headers, rows, totals }
        }
        Category::Wall => {
            let headers = vec![
                "col-mark",
                "col-type",
                "col-level",
                "col-length",
                "col-height",
                "col-thickness",
                "col-area",
                "col-volume",
            ];
            let (mut area_sum, mut volume_sum, mut length_sum) = (0.0, 0.0, 0.0);
            let rows = elements
                .iter()
                .filter_map(|e| {
                    let ElementKind::Wall(w) = &e.kind else { return None };
                    let openings: f64 = project
                        .hosted_by(e.id)
                        .filter_map(|h| project.element_type(h.type_id)?.opening_size())
                        .map(|(w, h)| w * h)
                        .sum();
                    let area = (w.length() * w.height - openings).max(0.0);
                    let volume = geom.volume(e.id);
                    area_sum += area;
                    volume_sum += volume;
                    length_sum += w.length();
                    Some(Row {
                        element: e.id,
                        cells: vec![
                            e.mark.clone(),
                            ty(e),
                            level(e),
                            format::meters(w.length()),
                            format::meters(w.height),
                            format::meters(project.thickness_of(e)),
                            format::number(area, 2),
                            format::number(volume, 3),
                        ],
                    })
                })
                .collect();
            let totals = vec![
                count,
                String::new(),
                String::new(),
                format::meters(length_sum),
                String::new(),
                String::new(),
                format::number(area_sum, 2),
                format::number(volume_sum, 3),
            ];
            Table { headers, rows, totals }
        }
        Category::Floor | Category::Ceiling | Category::Roof => {
            let headers = vec!["col-mark", "col-type", "col-level", "col-area", "col-perimeter", "col-volume"];
            let (mut area_sum, mut volume_sum) = (0.0, 0.0);
            let rows = elements
                .iter()
                .filter_map(|e| {
                    let sketch = e.sketch()?;
                    let area = polygon::area(sketch);
                    let volume = geom.volume(e.id);
                    area_sum += area;
                    volume_sum += volume;
                    Some(Row {
                        element: e.id,
                        cells: vec![
                            e.mark.clone(),
                            ty(e),
                            level(e),
                            format::number(area, 2),
                            format::meters(polygon::perimeter(sketch)),
                            format::number(volume, 3),
                        ],
                    })
                })
                .collect();
            let totals = vec![
                count,
                String::new(),
                String::new(),
                format::number(area_sum, 2),
                String::new(),
                format::number(volume_sum, 3),
            ];
            Table { headers, rows, totals }
        }
    }
}
