//! App preferences (not model data): what they hold, how they are kept in
//! eframe's storage (every key lives here), how the interface follows them,
//! and the Preferences window (File tab, Ctrl+,, home screen).
//!
//! Stored values are hostile input: numbers are parsed and clamped, lists
//! are bounded, and folders that no longer exist are kept but skipped where
//! they are used. Missing values take defaults that match the behavior
//! before the window existed.

use crate::dialog::Dialog;
use crate::i18n::Language;
use crate::state::AppState;
use crate::t;
use crate::table;
use crate::theme::{self, PALETTE, ThemeChoice};
use egui::{Align, Context, Layout, RichText, Ui};
use std::path::{Path, PathBuf};
use std::time::Duration;

/// eframe storage key of the UI language.
const LANGUAGE_KEY: &str = "language";
/// eframe storage key of the recent projects (one path per line).
const RECENT_KEY: &str = "recent_files";
/// eframe storage key of the component library folders (one path per line).
const LIBRARY_KEY: &str = "component_library_folders";
/// eframe storage key of the interface theme ("system", "light" or "dark").
const THEME_KEY: &str = "theme";
/// eframe storage key of the start-collapsed ribbon preference ("1" when collapsed).
const RIBBON_KEY: &str = "ribbon_collapsed";
/// eframe storage key of the automatic update check preference ("0" when off).
const UPDATE_AUTO_KEY: &str = "update_check_auto";
/// eframe storage key of the time of the last update check (seconds since the Unix epoch).
const UPDATE_LAST_KEY: &str = "update_check_last";
/// eframe storage key of the version whose update notice the user skipped.
const UPDATE_SKIPPED_KEY: &str = "update_skipped_version";
/// eframe storage key of the interface scale (percent).
const UI_SCALE_KEY: &str = "ui_scale";
/// eframe storage key of the font size (percent of the normal size).
const FONT_SIZE_KEY: &str = "font_size";
/// eframe storage key of the detailed tooltips preference ("0" when off).
const TOOLTIPS_KEY: &str = "detailed_tooltips";
/// eframe storage key of the 3D left-drag preference ("1" when it selects).
const SELECT_3D_KEY: &str = "select_drag_3d";
/// eframe storage key of the macros folder.
const MACRO_FOLDER_KEY: &str = "macro_folder";
/// eframe storage key of the default projects folder.
const PROJECTS_FOLDER_KEY: &str = "projects_folder";
/// eframe storage key of the autosave interval (minutes, 0 = off).
const AUTOSAVE_KEY: &str = "autosave_minutes";
/// eframe storage key of the number of recent projects kept.
const RECENT_COUNT_KEY: &str = "recent_count";

/// Interface scales offered (percent).
pub const UI_SCALES: [u32; 8] = [75, 90, 100, 110, 125, 150, 175, 200];
/// Font sizes offered (percent of the normal size) and their names.
const FONT_SIZES: [(u32, &str); 4] =
    [(90, "pref-font-small"), (100, "pref-font-normal"), (115, "pref-font-large"), (130, "pref-font-larger")];
/// Autosave intervals offered (minutes; 0 turns autosave off).
pub const AUTOSAVE_MINUTES: [u32; 6] = [0, 1, 2, 5, 10, 15];
/// Numbers of recent projects offered.
pub const RECENT_COUNTS: [usize; 6] = [5, 8, 12, 16, 20, 30];
/// At most this many library folders are kept.
const MAX_LIBRARY_FOLDERS: usize = 64;
/// At most this many recent projects are kept.
const MAX_RECENT: usize = 50;
/// The macro menu lists at most this many files.
pub const MAX_MACROS: usize = 200;
/// Directory entries looked at when listing macros (a huge folder is not read to the end).
const MAX_MACRO_SCAN: usize = 10_000;

