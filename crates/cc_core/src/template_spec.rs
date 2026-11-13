//! The sizes and names that differ between the metric and the imperial
//! templates. [`crate::Project::from_template_system`] builds both from the
//! same code and reads only these tables, so the two templates have the same
//! types, views and ids, in the same order.
//!
//! Lengths are meters, except annotation sizes (paper millimeters); imperial
//! values are written in inches or feet and kept exact multiples of 1/16".

use crate::units::{FOOT, INCH};
use crate::{MeasurementSystem, SheetSize};
use glam::DVec2;

/// A name key and a value.
pub(crate) type Sized1 = (&'static str, f64);
/// A name key and two values.
pub(crate) type Sized2 = (&'static str, f64, f64);
/// A name key and three values.
pub(crate) type Sized3 = (&'static str, f64, f64, f64);

/// A library type whose name states a size: the library's name key, the
/// key used instead, and the parameters (keys) set to that size.
pub(crate) type SizedLibraryType = (&'static str, &'static str, &'static [Sized1]);

pub(crate) struct TemplateSpec {
    /// Elevation of the upper level.
    pub first_level: f64,
    /// The two masonry walls: key, plaster on each side, masonry core.
    pub masonry: [Sized2; 2],
    /// The gypsum partition: key, thickness.
    pub partition: Sized1,
    /// The swing doors: key, width, height.
    pub doors: [Sized2; 2],
    /// The fixed windows: key, width, height, sill.
    pub windows: [Sized3; 2],
    /// The operable windows (side-hung, double side-hung, top-hung,
    /// sliding): key, width, height, sill.
    pub operable: [Sized3; 4],
    /// The floor slab: key, screed, concrete.
    pub slab: Sized2,
    /// Thickness of the gypsum ceiling.
    pub ceiling: f64,
    /// Square concrete column: key, side.
    pub column_rect: Sized1,
    /// Round concrete column: key, diameter.
    pub column_round: Sized1,
    /// Concrete beam: key, width, height.
    pub beam: Sized2,
    /// Steel H column: key, width and depth, web, flange.
    pub column_steel_h: Sized3,
    /// Steel square tube column: key, side, wall.
    pub column_steel_tube: Sized2,
    /// Steel I beam: key, (width, height, web, flange).
    pub beam_steel_i: (&'static str, [f64; 4]),
    /// Reinforcing bars: key, diameter.
    pub rebars: [Sized1; 5],
    /// Pad, strip and slab footings: key, width, length, thickness.
    pub footings: [Sized3; 3],
    /// Stair: key, width, maximum riser, tread.
    pub stair: Sized3,
    /// Railing: key, height, post spacing.
    pub railing: Sized2,
    /// Supply pipes, then drain pipes: key, diameter.
    pub pipes: [Sized1; 7],
    /// Conduits: key, diameter.
    pub conduits: [Sized1; 3],
    /// Grid: key, bubble size (paper mm).
    pub grid: Sized1,
    /// Linear dimension: key, text size (paper mm).
    pub dimension: Sized1,
    /// Texts: key, size (paper mm).
    pub texts: [Sized1; 2],
    /// Roof layers: tiles, wood.
    pub roof: (f64, f64),
    /// French doors (one leaf, two leaves): key, width, height.
    pub french_doors: [Sized2; 2],
    /// Scale of plans, ceiling plans and the legend.
    pub plan_scale: u32,
    /// Scale of the 3D view and the elevations.
    pub model_scale: u32,
    /// The sheet's paper and where the plan and the 3D view sit on it.
    pub sheet: (SheetSize, [DVec2; 2]),
    /// Library types whose name states a size: the library's key, the key
    /// used instead, and the parameters (keys) set to that size.
    pub library: &'static [SizedLibraryType],
}

impl TemplateSpec {
    pub(crate) fn of(system: MeasurementSystem) -> &'static TemplateSpec {
        match system {
            MeasurementSystem::Metric => &METRIC,
            MeasurementSystem::Imperial => &IMPERIAL,
        }
    }

    /// The key a library name is read from in this system.
    pub(crate) fn library_key<'a>(&self, key: &'a str) -> &'a str {
        self.library.iter().find(|(from, _, _)| *from == key).map_or(key, |(_, to, _)| to)
    }
}

const METRIC: TemplateSpec = TemplateSpec {
    first_level: 3.0,
    masonry: [("tpl-type-masonry-15", 0.02, 0.11), ("tpl-type-masonry-25", 0.025, 0.20)],
    partition: ("tpl-type-drywall-10", 0.10),
    doors: [("tpl-type-door-80", 0.8, 2.1), ("tpl-type-door-90", 0.9, 2.1)],
    windows: [("tpl-type-window-120", 1.2, 1.2, 1.0), ("tpl-type-window-150", 1.5, 1.0, 1.1)],
    operable: [
        ("tpl-type-window-casement-60", 0.6, 1.0, 1.1),
        ("tpl-type-window-double-120", 1.2, 1.2, 1.0),
        ("tpl-type-window-top-hung-60", 0.6, 0.6, 1.5),
        ("tpl-type-window-sliding-150", 1.5, 1.2, 1.0),
    ],
    slab: ("tpl-type-slab-12", 0.03, 0.12),
    ceiling: 0.02,
    column_rect: ("tpl-type-column-rect", 0.2),
    column_round: ("tpl-type-column-round", 0.3),
    beam: ("tpl-type-beam", 0.15, 0.4),
    column_steel_h: ("tpl-type-column-steel-h", 0.2, 0.009, 0.015),
    column_steel_tube: ("tpl-type-column-steel-tube", 0.15, 0.006),
    beam_steel_i: ("tpl-type-beam-steel-i", [0.15, 0.3, 0.0071, 0.0107]),
    rebars: [
        ("tpl-type-rebar-5", 0.005),
        ("tpl-type-rebar-8", 0.008),
        ("tpl-type-rebar-10", 0.010),
        ("tpl-type-rebar-12", 0.0125),
        ("tpl-type-rebar-16", 0.016),
    ],
    footings: [
        ("tpl-type-footing-pad", 1.0, 1.0, 0.4),
        ("tpl-type-footing-strip", 0.2, 1.0, 0.4),
        ("tpl-type-footing-slab", 1.0, 1.0, 0.3),
    ],
    stair: ("tpl-type-stair", 1.0, 0.18, 0.28),
    railing: ("tpl-type-railing", 1.1, 1.2),
    pipes: [
        ("tpl-type-pipe-20", 0.020),
        ("tpl-type-pipe-25", 0.025),
        ("tpl-type-pipe-32", 0.032),
        ("tpl-type-pipe-sewer-40", 0.040),
        ("tpl-type-pipe-sewer-50", 0.050),
        ("tpl-type-pipe-sewer-75", 0.075),
        ("tpl-type-pipe-sewer-100", 0.100),
    ],
    conduits: [("tpl-type-conduit-20", 0.020), ("tpl-type-conduit-25", 0.025), ("tpl-type-conduit-32", 0.032)],
    grid: ("tpl-type-grid", 8.0),
    dimension: ("tpl-type-dimension", 2.5),
    texts: [("tpl-type-text", 2.5), ("tpl-type-text-large", 3.5)],
    roof: (0.05, 0.10),
    french_doors: [("tpl-type-french-door-80", 0.8, 2.2), ("tpl-type-french-door-double-140", 1.4, 2.2)],
    plan_scale: 50,
    model_scale: 100,
    sheet: (SheetSize::A3, [DVec2::new(130.0, 160.0), DVec2::new(320.0, 190.0)]),
    library: &[],
};

/// Millimeters in an inch, for annotation sizes on paper.
const PAPER_INCH: f64 = 25.4;
/// ANSI B paper (17" x 11"), in millimeters.
const ANSI_B: (f64, f64) = (17.0 * PAPER_INCH, 11.0 * PAPER_INCH);

const IMPERIAL: TemplateSpec = TemplateSpec {
    first_level: 10.0 * FOOT,
    masonry: [
        ("tpl-imp-type-masonry-6", 0.75 * INCH, 4.5 * INCH),
        ("tpl-imp-type-masonry-10", 0.75 * INCH, 8.5 * INCH),
    ],
    partition: ("tpl-imp-type-drywall-4", 4.875 * INCH),
    doors: [("tpl-imp-type-door-32", 32.0 * INCH, 80.0 * INCH), ("tpl-imp-type-door-36", 36.0 * INCH, 80.0 * INCH)],
    windows: [
        ("tpl-imp-type-window-48", 48.0 * INCH, 48.0 * INCH, 36.0 * INCH),
        ("tpl-imp-type-window-60", 60.0 * INCH, 42.0 * INCH, 42.0 * INCH),
    ],
    operable: [
        ("tpl-imp-type-window-casement-24", 24.0 * INCH, 42.0 * INCH, 42.0 * INCH),
        ("tpl-imp-type-window-double-48", 48.0 * INCH, 48.0 * INCH, 36.0 * INCH),
        ("tpl-imp-type-window-top-hung-24", 24.0 * INCH, 24.0 * INCH, 60.0 * INCH),
        ("tpl-imp-type-window-sliding-60", 60.0 * INCH, 48.0 * INCH, 36.0 * INCH),
    ],
    slab: ("tpl-imp-type-slab-5", 1.0 * INCH, 5.0 * INCH),
    ceiling: 0.625 * INCH,
    column_rect: ("tpl-imp-type-column-rect", 8.0 * INCH),
    column_round: ("tpl-imp-type-column-round", 12.0 * INCH),
    beam: ("tpl-imp-type-beam", 6.0 * INCH, 16.0 * INCH),
    // About a W8x31: 8" deep and wide, 5/16" web, 7/16" flanges.
    column_steel_h: ("tpl-imp-type-column-steel-h", 8.0 * INCH, 0.3125 * INCH, 0.4375 * INCH),
    column_steel_tube: ("tpl-imp-type-column-steel-tube", 6.0 * INCH, 0.25 * INCH),
    // About a W12x26: 6 1/2" wide, 12" deep, 1/4" web, 3/8" flanges.
    beam_steel_i: ("tpl-imp-type-beam-steel-i", [6.5 * INCH, 12.0 * INCH, 0.25 * INCH, 0.375 * INCH]),
    // Bar sizes #n are n/8" across.
    rebars: [
        ("tpl-imp-type-rebar-3", 0.375 * INCH),
        ("tpl-imp-type-rebar-4", 0.5 * INCH),
        ("tpl-imp-type-rebar-5", 0.625 * INCH),
        ("tpl-imp-type-rebar-6", 0.75 * INCH),
        ("tpl-imp-type-rebar-8", 1.0 * INCH),
    ],
    footings: [
        ("tpl-imp-type-footing-pad", 3.0 * FOOT, 3.0 * FOOT, 12.0 * INCH),
        ("tpl-imp-type-footing-strip", 8.0 * INCH, 3.0 * FOOT, 16.0 * INCH),
        ("tpl-imp-type-footing-slab", 3.0 * FOOT, 3.0 * FOOT, 12.0 * INCH),
    ],
    stair: ("tpl-imp-type-stair", 3.0 * FOOT, 7.0 * INCH, 11.0 * INCH),
    railing: ("tpl-imp-type-railing", 42.0 * INCH, 4.0 * FOOT),
    pipes: [
        ("tpl-imp-type-pipe-1-2", 0.5 * INCH),
        ("tpl-imp-type-pipe-3-4", 0.75 * INCH),
        ("tpl-imp-type-pipe-1", 1.0 * INCH),
        ("tpl-imp-type-pipe-sewer-1-1-2", 1.5 * INCH),
        ("tpl-imp-type-pipe-sewer-2", 2.0 * INCH),
        ("tpl-imp-type-pipe-sewer-3", 3.0 * INCH),
        ("tpl-imp-type-pipe-sewer-4", 4.0 * INCH),
    ],
    conduits: [
        ("tpl-imp-type-conduit-1-2", 0.5 * INCH),
        ("tpl-imp-type-conduit-3-4", 0.75 * INCH),
        ("tpl-imp-type-conduit-1", 1.0 * INCH),
    ],
    grid: ("tpl-imp-type-grid", 0.375 * PAPER_INCH),
    dimension: ("tpl-imp-type-dimension", 3.0 / 32.0 * PAPER_INCH),
    texts: [("tpl-imp-type-text", 3.0 / 32.0 * PAPER_INCH), ("tpl-imp-type-text-large", 0.125 * PAPER_INCH)],
    roof: (2.0 * INCH, 4.0 * INCH),
    french_doors: [
        ("tpl-imp-type-french-door-32", 32.0 * INCH, 80.0 * INCH),
        ("tpl-imp-type-french-door-double-60", 60.0 * INCH, 80.0 * INCH),
    ],
    // 1/4" = 1'-0" and 1/8" = 1'-0".
    plan_scale: 48,
    model_scale: 96,
    // The metric sheet's viewports moved in proportion to the paper.
    sheet: (
        SheetSize::AnsiB,
        // ANSI B is SheetSize::default_for(Imperial).
        [
            DVec2::new(130.0 * ANSI_B.0 / 420.0, 160.0 * ANSI_B.1 / 297.0),
            DVec2::new(320.0 * ANSI_B.0 / 420.0, 190.0 * ANSI_B.1 / 297.0),
        ],
    ),
    library: &[
        ("tpl-type-table", "tpl-imp-type-table", &[("tpl-param-width", 48.0 * INCH), ("tpl-param-depth", 30.0 * INCH)]),
        ("tpl-type-bed", "tpl-imp-type-bed", &[("tpl-param-width", 60.0 * INCH), ("tpl-param-depth", 80.0 * INCH)]),
        ("tpl-type-wardrobe", "tpl-imp-type-wardrobe", &[("tpl-param-width", 60.0 * INCH)]),
        ("tpl-type-kitchen-sink", "tpl-imp-type-kitchen-sink", &[("tpl-param-width", 48.0 * INCH)]),
        (
            "tpl-type-floor-drain",
            "tpl-imp-type-floor-drain",
            &[("tpl-param-width", 6.0 * INCH), ("tpl-param-depth", 6.0 * INCH)],
        ),
    ],
};

impl TemplateSpec {
    /// `library` (component, type name, definition, as the component
    /// library makes it with names from `n`) with the types whose name
    /// states a size given that size in this system.
    pub(crate) fn sized_library(
        &self,
        n: &dyn Fn(&str) -> String,
        mut library: Vec<(String, String, crate::ComponentDef)>,
    ) -> Vec<(String, String, crate::ComponentDef)> {
        for (_, key, params) in self.library {
            let name = n(key);
            for (_, _, def) in library.iter_mut().filter(|(_, type_name, _)| *type_name == name) {
                for (param, value) in params.iter() {
                    let param = n(param);
                    def.params.iter_mut().filter(|p| p.name == param).for_each(|p| p.value = *value);
                }
            }
        }
        library
    }
}

#[cfg(test)]
mod tests {
    use crate::{Category, ElementTypeId, LevelId, MeasurementSystem, Project, TypeData, ViewId, ViewKind};

    fn imperial() -> Project {
        Project::from_template_system(MeasurementSystem::Imperial, &|_| None)
    }

    /// Pins the metric template as it was before the imperial one shared its
    /// code: ids, names, sizes and scales.
    #[test]
    fn metric_template_is_unchanged() {
        let p = Project::from_template();
        assert_eq!(p.measurement_system(), MeasurementSystem::Metric);
        assert_eq!(p.units, crate::Units::default());
        let levels: Vec<_> = p.levels.iter().map(|l| (l.id, l.elevation)).collect();
        assert_eq!(levels, [(LevelId(1), 0.0), (LevelId(2), 3.0)]);
        assert_eq!((p.types.len(), p.views.len(), p.materials.len(), p.profiles.len()), (64, 15, 13, 6));
        assert_eq!(p.next_id(), 102);
        for (id, name) in [
            (3, "Alvenaria 15 cm"),
            (5, "Divisória gesso 10 cm"),
            (6, "Porta 80x210"),
            (13, "Janela de correr 150x120"),
            (21, "Viga metálica I 300 x 150"),
            (26, "Barra Ø 16,0 mm"),
            (38, "Tubo PVC esgoto 100 mm"),
            (46, "Texto 3,5 mm"),
            (47, "Telhado cerâmico"),
            (48, "Mesa 120 x 80"),
            (54, "Armário 160 cm"),
            (57, "Ralo sifonado 150 mm"),
            (63, "Luminária de teto 100 VA"),
            (84, "Carimbo padrão"),
            (95, "Porta-janela 2 folhas 140x220"),
        ] {
            assert_eq!(p.types[&ElementTypeId(id)].name, name, "type {id}");
        }
        assert_eq!(p.types[&ElementTypeId(3)].thickness(), 0.15);
        assert_eq!(p.types[&ElementTypeId(6)].opening_size(), Some((0.8, 2.1)));
        let scales: Vec<(u64, u32)> = p.views.values().map(|v| (v.id.0, v.scale)).collect();
        assert_eq!(&scales[..6], [(69, 50), (70, 50), (71, 50), (72, 50), (73, 100), (74, 100)]);
        assert_eq!(p.views[&ViewId(78)].scale, 50);
        match &p.views[&ViewId(83)].kind {
            ViewKind::Sheet { size, viewports, .. } => {
                assert_eq!(*size, crate::SheetSize::A3);
                assert_eq!(viewports[1].center, glam::DVec2::new(320.0, 190.0));
            }
            other => panic!("not a sheet: {other:?}"),
        }
    }

    #[test]
    fn imperial_template_mirrors_the_metric_one() {
        let metric = Project::from_template();
        let p = imperial();
        assert_eq!(p.measurement_system(), MeasurementSystem::Imperial);
        assert_eq!(p.units, crate::Units::imperial());
        assert_eq!(p.levels.len(), metric.levels.len());
        assert_eq!(p.levels[1].elevation, 10.0 * crate::units::FOOT);
        assert_eq!(p.types.len(), metric.types.len());
        assert_eq!(p.views.len(), metric.views.len());
        assert_eq!(p.profiles.len(), metric.profiles.len());
        assert_eq!(p.next_id(), metric.next_id());
        // Same ids, same categories, in the same order.
        let kinds = |p: &Project| p.types.values().map(|t| (t.id, t.category())).collect::<Vec<_>>();
        assert_eq!(kinds(&p), kinds(&metric));
        let views = |p: &Project| p.views.keys().copied().collect::<Vec<_>>();
        assert_eq!(views(&p), views(&metric));
        let scales: Vec<u32> = p.views.values().map(|v| v.scale).collect();
        assert_eq!(&scales[..11], [48, 48, 48, 48, 96, 96, 96, 96, 96, 48, 1]);
        // The sheet's viewports stay on its paper.
        let sheet = p.views.values().find_map(|v| match &v.kind {
            ViewKind::Sheet { size, viewports, .. } => Some((*size, size.size_mm(), viewports.clone())),
            _ => None,
        });
        let Some((size, paper, viewports)) = sheet else { panic!("no sheet") };
        assert_eq!(size, crate::SheetSize::AnsiB);
        assert!((paper.x - 431.8).abs() < 1e-9 && (paper.y - 279.4).abs() < 1e-9);
        assert!(viewports.iter().all(|v| v.center.x > 0.0 && v.center.x < paper.x && v.center.y < paper.y));
    }

    #[test]
    fn imperial_template_loads_clean() {
        let mut p = imperial();
        assert_eq!(p.clone().repair(), 0);
        let text = crate::persist::to_ron(&p).expect("ron");
        let (back, repairs) = crate::persist::from_ron_with_repairs(&text).expect("load");
        assert_eq!(repairs, 0);
        assert_eq!(back.measurement_system(), MeasurementSystem::Imperial);
        assert_eq!(back.types.len(), p.types.len());
        // Loading adds no type: the annotation defaults are all there.
        let before = p.types.len();
        p.ensure_annotation_types();
        assert_eq!(p.types.len(), before);
    }

    /// An imperial project that lost its columns gets an imperial one back on load.
    #[test]
    fn imperial_projects_get_imperial_defaults_back() {
        let mut p = imperial();
        p.types.retain(|_, t| t.category() != Category::Column);
        p.ensure_annotation_types();
        let columns: Vec<_> = p.types.values().filter(|t| t.category() == Category::Column).collect();
        assert_eq!(columns.len(), 1);
        assert_eq!(columns[0].name, "Pilar 8\" x 8\"");
    }

    /// Whether `v` (meters) is a whole number of sixteenths of an inch.
    fn sixteenths(v: f64) -> bool {
        let n = v / crate::units::INCH * 16.0;
        v > 0.0 && (n - n.round()).abs() < 1e-6
    }

    #[test]
    fn imperial_sizes_are_round_inches() {
        let p = imperial();
        let mut checked = 0;
        for t in p.types.values() {
            let mut sizes: Vec<f64> = t.layers().iter().map(|l| l.thickness).collect();
            if let Some((w, h)) = t.opening_size() {
                sizes.extend([w, h]);
                if t.category() == Category::Window {
                    sizes.push(t.default_sill());
                }
            }
            match &t.data {
                TypeData::Column { section, .. } => match section {
                    crate::ColumnSection::Rectangular { width, depth } => sizes.extend([*width, *depth]),
                    crate::ColumnSection::Round { diameter } => sizes.push(*diameter),
                    crate::ColumnSection::Profile { width, depth, .. } => sizes.extend([*width, *depth]),
                    _ => {}
                },
                TypeData::Beam { width, height, .. } => sizes.extend([*width, *height]),
                TypeData::Rebar { diameter, .. } | TypeData::Pipe { diameter, .. } => sizes.push(*diameter),
                TypeData::Conduit { diameter, .. } => sizes.push(*diameter),
                TypeData::Foundation { width, length, thickness, .. } => sizes.extend([*width, *length, *thickness]),
                TypeData::Stair { width, max_riser, tread, .. } => sizes.extend([*width, *max_riser, *tread]),
                TypeData::Railing { height, post_spacing, .. } => sizes.extend([*height, *post_spacing]),
                TypeData::Ceiling { thickness, .. } => sizes.push(*thickness),
                _ => {}
            }
            for v in &sizes {
                assert!(sixteenths(*v), "{}: {v} m is not a whole 1/16\"", t.name);
            }
            checked += sizes.len();
        }
        assert!(checked > 70, "{checked}");
        for profile in p.profiles.values() {
            for (name, v) in profile.shape.params() {
                assert!(v == 0.0 || sixteenths(v), "{} {name}: {v}", profile.name);
            }
            assert!(!profile.name.contains("mm") && !profile.name.contains("cm"), "{}", profile.name);
        }
        assert!(p.levels.iter().all(|l| l.elevation == 0.0 || sixteenths(l.elevation)));
        // Sized library types take the size their name states.
        let table = p.types.values().find(|t| t.name == "Mesa 4'-0\" x 2'-6\"").expect("table");
        let TypeData::Component { def, .. } = &table.data else { panic!("not a component") };
        assert!(def.params.iter().any(|p| (p.value - 48.0 * crate::units::INCH).abs() < 1e-12));
    }

    #[test]
    fn imperial_names_state_no_metric_size() {
        let p = imperial();
        for t in p.types.values() {
            let name = &t.name;
            assert!(!name.contains(" cm") && !name.contains(" mm") && !name.ends_with(" m"), "{name}");
            assert!(!name.contains('?'), "missing name: {name}");
        }
    }
}
