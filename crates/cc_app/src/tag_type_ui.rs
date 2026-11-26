//! Tags, grid bubbles and view marks in the interface: the Type Properties
//! sections of tag types (text lines with `{Field}`s, symbol, sizes, pens,
//! colors, leader, orientation), of grid types (bubbles, text, pen, color,
//! line pattern) and of view mark types (head, texts, arrow, heads, line);
//! the Tag Types dialog (*Annotate → Tag Types*), which lists the tag
//! types per target and the view mark types and creates, duplicates and
//! opens them; the *Tag* section of a selected element's properties and the
//! tag type rows of a view's properties. Every change runs an engine
//! command: `type_set` (through [`crate::engine_edit::element_type`]),
//! `tag_type_create`, `view_mark_type_create`, `type_duplicate`, `tag_set`
//! and `view_set` (through [`crate::engine_edit::view`]).

use crate::dim_type_ui::{Edits, optional_row, weight_row};
use crate::i18n::tr;
use crate::icons::Icon;
use crate::properties::paper_drag;
use crate::state::AppState;
use crate::t;
use crate::table::{readonly_row, row, section};
use crate::theme::PALETTE;
use cc_core::grid::{BubbleEnds, GridStyle};
use cc_core::tag::{LeaderMode, TagLine, TagOrientation, TagStyle, TagSymbol, TagTarget, is_detail_component};
use cc_core::view_mark::{MarkArrow, MarkKind, MarkStyle, MarkText};
use cc_core::{Category, Element, ElementId, ElementKind, ElementType, ElementTypeId, LinePattern, TypeData, ViewId};
use cc_engine::Ref;
use cc_engine::commands::tags::TagSet;
use egui::{Context, Response, RichText, Ui};

// --- Names ---

/// The name of what a tag type tags.
pub fn target_label(target: TagTarget) -> String {
    match target {
        TagTarget::Door => t!("tag-target-door"),
        TagTarget::Window => t!("tag-target-window"),
        TagTarget::Room => t!("tag-target-room"),
        TagTarget::Column => t!("tag-target-column"),
        TagTarget::Beam => t!("tag-target-beam"),
        TagTarget::Foundation => t!("tag-target-foundation"),
        TagTarget::Pipe => t!("tag-target-pipe"),
        TagTarget::Conduit => t!("tag-target-conduit"),
        TagTarget::CableTray => t!("tag-target-cable-tray"),
    }
}

/// The name of what a view mark type marks.
pub fn mark_kind_label(kind: MarkKind) -> String {
    match kind {
        MarkKind::Section => t!("mark-kind-section"),
        MarkKind::Elevation => t!("mark-kind-elevation"),
    }
}

/// The shapes a tag, a grid bubble or a mark head may be drawn as.
const SHAPES: [TagSymbol; 9] = [
    TagSymbol::None,
    TagSymbol::Circle,
    TagSymbol::Hexagon,
    TagSymbol::Rectangle,
    TagSymbol::RoundedRectangle,
    TagSymbol::Ellipse,
    TagSymbol::Diamond,
    TagSymbol::Triangle,
    TagSymbol::Underline,
];

/// The name of a symbol (a detail component by its type's name).
fn symbol_label(project: &cc_core::Project, symbol: TagSymbol) -> String {
    match symbol {
        TagSymbol::None => t!("tag-symbol-none"),
        TagSymbol::Circle => t!("tag-symbol-circle"),
        TagSymbol::Hexagon => t!("tag-symbol-hexagon"),
        TagSymbol::Rectangle => t!("tag-symbol-rectangle"),
        TagSymbol::RoundedRectangle => t!("tag-symbol-rounded-rectangle"),
        TagSymbol::Ellipse => t!("tag-symbol-ellipse"),
        TagSymbol::Diamond => t!("tag-symbol-diamond"),
        TagSymbol::Triangle => t!("tag-symbol-triangle"),
        TagSymbol::Underline => t!("tag-symbol-underline"),
        TagSymbol::Component(id) => project.type_name(id).to_string(),
    }
}

fn leader_label(mode: LeaderMode) -> String {
    match mode {
        LeaderMode::Auto => t!("tag-leader-auto"),
        LeaderMode::Always => t!("tag-leader-always"),
        LeaderMode::Never => t!("tag-leader-never"),
    }
}

/// The leader modes and how commands name them.
const LEADERS: [(LeaderMode, &str); 3] =
    [(LeaderMode::Auto, "auto"), (LeaderMode::Always, "always"), (LeaderMode::Never, "never")];

fn orientation_label(o: TagOrientation) -> String {
    match o {
        TagOrientation::Horizontal => t!("tag-orientation-horizontal"),
        TagOrientation::AlongHost => t!("tag-orientation-along"),
    }
}

fn ends_label(ends: BubbleEnds) -> String {
    match ends {
        BubbleEnds::End => t!("bubble-ends-end"),
        BubbleEnds::Start => t!("bubble-ends-start"),
        BubbleEnds::Both => t!("bubble-ends-both"),
        BubbleEnds::None => t!("bubble-ends-none"),
    }
}

fn arrow_label(arrow: MarkArrow) -> String {
    match arrow {
        MarkArrow::Filled => t!("mark-arrow-filled"),
        MarkArrow::Narrow => t!("mark-arrow-narrow"),
        MarkArrow::Open => t!("mark-arrow-open"),
        MarkArrow::None => t!("mark-arrow-none"),
    }
}