/// The app preferences. Language and the update check's bookkeeping are kept
/// elsewhere (`i18n`, `Updates`), but stored and restored here too.
#[derive(Debug, Clone, PartialEq)]
pub struct Preferences {
    pub theme: ThemeChoice,
    /// Check for a newer release at startup (at most once a day).
    pub check_updates: bool,
    /// Interface scale in percent (the egui zoom factor), within [`UI_SCALES`]'s range.
    pub ui_scale: u32,
    /// Text size in percent of the normal size, independent of the scale.
    pub font_size: u32,
    /// The ribbon starts collapsed (only its tabs shown).
    pub ribbon_collapsed: bool,
    /// Tooltips of commands show what they do and the engine command, not only the name.
    pub detailed_tooltips: bool,
    /// A left drag in 3D views draws a selection box (Shift+drag orbits);
    /// otherwise it orbits and Shift+drag selects.
    pub select_drag_3d: bool,
    /// Folders of the component library.
    pub library_folders: Vec<PathBuf>,
    /// Where macros are recorded to and listed from.
    pub macro_folder: Option<PathBuf>,
    /// Where the project file dialogs start.
    pub projects_folder: Option<PathBuf>,
    /// Minutes between autosave copies of unsaved changes (0: off).
    pub autosave_minutes: u32,
    /// How many recent projects the home screen lists.
    pub recent_count: usize,
}

impl Default for Preferences {
    fn default() -> Self {
        Self {
            theme: ThemeChoice::System,
            check_updates: true,
            ui_scale: 100,
            font_size: 100,
            ribbon_collapsed: false,
            detailed_tooltips: true,
            select_drag_3d: false,
            library_folders: Vec::new(),
            macro_folder: None,
            projects_folder: None,
            autosave_minutes: 1,
            recent_count: 12,
        }
    }
}

fn clamp_scale(percent: u32) -> u32 {
    percent.clamp(UI_SCALES[0], UI_SCALES[UI_SCALES.len() - 1])
}

fn clamp_font(percent: u32) -> u32 {
    let (lo, hi) = (*theme::FONT_SCALE_RANGE.start(), *theme::FONT_SCALE_RANGE.end());
    percent.clamp((lo * 100.0).round() as u32, (hi * 100.0).round() as u32)
}

/// A zoom factor as a valid interface scale (percent).
fn scale_of_zoom(zoom: f32) -> u32 {
    if zoom.is_finite() && zoom > 0.0 { clamp_scale((zoom * 100.0).round().min(1000.0) as u32) } else { 100 }
}

/// Paths stored one per line: empty lines dropped, duplicates removed, at most `max`.
fn path_lines(text: &str, max: usize) -> Vec<PathBuf> {
    let mut paths: Vec<PathBuf> = Vec::new();
    for line in text.lines().map(|l| l.trim_end_matches('\r')).filter(|l| !l.trim().is_empty()) {
        let path = PathBuf::from(line);
        if !paths.contains(&path) {
            paths.push(path);
        }
        if paths.len() >= max {
            break;
        }
    }
    paths
}

fn join_paths(paths: &[PathBuf]) -> String {
    paths.iter().map(|p| p.to_string_lossy().into_owned()).collect::<Vec<_>>().join("\n")
}

fn folder(text: Option<String>) -> Option<PathBuf> {
    text.map(|t| t.trim_end_matches(['\r', '\n']).to_string()).filter(|t| !t.trim().is_empty()).map(PathBuf::from)
}

fn flag(on: bool) -> String {
    if on { "1" } else { "0" }.to_string()
}

impl Preferences {
    /// Read the preferences with `get` (a storage lookup). `zoom` is the
    /// interface zoom eframe restored, used when no scale was stored yet
    /// (it kept the Ctrl+Plus/Minus zoom before this preference existed).
    pub fn read(get: impl Fn(&str) -> Option<String>, zoom: f32) -> Self {
        let d = Self::default();
        let number = |key| get(key).and_then(|v| v.trim().parse::<u64>().ok());
        let on = |key, default: bool| get(key).map_or(default, |v| if default { v != "0" } else { v == "1" });
        Self {
            theme: get(THEME_KEY).and_then(|v| ThemeChoice::from_code(&v)).unwrap_or(d.theme),
            check_updates: on(UPDATE_AUTO_KEY, d.check_updates),
            ui_scale: number(UI_SCALE_KEY)
                .map_or_else(|| scale_of_zoom(zoom), |v| clamp_scale(v.min(u32::MAX as u64) as u32)),
            font_size: number(FONT_SIZE_KEY).map_or(d.font_size, |v| clamp_font(v.min(u32::MAX as u64) as u32)),
            ribbon_collapsed: on(RIBBON_KEY, d.ribbon_collapsed),
            detailed_tooltips: on(TOOLTIPS_KEY, d.detailed_tooltips),
            select_drag_3d: on(SELECT_3D_KEY, d.select_drag_3d),
            library_folders: get(LIBRARY_KEY).map(|v| path_lines(&v, MAX_LIBRARY_FOLDERS)).unwrap_or_default(),
            macro_folder: folder(get(MACRO_FOLDER_KEY)),
            projects_folder: folder(get(PROJECTS_FOLDER_KEY)),
            autosave_minutes: number(AUTOSAVE_KEY).map_or(d.autosave_minutes, |v| v.min(60) as u32),
            recent_count: number(RECENT_COUNT_KEY).map_or(d.recent_count, |v| v.clamp(1, MAX_RECENT as u64) as usize),
        }
    }

    /// Write the preferences with `set` (a storage write).
    pub fn write(&self, mut set: impl FnMut(&str, String)) {
        set(THEME_KEY, self.theme.code().to_string());
        set(UPDATE_AUTO_KEY, flag(self.check_updates));
        set(UI_SCALE_KEY, self.ui_scale.to_string());
        set(FONT_SIZE_KEY, self.font_size.to_string());
        set(RIBBON_KEY, flag(self.ribbon_collapsed));
        set(TOOLTIPS_KEY, flag(self.detailed_tooltips));
        set(SELECT_3D_KEY, flag(self.select_drag_3d));
        set(LIBRARY_KEY, join_paths(&self.library_folders));
        let text = |p: &Option<PathBuf>| p.as_ref().map(|p| p.to_string_lossy().into_owned()).unwrap_or_default();
        set(MACRO_FOLDER_KEY, text(&self.macro_folder));
        set(PROJECTS_FOLDER_KEY, text(&self.projects_folder));
        set(AUTOSAVE_KEY, self.autosave_minutes.to_string());
        set(RECENT_COUNT_KEY, self.recent_count.to_string());
    }

    /// Time between autosave copies, or `None` when autosave is off.
    pub fn autosave_interval(&self) -> Option<Duration> {
        (self.autosave_minutes > 0).then(|| Duration::from_secs(u64::from(self.autosave_minutes) * 60))
    }

    /// The macros folder, if set and present.
    pub fn macro_dir(&self) -> Option<&Path> {
        self.macro_folder.as_deref().filter(|p| p.is_dir())
    }

    /// The default projects folder, if set and present.
    pub fn projects_dir(&self) -> Option<&Path> {
        self.projects_folder.as_deref().filter(|p| p.is_dir())
    }

    /// The interface zoom factor.
    pub fn zoom(&self) -> f32 {
        clamp_scale(self.ui_scale) as f32 / 100.0
    }

    /// The font size multiplier.
    pub fn font_scale(&self) -> f32 {
        clamp_font(self.font_size) as f32 / 100.0
    }
}

/// Set the UI language at startup: `language` (from the command line), else
/// the stored one, else the operating system's.
pub fn load_language(get: impl Fn(&str) -> Option<String>, language: Option<Language>) {
    let saved = get(LANGUAGE_KEY).and_then(|c| Language::from_code(&c));
    crate::i18n::set_language(language.or(saved).unwrap_or_else(Language::from_system));
}

/// Restore every other stored preference into `state` at startup (`zoom`: see [`Preferences::read`]).
pub fn load(state: &mut AppState, get: impl Fn(&str) -> Option<String>, zoom: f32) {
    state.prefs = Preferences::read(&get, zoom);
    state.ribbon_collapsed = state.prefs.ribbon_collapsed;
    state.recent = get(RECENT_KEY).map(|v| path_lines(&v, state.prefs.recent_count)).unwrap_or_default();
    state.updates.last_check = get(UPDATE_LAST_KEY).and_then(|v| v.trim().parse().ok());
    state.updates.skipped = get(UPDATE_SKIPPED_KEY).and_then(|v| crate::update::Version::parse(&v));
}