/// The category whose schedule fields a target's tags may write.
fn target_category(target: TagTarget) -> Category {
    match target {
        TagTarget::Door => Category::Door,
        TagTarget::Window => Category::Window,
        TagTarget::Room => Category::Room,
        TagTarget::Column => Category::Column,
        TagTarget::Beam => Category::Beam,
        TagTarget::Foundation => Category::Foundation,
        TagTarget::Pipe => Category::Pipe,
        TagTarget::Conduit => Category::Conduit,
        TagTarget::CableTray => Category::CableTray,
    }
}

/// The `{Field}`s a tag of `target` may write: its name in braces and its
/// label, the tag keys first, then the target's schedule fields.
pub fn tag_fields(target: TagTarget) -> Vec<(String, String)> {
    let mut out = vec![("Mark".to_string(), tr("col-mark")), ("TypeName".to_string(), t!("tag-field-type-name"))];
    if matches!(target, TagTarget::Column | TagTarget::Beam | TagTarget::Foundation) {
        out.push(("Section".into(), t!("tag-field-section")));
    }
    // What plans of services have always written by their runs.
    let runs: &[(&str, &str)] = match target {
        TagTarget::Pipe => &[
            ("Systems", "tag-field-systems"),
            ("DiameterMark", "tag-field-diameter-mark"),
            ("SlopeMark", "tag-field-slope-mark"),
        ],
        TagTarget::Conduit => &[
            ("Systems", "tag-field-systems"),
            ("Circuits", "tag-field-circuits"),
            ("DiameterMark", "tag-field-diameter-mark"),
        ],
        TagTarget::CableTray => {
            &[("Systems", "tag-field-systems"), ("Circuits", "tag-field-circuits"), ("TraySize", "tag-field-tray-size")]
        }
        _ => &[],
    };
    out.extend(runs.iter().map(|(k, key)| (k.to_string(), tr(key))));
    for f in cc_core::ScheduleField::available(target_category(target)) {
        if f == cc_core::ScheduleField::Count {
            continue;
        }
        let Some(name) = serde_json::to_value(f).ok().and_then(|v| v.as_str().map(str::to_string)) else { continue };
        if !out.iter().any(|(k, _)| *k == name) {
            out.push((name, crate::properties::field_label(f)));
        }
    }
    out
}

/// The `{Field}`s a view mark may write.
fn mark_fields() -> Vec<(String, String)> {
    vec![
        ("Number".into(), t!("mark-field-number")),
        ("Sheet".into(), t!("mark-field-sheet")),
        ("Name".into(), t!("mark-field-name")),
        ("Letter".into(), t!("mark-field-letter")),
    ]
}

// --- Rows ---

/// A row choosing one of `options` (value and label).
fn combo_row<T: PartialEq + Copy>(
    ui: &mut Ui,
    edits: &mut Edits,
    label: &str,
    id: impl std::hash::Hash + std::fmt::Debug,
    value: &mut T,
    options: &[(T, String)],
) {
    let shown = options.iter().find(|(v, _)| v == value).map(|(_, l)| l.clone()).unwrap_or_default();
    let r = row(ui, label, |ui| {
        let before = *value;
        let mut r = egui::ComboBox::from_id_salt(id)
            .selected_text(shown)
            .show_ui(ui, |ui| {
                for (v, l) in options {
                    ui.selectable_value(value, *v, l.as_str());
                }
            })
            .response;
        if *value != before {
            r.mark_changed();
            edits.done = true;
        }
        r
    });
    edits.add(&r);
}

/// A row choosing a symbol: the shapes of `shapes` or one of the
/// project's detail components.
fn symbol_row(
    ui: &mut Ui,
    project: &cc_core::Project,
    edits: &mut Edits,
    id: &str,
    symbol: &mut TagSymbol,
    shapes: &[TagSymbol],
) {
    let mut options: Vec<(TagSymbol, String)> = shapes.iter().map(|s| (*s, symbol_label(project, *s))).collect();
    options.extend(
        project
            .types
            .values()
            .filter(|t| is_detail_component(Some(t)))
            .map(|t| (TagSymbol::Component(t.id), t.name.clone())),
    );
    combo_row(ui, edits, &t!("tag-symbol"), id, symbol, &options);
}

/// A paper size row from `min` to `max` millimeters.
fn mm_row(ui: &mut Ui, edits: &mut Edits, label: &str, value: &mut f64, (min, max): (f64, f64)) {
    let r = row(ui, label, |ui| ui.add(paper_drag(value).speed(0.05).range(min..=max)));
    edits.add(&r);
}

fn check_row(ui: &mut Ui, edits: &mut Edits, label: &str, value: &mut bool) {
    let r = row(ui, label, |ui| crate::table::checkbox(ui, value));
    edits.done |= r.changed();
    edits.add(&r);
}

fn color_row(ui: &mut Ui, edits: &mut Edits, label: &str, value: &mut Option<[u8; 3]>) {
    optional_row(ui, edits, label, value, [0, 0, 0], |ui, c| ui.color_edit_button_srgb(c));
}