/// Store every preference of `state` (and the language).
pub fn save(state: &AppState, mut set: impl FnMut(&str, String)) {
    set(LANGUAGE_KEY, crate::i18n::language().code().to_string());
    state.prefs.write(&mut set);
    let recent: Vec<PathBuf> = state.recent.iter().take(state.prefs.recent_count).cloned().collect();
    set(RECENT_KEY, join_paths(&recent));
    let updates = &state.updates;
    set(UPDATE_LAST_KEY, updates.last_check.map(|t| t.to_string()).unwrap_or_default());
    set(UPDATE_SKIPPED_KEY, updates.skipped.map(|v| v.to_string()).unwrap_or_default());
}

/// What the interface last applied of the preferences (theme, zoom, font size).
#[derive(Debug, Clone, Copy)]
pub struct Look {
    zoom: f32,
    font: f32,
    theme: Option<ThemeChoice>,
}

impl Look {
    /// The look `ctx` shows now.
    pub fn current(ctx: &Context) -> Self {
        Self { zoom: ctx.zoom_factor(), font: theme::font_scale(), theme: None }
    }
}

/// Make the interface follow the preferences; call it once at the start of
/// every frame. A zoom changed with Ctrl+Plus/Minus since the last frame
/// becomes the scale preference.
pub fn sync_look(ctx: &Context, prefs: &mut Preferences, look: &mut Look) {
    let zoom = ctx.zoom_factor();
    if (zoom - look.zoom).abs() > 1e-3 {
        prefs.ui_scale = scale_of_zoom(zoom);
        look.zoom = zoom;
    }
    if (prefs.zoom() - look.zoom).abs() > 1e-3 {
        look.zoom = prefs.zoom();
        ctx.set_zoom_factor(look.zoom);
    }
    if (prefs.font_scale() - look.font).abs() > 1e-3 {
        look.font = prefs.font_scale();
        theme::set_font_scale(ctx, look.font);
    }
    if look.theme != Some(prefs.theme) {
        look.theme = Some(prefs.theme);
        theme::set_theme(ctx, prefs.theme);
    }
}

/// Keep at most `prefs.recent_count` recent projects.
pub fn trim_recent(state: &mut AppState) {
    let count = state.prefs.recent_count.clamp(1, MAX_RECENT);
    state.recent.truncate(count);
}

/// The macro files (`.ccmacro`) of `folder`, sorted by name: not recursive,
/// at most [`MAX_MACROS`]; a missing or unreadable folder has none.
pub fn macro_files(folder: &Path) -> Vec<PathBuf> {
    let Ok(entries) = std::fs::read_dir(folder) else { return Vec::new() };
    let mut files: Vec<PathBuf> = entries
        .take(MAX_MACRO_SCAN)
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|e| e.eq_ignore_ascii_case("ccmacro")) && p.is_file())
        .collect();
    files.sort_by_key(|p| p.file_name().map(|n| n.to_string_lossy().to_lowercase()));
    files.truncate(MAX_MACROS);
    files
}

// --- The Preferences window ---

/// A page of the Preferences window.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Section {
    #[default]
    General,
    Interface,
    FileLocations,
    Saving,
}

impl Section {
    pub const ALL: [Section; 4] = [Section::General, Section::Interface, Section::FileLocations, Section::Saving];

    pub fn label(self) -> String {
        match self {
            Section::General => t!("pref-general"),
            Section::Interface => t!("pref-interface"),
            Section::FileLocations => t!("pref-file-locations"),
            Section::Saving => t!("pref-saving"),
        }
    }
}

/// What Cancel brings back: the preferences, language and ribbon state when the window opened.
#[derive(Debug, Clone)]
struct Snapshot {
    prefs: Preferences,
    language: Language,
    ribbon_collapsed: bool,
}

/// State of the open Preferences window. Changes show at once; Cancel
/// restores the values the window opened with.
#[derive(Debug, Clone)]
pub struct PreferencesDialog {
    pub section: Section,
    snapshot: Snapshot,
}

/// Open the Preferences window on `section` (or show that section if it is open).
pub fn open(state: &mut AppState, section: Section) {
    if let Some(dialog) = &mut state.dialogs.preferences {
        dialog.section = section;
        return;
    }
    let snapshot = Snapshot {
        prefs: state.prefs.clone(),
        language: crate::i18n::language(),
        ribbon_collapsed: state.ribbon_collapsed,
    };
    state.dialogs.preferences = Some(PreferencesDialog { section, snapshot });
}

fn restore(state: &mut AppState, snapshot: Snapshot) {
    state.prefs = snapshot.prefs;
    state.ribbon_collapsed = snapshot.ribbon_collapsed;
    crate::actions::set_language(state, snapshot.language);
}

pub fn show(ctx: &Context, state: &mut AppState) {
    let Some(mut dialog) = state.dialogs.preferences.take() else { return };
    let before = state.prefs.clone();
    let mut ok = 0;
    let response = Dialog::new("preferences", t!("dlg-preferences")).width(760.0).show(
        ctx,
        |ui| body(ui, state, &mut dialog),
        |f| {
            ok = f.primary(t!("action-ok"));
            f.secondary(t!("action-cancel"));
        },
    );
    if state.prefs != before {
        // The interface follows on the next frame (see `sync_look`).
        trim_recent(state);
        ctx.request_repaint();
    }
    match response.clicked {
        Some(i) if i == ok => trim_recent(state),
        Some(_) => restore(state, dialog.snapshot),
        None if response.dismissed => restore(state, dialog.snapshot),
        None => state.dialogs.preferences = Some(dialog),
    }
}

fn body(ui: &mut Ui, state: &mut AppState, dialog: &mut PreferencesDialog) {
    crate::dialog::two_columns(ui, [150.0, 601.0], |ui, column| {
        if column == 0 {
            for section in Section::ALL {
                let label = RichText::new(section.label()).size(13.0);
                let r = ui.add_sized([150.0, 28.0], egui::Button::selectable(dialog.section == section, label));
                if r.clicked() {
                    dialog.section = section;
                }
            }
            return;
        }
        // The same height on every page, so the window does not jump.
        ui.set_min_height(240.0);
        ui.label(theme::title(dialog.section.label()));
        ui.add_space(6.0);
        match dialog.section {
            Section::General => general(ui, state),
            Section::Interface => interface(ui, state),
            Section::FileLocations => file_locations(ui, state),
            Section::Saving => saving(ui, state),
        }
    });
}

/// A combo box in a table row choosing among `options`.
fn combo<T: PartialEq + Copy>(
    ui: &mut Ui,
    id: &str,
    value: &mut T,
    options: impl IntoIterator<Item = T>,
    label: impl Fn(T) -> String,
) {
    egui::ComboBox::from_id_salt(("pref", id)).selected_text(label(*value)).show_ui(ui, |ui| {
        for option in options {
            ui.selectable_value(value, option, label(option));
        }
    });
}

/// A checkbox in a table row (its label is the row's).
fn check(ui: &mut Ui, label: &str, value: &mut bool, hint: Option<String>) {
    ui.add_space(4.0);
    // Unlike the flat cells around it, the box shows when unchecked too.
    ui.visuals_mut().widgets.inactive.bg_stroke = egui::Stroke::new(1.0, PALETTE.border_strong);
    let r = ui.checkbox(value, "");
    r.widget_info(|| egui::WidgetInfo::selected(egui::WidgetType::Checkbox, true, *value, label));
    if let Some(hint) = hint {
        r.on_hover_text(hint);
    }
}

fn general(ui: &mut Ui, state: &mut AppState) {
    let label = t!("settings-language");
    table::row(ui, &label, |ui| {
        let mut lang = crate::i18n::language();
        combo(ui, "language", &mut lang, Language::ALL, |l| l.native_name().to_string());
        crate::actions::set_language(state, lang);
    });
    table::row(ui, &t!("settings-theme"), |ui| {
        combo(ui, "theme", &mut state.prefs.theme, ThemeChoice::ALL, ThemeChoice::label);
    });
    let label = t!("update-auto");
    table::row(ui, &label, |ui| check(ui, &label, &mut state.prefs.check_updates, Some(t!("update-auto-hint"))));
}