/// A text template field with a menu inserting `{Field}`s of `fields`.
fn template_edit(
    ui: &mut Ui,
    edits: &mut Edits,
    id: impl std::hash::Hash + std::fmt::Debug,
    text: &mut String,
    (max, fields): (usize, &[(String, String)]),
) -> Response {
    let menu_w = 56.0;
    let w = (ui.available_width() - menu_w - 6.0).max(40.0);
    let mut r = ui.add(egui::TextEdit::singleline(text).id_salt(id).char_limit(max).desired_width(w));
    let menu = ui.menu_button(t!("tag-insert-field"), |ui| {
        let mut picked = None;
        egui::ScrollArea::vertical().max_height(260.0).show(ui, |ui| {
            for (key, label) in fields {
                if ui.button(format!("{label}  {{{key}}}")).clicked() {
                    picked = Some(key.clone());
                }
            }
        });
        if picked.is_some() {
            ui.close();
        }
        picked
    });
    if let Some(Some(key)) = menu.inner
        && text.chars().count() + key.chars().count() + 2 <= max
    {
        text.push_str(&format!("{{{key}}}"));
        r.mark_changed();
        edits.done = true;
    }
    r
}

// --- Type Properties ---

/// The sections of a tag, grid or view mark type; nothing for other types.
pub fn type_sections(ui: &mut Ui, state: &mut AppState, ty: &ElementType) {
    match &ty.data {
        TypeData::Tag { target, style } => tag_sections(ui, state, ty.id, *target, style),
        TypeData::Grid { bubble, style } => grid_sections(ui, state, ty.id, *bubble, style),
        TypeData::ViewMark { kind, style } => mark_sections(ui, state, ty.id, *kind, style),
        _ => {}
    }
}

/// Send the type's new data and end the merge when an edit ended.
fn apply(state: &mut AppState, tid: ElementTypeId, edits: &Edits, data: TypeData) {
    if edits.changed {
        let key = crate::properties::key((tid, "tag-type-style"));
        crate::engine_edit::element_type(state, tid, Some(key), |t| t.data = data);
    }
    if edits.done {
        state.session.doc.break_merge();
    }
}

fn tag_sections(ui: &mut Ui, state: &mut AppState, tid: ElementTypeId, target: TagTarget, style: &TagStyle) {
    use cc_core::tag::{MAX_LINES, MAX_PADDING, MAX_SYMBOL_SIZE, MAX_TEXT, MAX_TEXT_SIZE, MIN_TEXT_SIZE};
    let mut s = style.clone();
    let mut edits = Edits::default();
    let fields = tag_fields(target);
    section(ui, "tag-type-target", t!("section-identity"), |ui| {
        readonly_row(ui, &t!("tag-target"), target_label(target));
    });
    section(ui, "tag-type-text", t!("section-text"), |ui| {
        let n = s.lines.len();
        let mut action: Option<(usize, i8)> = None;
        for i in 0..n {
            let Some(line) = s.lines.get_mut(i) else { continue };
            let r = row(ui, &t!("tag-line", "n" => i + 1), |ui| {
                template_edit(ui, &mut edits, ("tag-line", tid.0, i), &mut line.text, (MAX_TEXT, &fields))
            });
            edits.add(&r);
            let r = row(ui, &t!("tag-line-size"), |ui| {
                let r = ui.add(paper_drag(&mut line.size).speed(0.05).range(MIN_TEXT_SIZE..=MAX_TEXT_SIZE));
                if ui.add_enabled(i > 0, egui::Button::new("↑")).on_hover_text(t!("tag-line-up")).clicked() {
                    action = Some((i, -1));
                }
                if ui.add_enabled(i + 1 < n, egui::Button::new("↓")).on_hover_text(t!("tag-line-down")).clicked() {
                    action = Some((i, 1));
                }
                if ui.button("✕").on_hover_text(t!("tag-line-remove")).clicked() {
                    action = Some((i, 0));
                }
                r
            });
            edits.add(&r);
        }
        crate::table::button_row(ui, |ui| {
            if ui.add_enabled(n < MAX_LINES, egui::Button::new(t!("tag-line-add"))).clicked() {
                let size = s.lines.last().map_or(2.5, |l| l.size);
                s.lines.push(TagLine { text: String::new(), size });
                edits.changed = true;
                edits.done = true;
            }
        });
        match action {
            Some((i, 0)) if i < s.lines.len() => {
                s.lines.remove(i);
            }
            Some((i, -1)) if i > 0 && i < s.lines.len() => s.lines.swap(i, i - 1),
            Some((i, 1)) if i + 1 < s.lines.len() => s.lines.swap(i, i + 1),
            _ => {}
        }
        if action.is_some() {
            edits.changed = true;
            edits.done = true;
        }
        color_row(ui, &mut edits, &t!("tag-text-color"), &mut s.text_color);
        check_row(ui, &mut edits, &t!("tag-fill"), &mut s.fill);
    });
    section(ui, "tag-type-graphics", t!("section-graphics"), |ui| {
        symbol_row(ui, state.project(), &mut edits, "tag-symbol", &mut s.symbol, &SHAPES);
        let tip = t!("tag-size-hint");
        for (label, v) in [(t!("tag-width"), &mut s.width), (t!("tag-height"), &mut s.height)] {
            let r = crate::table::row_tip(ui, &label, Some(&tip), |ui| {
                ui.add(paper_drag(v).speed(0.05).range(0.0..=MAX_SYMBOL_SIZE))
            });
            edits.add(&r);
        }
        mm_row(ui, &mut edits, &t!("tag-padding"), &mut s.padding, (0.0, MAX_PADDING));
        weight_row(ui, &mut edits, &t!("tag-pen"), &mut s.pen);
        color_row(ui, &mut edits, &t!("tag-color"), &mut s.color);
        if target == TagTarget::Room || s.crossed_when_unplaced {
            check_row(ui, &mut edits, &t!("tag-crossed"), &mut s.crossed_when_unplaced);
        }
    });
    section(ui, "tag-type-leader", t!("section-tag-leader"), |ui| {
        let options: Vec<_> = LEADERS.iter().map(|(m, _)| (*m, leader_label(*m))).collect();
        combo_row(ui, &mut edits, &t!("tag-leader"), ("tag-leader", tid.0), &mut s.leader, &options);
        let r = row(ui, &t!("tag-leader-end"), |ui| {
            let mut r = ui.push_id("tag-leader-end", |ui| {
                crate::text_tool::symbol_combo(ui, ("tag-leader-end", tid.0), &mut s.leader_end)
            });
            if r.inner {
                r.response.mark_changed();
                edits.done = true;
            }
            r.response
        });
        edits.add(&r);
        let options: Vec<_> =
            [TagOrientation::Horizontal, TagOrientation::AlongHost].map(|o| (o, orientation_label(o))).to_vec();
        combo_row(ui, &mut edits, &t!("tag-orientation"), ("tag-orientation", tid.0), &mut s.orientation, &options);
    });
    if s != *style {
        edits.changed = true;
    }
    apply(state, tid, &edits, TypeData::Tag { target, style: s });
}