fn interface(ui: &mut Ui, state: &mut AppState) {
    table::row(ui, &t!("pref-ui-scale"), |ui| {
        let mut scale = clamp_scale(state.prefs.ui_scale);
        let options = UI_SCALES.into_iter().chain((!UI_SCALES.contains(&scale)).then_some(scale));
        combo(ui, "scale", &mut scale, options, |s| format!("{s}%"));
        state.prefs.ui_scale = scale;
    });
    table::row(ui, &t!("pref-font-size"), |ui| {
        let name = |p: u32| {
            FONT_SIZES.iter().find(|(v, _)| *v == p).map_or_else(|| format!("{p}%"), |(_, key)| crate::i18n::tr(key))
        };
        let current = state.prefs.font_size;
        let options = FONT_SIZES
            .map(|(v, _)| v)
            .into_iter()
            .chain((!FONT_SIZES.iter().any(|(v, _)| *v == current)).then_some(current));
        combo(ui, "font", &mut state.prefs.font_size, options, name);
    });
    let label = t!("pref-ribbon-collapsed");
    table::row(ui, &label, |ui| {
        let before = state.prefs.ribbon_collapsed;
        check(ui, &label, &mut state.prefs.ribbon_collapsed, None);
        if state.prefs.ribbon_collapsed != before {
            state.ribbon_collapsed = state.prefs.ribbon_collapsed;
        }
    });
    let label = t!("pref-detailed-tooltips");
    table::row(ui, &label, |ui| {
        check(ui, &label, &mut state.prefs.detailed_tooltips, Some(t!("pref-detailed-tooltips-hint")))
    });
    let label = t!("pref-select-drag-3d");
    table::row(ui, &label, |ui| {
        check(ui, &label, &mut state.prefs.select_drag_3d, Some(t!("pref-select-drag-3d-hint")))
    });
}

/// A folder in a table row: its path, Browse… and Clear.
fn folder_row(ui: &mut Ui, label: &str, value: &mut Option<PathBuf>) {
    table::row(ui, label, |ui| {
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            if ui.add_enabled(value.is_some(), egui::Button::new(t!("pref-clear"))).clicked() {
                *value = None;
            }
            if ui.button(t!("pref-browse")).clicked() {
                let mut picker = rfd::FileDialog::new();
                if let Some(dir) = value.as_deref().filter(|p| p.is_dir()) {
                    picker = picker.set_directory(dir);
                }
                if let Some(folder) = picker.pick_folder() {
                    *value = Some(folder);
                }
            }
            let (text, color) = match value.as_deref() {
                Some(p) if p.is_dir() => (p.display().to_string(), PALETTE.text),
                Some(p) => (t!("pref-folder-missing", "path" => p.display()), PALETTE.warning),
                None => (t!("pref-not-set"), PALETTE.text_faint),
            };
            ui.with_layout(Layout::left_to_right(Align::Center), |ui| {
                ui.add_space(4.0);
                let path = value.as_ref().map(|p| p.display().to_string()).unwrap_or_default();
                ui.add(egui::Label::new(RichText::new(text).color(color)).truncate()).on_hover_text(path);
            });
        });
    });
}