fn grid_sections(ui: &mut Ui, state: &mut AppState, tid: ElementTypeId, bubble: f64, style: &GridStyle) {
    use cc_core::grid::{MAX_TEXT_SIZE, MIN_TEXT_SIZE};
    let (mut b, mut s) = (bubble, *style);
    let mut edits = Edits::default();
    section(ui, "grid-type-bubbles", t!("section-grid-bubbles"), |ui| {
        let options: Vec<_> = BubbleEnds::ALL.iter().map(|e| (*e, ends_label(*e))).collect();
        combo_row(ui, &mut edits, &t!("grid-ends"), ("grid-ends", tid.0), &mut s.ends, &options);
        symbol_row(ui, state.project(), &mut edits, "grid-symbol", &mut s.symbol, &SHAPES);
        mm_row(ui, &mut edits, &t!("grid-bubble-size"), &mut b, (1.0, 100.0));
        let default = s.text_height(b);
        optional_row(ui, &mut edits, &t!("grid-text-size"), &mut s.text_size, default, |ui, v| {
            ui.add(paper_drag(v).speed(0.05).range(MIN_TEXT_SIZE..=MAX_TEXT_SIZE))
        });
    });
    section(ui, "grid-type-graphics", t!("section-graphics"), |ui| {
        weight_row(ui, &mut edits, &t!("tag-pen"), &mut s.pen);
        color_row(ui, &mut edits, &t!("mat-color"), &mut s.color);
        let mut options = vec![(None, t!("grid-pattern-default"))];
        options.extend(LinePattern::ALL.iter().map(|p| (Some(*p), tr(p.key()))));
        combo_row(ui, &mut edits, &t!("grid-line-pattern"), ("grid-pattern", tid.0), &mut s.line_pattern, &options);
    });
    if b != bubble || s != *style {
        edits.changed = true;
    }
    apply(state, tid, &edits, TypeData::Grid { bubble: b, style: s });
}

/// The rows of one text of a mark: its template and size.
fn mark_text_rows(ui: &mut Ui, edits: &mut Edits, label: &str, id: (u64, &str), text: &mut MarkText) {
    use cc_core::view_mark::{MAX_TEXT, MAX_TEXT_SIZE, MIN_TEXT_SIZE};
    let fields = mark_fields();
    let r = row(ui, label, |ui| template_edit(ui, edits, ("mark-text", id), &mut text.text, (MAX_TEXT, &fields)));
    edits.add(&r);
    mm_row(ui, edits, &t!("tag-line-size"), &mut text.size, (MIN_TEXT_SIZE, MAX_TEXT_SIZE));
}

fn mark_sections(ui: &mut Ui, state: &mut AppState, tid: ElementTypeId, kind: MarkKind, style: &MarkStyle) {
    let mut s = style.clone();
    let mut edits = Edits::default();
    section(ui, "mark-type-kind", t!("section-identity"), |ui| {
        readonly_row(ui, &t!("tag-target"), mark_kind_label(kind));
    });
    section(ui, "mark-type-head", t!("section-mark-head"), |ui| {
        let r = crate::table::row_tip(ui, &t!("mark-head"), Some(&t!("mark-head-hint")), |ui| {
            ui.add(paper_drag(&mut s.head).speed(0.1).range(0.0..=cc_core::view_mark::MAX_HEAD))
        });
        edits.add(&r);
        let shapes = &SHAPES[..SHAPES.len() - 1];
        symbol_row(ui, state.project(), &mut edits, "mark-symbol", &mut s.symbol, shapes);
        check_row(ui, &mut edits, &t!("mark-split"), &mut s.split);
        color_row(ui, &mut edits, &t!("mat-color"), &mut s.color);
    });
    section(ui, "mark-type-text", t!("section-text"), |ui| {
        mark_text_rows(ui, &mut edits, &t!("mark-top"), (tid.0, "top"), &mut s.top);
        mark_text_rows(ui, &mut edits, &t!("mark-bottom"), (tid.0, "bottom"), &mut s.bottom);
        mark_text_rows(ui, &mut edits, &t!("mark-label"), (tid.0, "label"), &mut s.label);
    });
    section(ui, "mark-type-graphics", t!("section-graphics"), |ui| {
        let arrows = [MarkArrow::Filled, MarkArrow::Narrow, MarkArrow::Open, MarkArrow::None];
        let options: Vec<_> = arrows.iter().map(|a| (*a, arrow_label(*a))).collect();
        combo_row(ui, &mut edits, &t!("mark-arrow"), ("mark-arrow", tid.0), &mut s.arrow, &options);
        color_row(ui, &mut edits, &t!("mark-arrow-color"), &mut s.arrow_color);
        if kind == MarkKind::Section {
            let options: Vec<_> = BubbleEnds::ALL.iter().map(|e| (*e, ends_label(*e))).collect();
            combo_row(ui, &mut edits, &t!("mark-heads"), ("mark-heads", tid.0), &mut s.heads, &options);
            let r = row(ui, &t!("mark-line-weight"), |ui| {
                let range = cc_core::tag::MIN_PEN..=cc_core::tag::MAX_PEN;
                ui.add(crate::properties::weight_drag(&mut s.line_weight).speed(0.01).range(range))
            });
            edits.add(&r);
        }
    });
    if s != *style {
        edits.changed = true;
    }
    apply(state, tid, &edits, TypeData::ViewMark { kind, style: s });
}

// --- The Tag Types dialog ---

/// What a row of the Tag Types dialog stands for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Tag(TagTarget),
    Mark(MarkKind),
}

impl Kind {
    fn label(self) -> String {
        match self {
            Kind::Tag(t) => target_label(t),
            Kind::Mark(k) => mark_kind_label(k),
        }
    }

    /// The kind of a tag or view mark type.
    fn of(data: &TypeData) -> Option<Kind> {
        match data {
            TypeData::Tag { target, .. } => Some(Kind::Tag(*target)),
            TypeData::ViewMark { kind, .. } => Some(Kind::Mark(*kind)),
            _ => None,
        }
    }
}

/// A row of the dialog: a type, or the built-in default of a kind with none.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Pick {
    Type(ElementTypeId),
    Default(Kind),
}

/// The open Tag Types dialog.
pub struct TagTypesDialog {
    pub selected: Option<Pick>,
    edits: crate::graphics_dialog::Edits,
}

pub fn open_list(state: &mut AppState) {
    let edits = crate::graphics_dialog::Edits::begin(state);
    state.dialogs.tag_types = Some(TagTypesDialog { selected: None, edits });
}

/// The dialog's rows: every kind, with its types or its default.
fn rows(project: &cc_core::Project) -> Vec<(Kind, Pick, String)> {
    let kinds = TagTarget::ALL.map(Kind::Tag).into_iter().chain(MarkKind::ALL.map(Kind::Mark));
    let mut out = Vec::new();
    for kind in kinds {
        let types: Vec<(ElementTypeId, String)> = project
            .types
            .values()
            .filter(|t| Kind::of(&t.data) == Some(kind))
            .map(|t| (t.id, t.name.clone()))
            .collect();
        if types.is_empty() {
            out.push((kind, Pick::Default(kind), t!("tag-types-default")));
        }
        out.extend(types.into_iter().map(|(id, name)| (kind, Pick::Type(id), name)));
    }
    out
}

/// Create a type of `kind` with its default style; returns its id.
fn create(state: &mut AppState, kind: Kind) -> Option<ElementTypeId> {
    let out = match kind {
        Kind::Tag(target) => crate::actions::run_command(
            state,
            cc_engine::commands::tags::TagTypeCreate { target: target.code().into(), ..Default::default() },
        ),
        Kind::Mark(k) => crate::actions::run_command(
            state,
            cc_engine::commands::view_marks::ViewMarkTypeCreate { kind: k.code().into(), name: None, style: None },
        ),
    }?;
    out["id"].as_u64().map(ElementTypeId)
}