fn file_locations(ui: &mut Ui, state: &mut AppState) {
    folder_row(ui, &t!("pref-macro-folder"), &mut state.prefs.macro_folder);
    folder_row(ui, &t!("pref-projects-folder"), &mut state.prefs.projects_folder);
    ui.add_space(12.0);
    ui.label(theme::title(t!("lib-folders")));
    let folders = &mut state.prefs.library_folders;
    if folders.is_empty() {
        ui.label(theme::muted(t!("lib-empty")).size(12.0));
    }
    let mut remove = None;
    egui::ScrollArea::vertical().id_salt("pref-library-folders").max_height(180.0).show(ui, |ui| {
        for (i, folder) in folders.iter().enumerate() {
            let missing = !folder.is_dir();
            let text = folder.display().to_string();
            table::row(ui, &text, |ui| {
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    let r = ui.button(t!("action-remove"));
                    r.widget_info(|| {
                        let label = format!("{} {text}", t!("action-remove"));
                        egui::WidgetInfo::labeled(egui::WidgetType::Button, true, label)
                    });
                    if r.clicked() {
                        remove = Some(i);
                    }
                    if missing {
                        ui.label(RichText::new(t!("pref-missing")).color(PALETTE.warning));
                    }
                });
            });
        }
    });
    if let Some(i) = remove
        && i < folders.len()
    {
        folders.remove(i);
    }
    ui.add_space(6.0);
    if ui.button(t!("lib-add-folder")).clicked()
        && let Some(folder) = rfd::FileDialog::new().pick_folder()
        && !folders.contains(&folder)
        && folders.len() < MAX_LIBRARY_FOLDERS
    {
        folders.push(folder);
    }
}