pub fn show(ctx: &Context, state: &mut AppState) {
    let Some(d) = state.dialogs.tag_types.as_ref() else { return };
    let mut selected = d.selected;
    let list = rows(state.project());
    if selected.is_some_and(|s| !list.iter().any(|(_, p, _)| *p == s)) {
        selected = None;
    }
    let mut ok = 0;
    let mut action: Option<&str> = None;
    let mut add_to: Option<Kind> = None;
    let response = crate::dialog::Dialog::new("tag-types", t!("dlg-tag-types")).width(720.0).show(
        ctx,
        |ui| {
            ui.add(egui::Label::new(RichText::new(t!("tag-types-hint")).size(12.0).color(PALETTE.text_muted)).wrap());
            ui.add_space(14.0);
            egui::ScrollArea::vertical().max_height(500.0).auto_shrink([false, true]).show(ui, |ui| {
                let groups = [
                    (t!("tag-types-tags"), TagTarget::ALL.map(Kind::Tag).to_vec()),
                    (t!("tag-types-marks"), MarkKind::ALL.map(Kind::Mark).to_vec()),
                ];
                for (g, (title, kinds)) in groups.into_iter().enumerate() {
                    if g > 0 {
                        ui.add_space(18.0);
                    }
                    crate::settings::list_title(ui, &title, None, |_| {});
                    crate::settings::card(ui, |ui| {
                        let count = kinds.len();
                        for (i, kind) in kinds.into_iter().enumerate() {
                            let picks: Vec<&(Kind, Pick, String)> =
                                list.iter().filter(|(k, _, _)| *k == kind).collect();
                            let height = 48.0;
                            crate::settings::card_row(
                                ui,
                                ("tag-kind", kind.label()),
                                height,
                                false,
                                i + 1 == count,
                                |ui| {
                                    kind_badge(ui, kind);
                                    crate::settings::cell(ui, 150.0, |ui| {
                                        ui.label(
                                            RichText::new(kind.label()).family(crate::theme::semibold()).size(13.0),
                                        );
                                    });
                                    let room = ui.available_width() - 40.0;
                                    crate::settings::cell(ui, room, |ui| {
                                        ui.spacing_mut().item_spacing.x = 6.0;
                                        for (_, pick, name) in picks {
                                            let r = chip(
                                                ui,
                                                name,
                                                selected == Some(*pick),
                                                matches!(pick, Pick::Default(_)),
                                            );
                                            if r.clicked() {
                                                selected = Some(*pick);
                                            }
                                            if r.double_clicked() {
                                                selected = Some(*pick);
                                                action = Some("edit");
                                            }
                                        }
                                    });
                                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                        let label = format!("{}: {}", t!("tag-types-new"), kind.label());
                                        if crate::settings::icon_button(ui, Icon::Plus, &label, true, false).clicked() {
                                            add_to = Some(kind);
                                        }
                                    });
                                },
                            );
                        }
                    });
                }
            });
            ui.add_space(12.0);
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 8.0;
                let some = selected.is_some();
                let is_type = matches!(selected, Some(Pick::Type(_)));
                if crate::settings::button(ui, some, &t!("tag-types-new")).clicked() {
                    action = Some("new");
                }
                if crate::settings::button(ui, is_type, &t!("type-duplicate")).clicked() {
                    action = Some("duplicate");
                }
                if crate::settings::button(ui, some, &t!("tag-types-edit")).clicked() {
                    action = Some("edit");
                }
                if let Some(pick) = selected
                    && let Some((kind, _, name)) = list.iter().find(|(_, p, _)| *p == pick)
                {
                    ui.add_space(6.0);
                    let text = format!("{} · {}", kind.label(), name);
                    ui.label(RichText::new(text).size(12.0).color(PALETTE.text_muted));
                }
            });
        },
        |f| {
            ok = f.primary(t!("action-ok"));
            f.secondary(t!("action-cancel"));
        },
    );
    if let Some(kind) = add_to
        && let Some(id) = create(state, kind)
    {
        selected = Some(Pick::Type(id));
    }
    let kind_of = |state: &AppState, pick: Pick| match pick {
        Pick::Default(kind) => Some(kind),
        Pick::Type(id) => state.project().element_type(id).and_then(|t| Kind::of(&t.data)),
    };
    let mut edit_type = None;
    match (action, selected) {
        (Some("new"), Some(pick)) => {
            if let Some(id) = kind_of(state, pick).and_then(|k| create(state, k)) {
                selected = Some(Pick::Type(id));
            }
        }
        (Some("duplicate"), Some(Pick::Type(id))) => {
            let name = t!("copy-of", "name" => state.project().type_name(id));
            let command = cc_engine::commands::types::TypeDuplicate {
                type_ref: Ref::Id(id.0),
                name: Some(name),
                elements: Vec::new(),
            };
            if let Some(new) = crate::actions::run_command(state, command).and_then(|o| o["id"].as_u64()) {
                selected = Some(Pick::Type(ElementTypeId(new)));
            }
        }
        // Editing a default first makes it a type.
        (Some("edit"), Some(Pick::Default(kind))) => edit_type = create(state, kind),
        (Some("edit"), Some(Pick::Type(id))) => edit_type = Some(id),
        _ => {}
    }
    if let Some(d) = state.dialogs.tag_types.as_mut() {
        d.selected = selected;
    }
    let close = match response.clicked {
        Some(i) => Some(i == ok),
        None if response.dismissed => Some(false),
        None => None,
    };
    // Editing a type keeps what the dialog did and hands over to Type Properties.
    let close = if edit_type.is_some() { Some(true) } else { close };
    if let Some(keep) = close
        && let Some(d) = state.dialogs.tag_types.take()
    {
        d.edits.finish(state, keep);
    }
    if let Some(id) = edit_type {
        state.dialogs.type_editor = Some(id);
    }
}

// --- Properties ---

/// The view where a selected element's tag is adjusted: the active view
/// when it is a floor or ceiling plan (for grids, also an elevation or a
/// section).
fn tag_view(state: &AppState, grid: bool) -> Option<ViewId> {
    let id = state.active_view?;
    let v = state.project().view(id)?;
    let fits = v.level().is_some() || (grid && MarkKind::of_view(&v.kind).is_some());
    fits.then_some(id)
}

/// Run `tag_set` on the tag of `id` in `view`, merged while `key` is dragged.
fn tag_set(state: &mut AppState, key: Option<u64>, command: TagSet) {
    crate::graphics_dialog::run(state, key, command);
}

fn tag_set_of(view: ViewId, id: ElementId) -> TagSet {
    TagSet { view: Ref::Id(view.0), ids: vec![id.0], ..Default::default() }
}

/// Put the tag of `id` in `view` back as the view draws it by default.
pub fn reset_tag(state: &mut AppState, view: ViewId, id: ElementId) {
    tag_set(state, None, TagSet { reset: true, ..tag_set_of(view, id) });
}

/// Hide the tag of `id` in `view` (a grid's bubbles).
pub fn hide_tag(state: &mut AppState, view: ViewId, id: ElementId) {
    tag_set(state, None, TagSet { hidden: Some(true), ..tag_set_of(view, id) });
}

/// The *Tag* section of a selected element that has a tag in the active
/// view (a tagged element in a plan, a grid in a plan, an elevation or a
/// section).
pub fn element_section(ui: &mut Ui, state: &mut AppState, el: &Element) {
    let grid = matches!(el.kind, ElementKind::Grid(_));
    let target = TagTarget::of(el, state.project());
    if target.is_none() && !grid {
        return;
    }
    let Some(view) = tag_view(state, grid) else { return };
    let id = el.id;
    let placement = state.project().view(view).and_then(|v| v.tags.get(&id).copied()).unwrap_or_default();
    section(ui, "element-tag", t!("section-tag"), |ui| {
        if let Some(target) = target {
            // The type of this element's tag in this view.
            let mut options = vec![(None, t!("tag-type-view"))];
            options.extend(state.project().tag_types_of(target).map(|t| (Some(t.id), t.name.clone())));
            let mut ty = placement.tag_type;
            let shown = options.iter().find(|(v, _)| *v == ty).map(|(_, l)| l.clone()).unwrap_or_default();
            row(ui, &t!("tag-type-element"), |ui| {
                egui::ComboBox::from_id_salt(("element-tag-type", id.0)).selected_text(shown).show_ui(ui, |ui| {
                    for (v, l) in &options {
                        ui.selectable_value(&mut ty, *v, l.as_str());
                    }
                });
            });
            if ty != placement.tag_type {
                let type_ref = Some(ty.map(|t| Ref::Id(t.0)));
                tag_set(state, None, TagSet { type_ref, ..tag_set_of(view, id) });
            }
            // An angle of its own, else as its style orients it.
            let mut rotation = placement.rotation_deg;
            let mut edits = Edits::default();
            optional_row(ui, &mut edits, &t!("prop-rotation"), &mut rotation, 0.0, |ui, v| {
                ui.add(egui::DragValue::new(v).speed(1.0).range(-360.0..=360.0).suffix("°"))
            });
            if edits.changed && rotation != placement.rotation_deg {
                let key = crate::properties::key((view, id, "tag-rotation"));
                tag_set(state, Some(key), TagSet { rotation: Some(rotation), ..tag_set_of(view, id) });
            }
            if edits.done {
                state.session.doc.break_merge();
            }
            let mut leader = placement.leader;
            let mut options = vec![(None, t!("tag-leader-type"))];
            options.extend(LEADERS.iter().map(|(m, _)| (Some(*m), leader_label(*m))));
            let shown = options.iter().find(|(v, _)| *v == leader).map(|(_, l)| l.clone()).unwrap_or_default();
            row(ui, &t!("tag-leader"), |ui| {
                egui::ComboBox::from_id_salt(("element-tag-leader", id.0)).selected_text(shown).show_ui(ui, |ui| {
                    for (v, l) in &options {
                        ui.selectable_value(&mut leader, *v, l.as_str());
                    }
                });
            });
            if leader != placement.leader {
                let name = leader.and_then(|m| LEADERS.iter().find(|(x, _)| *x == m)).map(|(_, n)| n.to_string());
                tag_set(state, None, TagSet { leader: Some(name), ..tag_set_of(view, id) });
            }
        }
        if let ElementKind::Grid(g) = &el.kind {
            // The grid's own bubble ends, else its type's.
            let mut ends = g.ends;
            let mut options = vec![(None, t!("grid-ends-type"))];
            options.extend(BubbleEnds::ALL.iter().map(|e| (Some(*e), ends_label(*e))));
            let shown = options.iter().find(|(v, _)| *v == ends).map(|(_, l)| l.clone()).unwrap_or_default();
            row(ui, &t!("grid-ends"), |ui| {
                egui::ComboBox::from_id_salt(("element-grid-ends", id.0)).selected_text(shown).show_ui(ui, |ui| {
                    for (v, l) in &options {
                        ui.selectable_value(&mut ends, *v, l.as_str());
                    }
                });
            });
            if ends != g.ends {
                crate::engine_edit::element(state, id, None, |e| {
                    if let ElementKind::Grid(g) = &mut e.kind {
                        g.ends = ends;
                    }
                });
            }
        }
        let mut hidden = placement.hidden;
        let label = if grid { t!("grid-hide-bubbles") } else { t!("tag-hidden") };
        if row(ui, &label, |ui| crate::table::checkbox(ui, &mut hidden)).changed() {
            tag_set(state, None, TagSet { hidden: Some(hidden), ..tag_set_of(view, id) });
        }
        crate::table::button_row(ui, |ui| {
            if ui.add_enabled(!placement.is_default(), egui::Button::new(t!("action-reset-tag"))).clicked() {
                reset_tag(state, view, id);
            }
        });
    });
}