fn saving(ui: &mut Ui, state: &mut AppState) {
    table::row(ui, &t!("pref-autosave"), |ui| {
        let current = state.prefs.autosave_minutes;
        let options = AUTOSAVE_MINUTES.into_iter().chain((!AUTOSAVE_MINUTES.contains(&current)).then_some(current));
        let label = |m: u32| if m == 0 { t!("pref-autosave-off") } else { t!("pref-autosave-every", "count" => m) };
        combo(ui, "autosave", &mut state.prefs.autosave_minutes, options, label);
    });
    table::row(ui, &t!("pref-recent-count"), |ui| {
        let current = state.prefs.recent_count;
        let options = RECENT_COUNTS.into_iter().chain((!RECENT_COUNTS.contains(&current)).then_some(current));
        combo(ui, "recent", &mut state.prefs.recent_count, options, |n| n.to_string());
    });
    ui.add_space(8.0);
    ui.label(theme::muted(t!("pref-autosave-hint")).size(12.0));
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn stored(prefs: &Preferences) -> HashMap<String, String> {
        let mut map = HashMap::new();
        prefs.write(|k, v| {
            map.insert(k.to_string(), v);
        });
        map
    }

    /// Nothing stored: today's behavior (system theme, update checks, a
    /// one-minute autosave, twelve recent projects, detailed tooltips).
    #[test]
    fn defaults_when_nothing_is_stored() {
        let prefs = Preferences::read(|_| None, 1.0);
        assert_eq!(prefs, Preferences::default());
        assert_eq!(prefs.autosave_interval(), Some(Duration::from_secs(60)));
        assert_eq!(prefs.recent_count, 12);
        // A zoom eframe kept from Ctrl+Plus becomes the scale.
        assert_eq!(Preferences::read(|_| None, 1.25).ui_scale, 125);
    }

    #[test]
    fn preferences_round_trip() {
        let prefs = Preferences {
            theme: ThemeChoice::Dark,
            check_updates: false,
            ui_scale: 150,
            font_size: 115,
            ribbon_collapsed: true,
            detailed_tooltips: false,
            select_drag_3d: true,
            library_folders: vec![PathBuf::from("/lib/a"), PathBuf::from("C:\\lib b")],
            macro_folder: Some(PathBuf::from("/macros")),
            projects_folder: Some(PathBuf::from("/projects")),
            autosave_minutes: 0,
            recent_count: 20,
        };
        let map = stored(&prefs);
        let back = Preferences::read(|k| map.get(k).cloned(), 1.0);
        assert_eq!(back, prefs);
        assert_eq!(back.autosave_interval(), None);
        // Cleared folders are stored empty and read back as unset.
        let map = stored(&Preferences::default());
        assert_eq!(Preferences::read(|k| map.get(k).cloned(), 1.0), Preferences::default());
    }

    /// Stored values are hostile input: they are clamped or ignored.
    #[test]
    fn invalid_values_are_clamped_or_ignored() {
        let lines: String = (0..500).map(|i| format!("/f{}\n\n/f{}\r\n", i % 100, i % 100)).collect();
        let map: HashMap<&str, String> = [
            (UI_SCALE_KEY, "100000".to_string()),
            (FONT_SIZE_KEY, "1".to_string()),
            (AUTOSAVE_KEY, "-3".to_string()),
            (RECENT_COUNT_KEY, "0".to_string()),
            (THEME_KEY, "sepia".to_string()),
            (LIBRARY_KEY, lines),
            (MACRO_FOLDER_KEY, "  ".to_string()),
            (UPDATE_AUTO_KEY, "maybe".to_string()),
        ]
        .into();
        let prefs = Preferences::read(|k| map.get(k).cloned(), f32::NAN);
        assert_eq!(prefs.ui_scale, 200);
        assert_eq!(prefs.font_size, 80);
        assert_eq!(prefs.autosave_minutes, 1, "unparsable: the default");
        assert_eq!(prefs.recent_count, 1);
        assert_eq!(prefs.theme, ThemeChoice::System);
        assert_eq!(prefs.library_folders.len(), MAX_LIBRARY_FOLDERS, "bounded and deduplicated");
        assert_eq!(prefs.library_folders[0], PathBuf::from("/f0"));
        assert_eq!(prefs.macro_folder, None);
        assert!(prefs.check_updates, "only \"0\" turns it off");
        assert_eq!(Preferences::read(|_| None, f32::INFINITY).ui_scale, 100);
        assert_eq!(Preferences::read(|_| None, 9.0).ui_scale, 200);
        // A folder that does not exist is kept but not used.
        let gone = Preferences { macro_folder: Some(PathBuf::from("/no/such/folder/ccad")), ..Default::default() };
        assert_eq!(gone.macro_dir(), None);
        assert!(macro_files(Path::new("/no/such/folder/ccad")).is_empty());
    }

    #[test]
    fn macro_files_are_listed_sorted_and_bounded() {
        let dir = std::env::temp_dir().join(format!("ccad-macros-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("nested.ccmacro")).unwrap();
        for name in ["b.ccmacro", "A.CCMACRO", "notes.txt", "c.ccmacro"] {
            std::fs::write(dir.join(name), "").unwrap();
        }
        let names: Vec<String> =
            macro_files(&dir).iter().map(|p| p.file_name().unwrap().to_string_lossy().into_owned()).collect();
        assert_eq!(names, ["A.CCMACRO", "b.ccmacro", "c.ccmacro"]);
        for i in 0..MAX_MACROS + 5 {
            std::fs::write(dir.join(format!("m{i:03}.ccmacro")), "").unwrap();
        }
        assert_eq!(macro_files(&dir).len(), MAX_MACROS);
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Startup reads the stored recent projects up to the preferred count, and saving writes them back.
    #[test]
    fn load_and_save_restore_the_app_state() {
        let recent: String = (0..30).map(|i| format!("p{i}.ccad\n")).collect();
        let map: HashMap<&str, String> = [
            (RECENT_KEY, recent),
            (RECENT_COUNT_KEY, "5".to_string()),
            (RIBBON_KEY, "1".to_string()),
            (UPDATE_LAST_KEY, "1234".to_string()),
            // The current language: other tests running at the same time read it.
            (LANGUAGE_KEY, crate::i18n::language().code().to_string()),
        ]
        .into();
        let mut state = AppState::new(cc_core::Document::new(cc_core::Project::from_template()));
        load_language(|k| map.get(k).cloned(), None);
        load(&mut state, |k| map.get(k).cloned(), 1.0);
        assert_eq!(state.recent.len(), 5);
        assert!(state.ribbon_collapsed, "the ribbon starts collapsed");
        assert_eq!(state.updates.last_check, Some(1234));
        let language = crate::i18n::language();
        let mut out = HashMap::new();
        save(&state, |k, v| {
            out.insert(k.to_string(), v);
        });
        assert_eq!(out[RECENT_KEY].lines().count(), 5);
        assert_eq!(out[LANGUAGE_KEY], language.code());
        assert_eq!(out[RIBBON_KEY], "1");
    }
}