/// A row choosing a view's type among `options` (`None`: automatic).
fn view_type_row(
    ui: &mut Ui,
    label: &str,
    id: impl std::hash::Hash + std::fmt::Debug,
    current: Option<ElementTypeId>,
    options: &[(Option<ElementTypeId>, String)],
) -> Option<Option<ElementTypeId>> {
    let mut value = current;
    let shown = options.iter().find(|(v, _)| *v == value).map(|(_, l)| l.clone()).unwrap_or_default();
    row(ui, label, |ui| {
        egui::ComboBox::from_id_salt(id).selected_text(shown).show_ui(ui, |ui| {
            for (v, l) in options {
                ui.selectable_value(&mut value, *v, l.as_str());
            }
        });
    });
    (value != current).then_some(value)
}

/// A view's tag rows: the tag type per target of a floor or ceiling plan,
/// the mark type of a section or an elevation.
pub fn view_rows(ui: &mut Ui, state: &mut AppState, id: ViewId) {
    let Some(view) = state.project().view(id).cloned() else { return };
    if view.level().is_some() {
        section(ui, "view-tag-types", t!("section-tag-types"), |ui| {
            for target in TagTarget::ALL {
                let mut options = vec![(None, t!("tag-type-auto"))];
                options.extend(state.project().tag_types_of(target).map(|t| (Some(t.id), t.name.clone())));
                let current = view.tag_types.get(&target).copied();
                let label = target_label(target);
                if let Some(new) = view_type_row(ui, &label, ("view-tag-type", id.0, target.code()), current, &options)
                {
                    crate::engine_edit::view(state, id, None, |v| match new {
                        Some(t) => {
                            v.tag_types.insert(target, t);
                        }
                        None => {
                            v.tag_types.remove(&target);
                        }
                    });
                }
            }
        });
    }
    if let Some(kind) = MarkKind::of_view(&view.kind)
        && !view.is_detail()
    {
        section(ui, "view-mark-type", t!("section-tag-types"), |ui| {
            let mut options = vec![(None, t!("tag-type-auto"))];
            options.extend(state.project().mark_types_of(kind).map(|t| (Some(t.id), t.name.clone())));
            let label = t!("prop-mark-type");
            if let Some(new) = view_type_row(ui, &label, ("view-mark-type", id.0), view.mark_type, &options) {
                crate::engine_edit::view(state, id, None, |v| v.mark_type = new);
            }
        });
    }
}

/// The icon of what a kind of tag or mark is for, on a tinted square.
fn kind_badge(ui: &mut egui::Ui, kind: Kind) {
    let icon = match kind {
        Kind::Tag(TagTarget::Door) => Icon::Door,
        Kind::Tag(TagTarget::Window) => Icon::Window,
        Kind::Tag(TagTarget::Room) => Icon::Room,
        Kind::Tag(TagTarget::Column) => Icon::Column,
        Kind::Tag(TagTarget::Beam) => Icon::Beam,
        Kind::Tag(TagTarget::Foundation) => Icon::Footing,
        Kind::Tag(TagTarget::Pipe) => Icon::Pipe,
        Kind::Tag(TagTarget::Conduit) => Icon::Conduit,
        Kind::Tag(TagTarget::CableTray) => Icon::CableTray,
        Kind::Mark(MarkKind::Section) => Icon::Section,
        Kind::Mark(MarkKind::Elevation) => Icon::Elevation,
    };
    let (rect, _) = ui.allocate_exact_size(egui::vec2(30.0, 30.0), egui::Sense::hover());
    ui.painter().rect_filled(rect, 6.0, PALETTE.accent_soft);
    crate::icons::paint(ui.painter(), rect.shrink(6.0), icon, PALETTE.accent_text);
}

/// A type as a rounded chip: picked ones in the accent, the built-in
/// default in muted italics.
fn chip(ui: &mut egui::Ui, name: &str, selected: bool, default: bool) -> egui::Response {
    let mut text = RichText::new(name).size(12.5);
    text = if default { text.italics().color(PALETTE.text_muted) } else { text.color(PALETTE.text) };
    if selected {
        text = text.color(PALETTE.accent_text);
    }
    let galley =
        egui::WidgetText::from(text).into_galley(ui, Some(egui::TextWrapMode::Truncate), 220.0, egui::TextStyle::Body);
    let (rect, r) = ui.allocate_exact_size(galley.size() + egui::vec2(22.0, 10.0), egui::Sense::click());
    let (fill, stroke) = if selected {
        (PALETTE.accent_soft, egui::Stroke::new(1.5, PALETTE.accent))
    } else if r.hovered() {
        (PALETTE.hover, egui::Stroke::new(1.0, PALETTE.border_strong))
    } else {
        (PALETTE.canvas, egui::Stroke::new(1.0, PALETTE.border))
    };
    ui.painter().rect(rect, rect.height() / 2.0, fill, stroke, egui::StrokeKind::Inside);
    ui.painter().galley(rect.center() - galley.size() / 2.0, galley, PALETTE.text);
    r.widget_info(|| egui::WidgetInfo::selected(egui::WidgetType::SelectableLabel, true, selected, name));
    r
}
