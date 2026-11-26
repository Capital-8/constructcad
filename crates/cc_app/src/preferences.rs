//! App preferences (not model data): what they hold, how they are kept in
//! eframe's storage (every key lives here), how the interface follows them,
//! and the Preferences window (File tab, Ctrl+,, home screen).
//!
//! Stored values are hostile input: numbers are parsed and clamped, lists
//! are bounded, and folders that no longer exist are kept but skipped where
//! they are used. Missing values take defaults that match the behavior
//! before the window existed.

use crate::commands::UiCommand;
use crate::dialog::Dialog;
use crate::i18n::Language;
use crate::icons::{self, Icon};
use crate::settings::{self, Pane};
use crate::state::AppState;
use crate::t;
use crate::table;
use crate::theme::{self, PALETTE, ThemeChoice};
use egui::{Align, Context, Layout, RichText, Ui};
use std::collections::{BTreeMap, BTreeSet};
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
/// eframe storage key of the trackpad preference ("0" when two-finger scrolling zooms).
const TRACKPAD_KEY: &str = "trackpad_pans";
/// eframe storage key of the custom two-letter shortcuts (`CommandId=KEYS` per line).
const SHORTCUTS_KEY: &str = "shortcut_keys";
/// eframe storage key of the CAD blocks preference ("1" when blocks become detail components).
const CAD_BLOCKS_KEY: &str = "cad_blocks";
/// eframe storage key of the object snaps (`snaps::SnapSettings::to_text`).
const SNAPS_KEY: &str = "object_snaps";
/// eframe storage key of the macros folder.
const MACRO_FOLDER_KEY: &str = "macro_folder";
/// eframe storage key of the default projects folder.
const PROJECTS_FOLDER_KEY: &str = "projects_folder";
/// eframe storage key of the autosave interval (minutes, 0 = off).
const AUTOSAVE_KEY: &str = "autosave_minutes";
/// eframe storage key of the number of recent projects kept.
const RECENT_COUNT_KEY: &str = "recent_count";
/// eframe storage key of the plugins folder (empty: the default one).
const PLUGIN_FOLDER_KEY: &str = "plugin_folder";
/// eframe storage key of the plugins turned off (one id per line).
const PLUGINS_OFF_KEY: &str = "plugins_off";
/// eframe storage key of the plugins allowed to work with files (one id per line).
const PLUGINS_FILES_KEY: &str = "plugins_files";
/// eframe storage key of the plugins allowed to run external programs (one id per line).
const PLUGINS_TOOLS_KEY: &str = "plugins_tools";
/// eframe storage key of the plugins allowed to access the network (one id per line).
const PLUGINS_NETWORK_KEY: &str = "plugins_network";
/// eframe storage key of plugin developer mode ("1": on).
const PLUGIN_DEV_KEY: &str = "plugin_dev_mode";
/// eframe storage key of the plugin development folder (empty: none).
const PLUGIN_DEV_FOLDER_KEY: &str = "plugin_dev_folder";
/// eframe storage key of where the side panels are docked (`side_panels::PanelLayout::to_text`).
const PANELS_KEY: &str = "side_panels";
/// eframe storage key of the Structure tab's material (`StructuralMaterial::code`).
const STRUCTURE_MATERIAL_KEY: &str = "structure_material";
/// eframe storage key of the quick access toolbar's commands (one `UiCommand::id` per
/// line; `default` or no value: the default set).
const QUICK_ACCESS_KEY: &str = "quick_access";
/// The most commands kept in the quick access toolbar.
const MAX_QUICK_ACCESS: usize = 40;

/// eframe storage key of the template last picked for a new project (`MeasurementSystem::code`).
const NEW_PROJECT_KEY: &str = "new_project_system";
/// Most plugin ids kept in each of those lists.
const MAX_PLUGIN_IDS: usize = 1000;

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
/// At most this many custom shortcuts are read.
const MAX_SHORTCUTS: usize = 500;

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
    /// Two fingers on a trackpad pan the views and a pinch zooms; otherwise
    /// any scroll zooms, as a mouse wheel does (see `gestures.rs`).
    pub trackpad_pans: bool,
    /// Import CAD brings a drawing's blocks in as detail components instead
    /// of exploding them into the imported drawing.
    pub cad_blocks: bool,
    /// Which points the tools snap to.
    pub snaps: crate::snaps::SnapSettings,
    /// Custom two-letter keys by command id (`UiCommand::id`); empty keys
    /// take a command's keys away.
    pub shortcut_keys: BTreeMap<String, String>,
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
    /// Where plugins are installed and loaded from (`None`: the default
    /// folder, see [`Preferences::plugins_dir`]).
    pub plugin_folder: Option<PathBuf>,
    /// Plugins turned off (by id): not loaded.
    pub plugins_off: BTreeSet<String>,
    /// Plugins the user allowed to work with files (the `files` permission).
    pub plugins_files: BTreeSet<String>,
    /// Plugins the user allowed to run external programs (the `tools`
    /// permission).
    pub plugins_tools: BTreeSet<String>,
    /// Plugins the user allowed to access HTTPS hosts in `allowed_hosts`
    /// (the `network` permission).
    pub plugins_network: BTreeSet<String>,
    /// Plugin developer mode: the plugins load again when a file of the
    /// plugins folder or the development folder changes, and the
    /// development folder's plugins are loaded (see [`crate::plugin_dev`]).
    pub plugin_dev_mode: bool,
    /// The folder of plugins being developed (a plugin's folder or a folder
    /// of plugins), loaded in developer mode.
    pub plugin_dev_folder: Option<PathBuf>,
    /// Where the Project Browser and Properties are docked.
    pub panels: crate::side_panels::PanelLayout,
    /// The material the Structure tab's column and beam tools place.
    pub structure_material: cc_core::StructuralMaterial,
    /// The commands the user put in the quick access toolbar after Undo and
    /// Redo, by `UiCommand::id`, in order; `None`: the default set.
    pub quick_access: Option<Vec<String>>,

    /// The template last picked in the New Project dialog, picked first next time.
    pub new_project_system: cc_core::MeasurementSystem,
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
            trackpad_pans: true,
            cad_blocks: false,
            snaps: Default::default(),
            shortcut_keys: BTreeMap::new(),
            library_folders: Vec::new(),
            macro_folder: None,
            projects_folder: None,
            autosave_minutes: 1,
            recent_count: 12,
            plugin_folder: None,
            plugins_off: BTreeSet::new(),
            plugins_files: BTreeSet::new(),
            plugins_tools: BTreeSet::new(),
            plugins_network: BTreeSet::new(),
            plugin_dev_mode: false,
            plugin_dev_folder: None,
            panels: Default::default(),
            structure_material: Default::default(),
            quick_access: None,

            new_project_system: cc_core::MeasurementSystem::Metric,
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

/// Custom shortcuts stored as `CommandId=KEYS` lines: only commands that
/// exist and may change keys, with two capital letters or none.
fn shortcut_lines(text: &str) -> BTreeMap<String, String> {
    text.lines()
        .take(MAX_SHORTCUTS)
        .filter_map(|line| {
            let (id, keys) = line.trim_end_matches('\r').split_once('=')?;
            let command = crate::commands::UiCommand::from_id(id.trim())?;
            let keys = keys.trim().to_ascii_uppercase();
            let valid = keys.is_empty() || crate::commands::is_letter_keys(&keys);
            (valid && command.customizable()).then(|| (command.id(), keys))
        })
        .collect()
}

/// Plugin ids, one per line (invalid ones dropped, bounded).
fn id_lines(text: &str) -> BTreeSet<String> {
    text.lines()
        .map(str::trim)
        .filter(|id| cc_plugins::plugin::valid_id(id))
        .take(MAX_PLUGIN_IDS)
        .map(str::to_string)
        .collect()
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
            trackpad_pans: on(TRACKPAD_KEY, d.trackpad_pans),
            cad_blocks: on(CAD_BLOCKS_KEY, d.cad_blocks),
            snaps: get(SNAPS_KEY).map_or(d.snaps, |v| crate::snaps::SnapSettings::from_text(&v)),
            shortcut_keys: get(SHORTCUTS_KEY).map(|v| shortcut_lines(&v)).unwrap_or_default(),
            library_folders: get(LIBRARY_KEY).map(|v| path_lines(&v, MAX_LIBRARY_FOLDERS)).unwrap_or_default(),
            macro_folder: folder(get(MACRO_FOLDER_KEY)),
            projects_folder: folder(get(PROJECTS_FOLDER_KEY)),
            autosave_minutes: number(AUTOSAVE_KEY).map_or(d.autosave_minutes, |v| v.min(60) as u32),
            recent_count: number(RECENT_COUNT_KEY).map_or(d.recent_count, |v| v.clamp(1, MAX_RECENT as u64) as usize),
            plugin_folder: folder(get(PLUGIN_FOLDER_KEY)),
            plugins_off: get(PLUGINS_OFF_KEY).map(|v| id_lines(&v)).unwrap_or_default(),
            plugins_files: get(PLUGINS_FILES_KEY).map(|v| id_lines(&v)).unwrap_or_default(),
            plugins_tools: get(PLUGINS_TOOLS_KEY).map(|v| id_lines(&v)).unwrap_or_default(),
            plugins_network: get(PLUGINS_NETWORK_KEY).map(|v| id_lines(&v)).unwrap_or_default(),
            plugin_dev_mode: on(PLUGIN_DEV_KEY, d.plugin_dev_mode),
            plugin_dev_folder: folder(get(PLUGIN_DEV_FOLDER_KEY)),
            panels: get(PANELS_KEY).and_then(|v| crate::side_panels::PanelLayout::from_text(&v)).unwrap_or_default(),
            structure_material: get(STRUCTURE_MATERIAL_KEY)
                .and_then(|v| cc_core::StructuralMaterial::from_code(&v))
                .unwrap_or(d.structure_material),
            quick_access: get(QUICK_ACCESS_KEY).filter(|v| v.trim() != "default").map(|v| {
                v.lines()
                    .map(str::trim)
                    .filter(|id| !id.is_empty() && id.len() <= 64)
                    .take(MAX_QUICK_ACCESS)
                    .map(str::to_string)
                    .collect()
            }),

            new_project_system: get(NEW_PROJECT_KEY)
                .and_then(|v| cc_core::MeasurementSystem::from_code(&v))
                .unwrap_or(d.new_project_system),
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
        set(TRACKPAD_KEY, flag(self.trackpad_pans));
        set(CAD_BLOCKS_KEY, flag(self.cad_blocks));
        set(SNAPS_KEY, self.snaps.to_text());
        let keys = self.shortcut_keys.iter().map(|(id, keys)| format!("{id}={keys}")).collect::<Vec<_>>();
        set(SHORTCUTS_KEY, keys.join("\n"));
        set(LIBRARY_KEY, join_paths(&self.library_folders));
        let text = |p: &Option<PathBuf>| p.as_ref().map(|p| p.to_string_lossy().into_owned()).unwrap_or_default();
        set(MACRO_FOLDER_KEY, text(&self.macro_folder));
        set(PROJECTS_FOLDER_KEY, text(&self.projects_folder));
        set(AUTOSAVE_KEY, self.autosave_minutes.to_string());
        set(RECENT_COUNT_KEY, self.recent_count.to_string());
        set(PLUGIN_FOLDER_KEY, text(&self.plugin_folder));
        let ids = |set: &BTreeSet<String>| set.iter().cloned().collect::<Vec<_>>().join("\n");
        set(PLUGINS_OFF_KEY, ids(&self.plugins_off));
        set(PLUGINS_FILES_KEY, ids(&self.plugins_files));
        set(PLUGINS_TOOLS_KEY, ids(&self.plugins_tools));
        set(PLUGINS_NETWORK_KEY, ids(&self.plugins_network));
        set(PLUGIN_DEV_KEY, flag(self.plugin_dev_mode));
        set(PLUGIN_DEV_FOLDER_KEY, text(&self.plugin_dev_folder));
        set(PANELS_KEY, self.panels.to_text());
        set(STRUCTURE_MATERIAL_KEY, self.structure_material.code().to_string());
        let quick = self.quick_access.as_ref().map_or_else(
            || "default".to_string(),
            |ids| {
                ids.join(
                    "
",
                )
            },
        );
        set(QUICK_ACCESS_KEY, quick);

        set(NEW_PROJECT_KEY, self.new_project_system.code().to_string());
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

    /// The plugins folder: the one set, else `plugins` in the app's data
    /// folder (`None` when there is none).
    pub fn plugins_dir(&self) -> Option<PathBuf> {
        // Tests never load the user's own plugins.
        let default = if cfg!(test) { None } else { crate::recovery::data_dir().map(|d| d.join("plugins")) };
        self.plugin_folder.clone().or(default)
    }

    /// The plugin development folder, in developer mode (`None` when off or
    /// not set).
    pub fn plugin_dev_dir(&self) -> Option<PathBuf> {
        self.plugin_dev_folder.clone().filter(|_| self.plugin_dev_mode)
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
    crate::commands::set_key_overrides(&prefs.shortcut_keys);
    crate::gestures::set_trackpad_pans(prefs.trackpad_pans);
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
    Drawing,
    FileLocations,
    Saving,
    Shortcuts,
}

impl Section {
    pub const ALL: [Section; 6] = [
        Section::General,
        Section::Interface,
        Section::Drawing,
        Section::FileLocations,
        Section::Saving,
        Section::Shortcuts,
    ];

    pub fn label(self) -> String {
        match self {
            Section::General => t!("pref-general"),
            Section::Interface => t!("pref-interface"),
            Section::Drawing => t!("pref-drawing"),
            Section::FileLocations => t!("pref-file-locations"),
            Section::Saving => t!("pref-saving"),
            Section::Shortcuts => t!("pref-shortcuts"),
        }
    }

    /// What the page holds, under its title.
    fn description(self) -> String {
        match self {
            Section::General => t!("pref-general-desc"),
            Section::Interface => t!("pref-interface-desc"),
            Section::Drawing => t!("pref-drawing-desc"),
            Section::FileLocations => t!("pref-file-locations-desc"),
            Section::Saving => t!("pref-saving-desc"),
            Section::Shortcuts => t!("pref-shortcuts-hint"),
        }
    }

    fn icon(self) -> Icon {
        match self {
            Section::General => Icon::Settings,
            Section::Interface => Icon::Interface,
            Section::Drawing => Icon::Select,
            Section::FileLocations => Icon::Open,
            Section::Saving => Icon::Save,
            Section::Shortcuts => Icon::Keyboard,
        }
    }
}

/// Width of the window's body (sidebar and page).
const WIDTH: f32 = 880.0;
/// Height of the body: the same on every page, so the window does not
/// jump; longer pages scroll.
const HEIGHT: f32 = 500.0;
/// Width of the combo boxes.
const COMBO_WIDTH: f32 = 190.0;

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
    shortcuts: ShortcutEditor,
}

/// The Shortcuts page: its search text and the keys being typed for a command.
#[derive(Debug, Clone, Default)]
struct ShortcutEditor {
    filter: String,
    /// The search field took the focus when the page first showed.
    searched: bool,
    /// The command whose keys are being typed, the letters so far, and why
    /// they were refused (keys of another command).
    editing: Option<(UiCommand, String, Option<String>)>,
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
    state.dialogs.preferences = Some(PreferencesDialog { section, snapshot, shortcuts: ShortcutEditor::default() });
}

fn restore(state: &mut AppState, snapshot: Snapshot) {
    state.prefs = snapshot.prefs;
    state.ribbon_collapsed = snapshot.ribbon_collapsed;
    crate::actions::set_language(state, snapshot.language);
}

/// Restore Defaults: the default values of the page's preferences (the
/// language, the panel layout and the folders are left as they are).
fn restore_page(state: &mut AppState, section: Section) {
    let d = Preferences::default();
    let p = &mut state.prefs;
    match section {
        Section::General => {
            p.check_updates = d.check_updates;
            p.cad_blocks = d.cad_blocks;
        }
        Section::Interface => {
            p.theme = d.theme;
            p.ui_scale = d.ui_scale;
            p.font_size = d.font_size;
            p.ribbon_collapsed = d.ribbon_collapsed;
            p.detailed_tooltips = d.detailed_tooltips;
            state.ribbon_collapsed = d.ribbon_collapsed;
        }
        Section::Drawing => {
            p.snaps = d.snaps;
            p.select_drag_3d = d.select_drag_3d;
            p.trackpad_pans = d.trackpad_pans;
        }
        Section::FileLocations => {}
        Section::Saving => {
            p.autosave_minutes = d.autosave_minutes;
            p.recent_count = d.recent_count;
        }
        Section::Shortcuts => p.shortcut_keys = d.shortcut_keys,
    }
}

pub fn show(ctx: &Context, state: &mut AppState) {
    let Some(mut dialog) = state.dialogs.preferences.take() else { return };
    let before = state.prefs.clone();
    let (mut ok, mut defaults) = (0, None);
    let section = dialog.section;
    let response = Dialog::new("preferences", t!("dlg-preferences")).width(WIDTH).flush().show(
        ctx,
        |ui| body(ui, state, &mut dialog),
        |f| {
            if section != Section::FileLocations {
                defaults = Some(f.extra(t!("pref-restore-defaults")));
            }
            ok = f.primary(t!("action-ok"));
            f.secondary(t!("action-cancel"));
        },
    );
    if response.clicked.is_some() && response.clicked == defaults {
        restore_page(state, section);
        dialog.shortcuts.editing = None;
    }
    if state.prefs != before {
        // The interface follows on the next frame (see `sync_look`).
        trim_recent(state);
        crate::commands::set_key_overrides(&state.prefs.shortcut_keys);
        ctx.request_repaint();
    }
    match response.clicked {
        Some(i) if i == ok => trim_recent(state),
        Some(i) if Some(i) == defaults => state.dialogs.preferences = Some(dialog),
        Some(_) => restore(state, dialog.snapshot),
        None if response.dismissed => restore(state, dialog.snapshot),
        None => state.dialogs.preferences = Some(dialog),
    }
}

fn body(ui: &mut Ui, state: &mut AppState, dialog: &mut PreferencesDialog) {
    settings::sidebar_layout(ui, &t!("dlg-preferences"), HEIGHT, |ui, pane| {
        if pane == Pane::Sidebar {
            for section in Section::ALL {
                if settings::nav_item(ui, section.icon(), &section.label(), dialog.section == section).clicked() {
                    dialog.section = section;
                }
            }
            return;
        }
        let section = dialog.section;
        let scroll = section != Section::Shortcuts;
        settings::page(ui, section as u8, &section.label(), &section.description(), scroll, |ui| match section {
            Section::General => general(ui, state),
            Section::Interface => interface(ui, state),
            Section::Drawing => drawing(ui, state),
            Section::FileLocations => file_locations(ui, state),
            Section::Saving => saving(ui, state),
            // The list scrolls under its search field.
            Section::Shortcuts => shortcuts(ui, state, &mut dialog.shortcuts),
        });
    });
}

fn general(ui: &mut Ui, state: &mut AppState) {
    ui.add_space(8.0);
    settings::setting(ui, &t!("settings-language"), Some(&t!("pref-language-hint")), |ui| {
        let mut lang = crate::i18n::language();
        settings::combo(ui, "language", COMBO_WIDTH, &mut lang, Language::ALL, |l| l.native_name().to_string());
        crate::actions::set_language(state, lang);
    });
    settings::group(ui, &t!("pref-group-updates"));
    settings::switch_setting(ui, &t!("update-auto"), Some(&t!("update-auto-hint")), &mut state.prefs.check_updates);
    settings::group(ui, &t!("group-import"));
    settings::switch_setting(ui, &t!("label-cad-blocks"), Some(&t!("hint-cad-blocks")), &mut state.prefs.cad_blocks);
}

fn interface(ui: &mut Ui, state: &mut AppState) {
    settings::group(ui, &t!("settings-theme"));
    ui.add_space(6.0);
    ui.label(RichText::new(t!("pref-theme-hint")).size(12.0 * theme::font_scale()).color(PALETTE.text_muted));
    ui.add_space(12.0);
    settings::theme_cards(ui, &mut state.prefs.theme);
    ui.add_space(6.0);

    settings::group(ui, &t!("pref-group-size"));
    settings::setting(ui, &t!("pref-ui-scale"), Some(&t!("pref-ui-scale-hint")), |ui| {
        let mut scale = clamp_scale(state.prefs.ui_scale);
        let options = UI_SCALES.into_iter().chain((!UI_SCALES.contains(&scale)).then_some(scale));
        settings::combo(ui, "scale", COMBO_WIDTH, &mut scale, options, |s| format!("{s}%"));
        state.prefs.ui_scale = scale;
    });
    settings::setting(ui, &t!("pref-font-size"), Some(&t!("pref-font-size-hint")), |ui| {
        let current = state.prefs.font_size;
        let mut options: Vec<settings::Segment<u32>> = FONT_SIZES
            .iter()
            .enumerate()
            .map(|(i, (value, key))| settings::Segment {
                value: *value,
                text: "Aa".to_string(),
                text_size: 11.0 + 2.0 * i as f32,
                name: crate::i18n::tr(key),
            })
            .collect();
        if !FONT_SIZES.iter().any(|(v, _)| *v == current) {
            let text = format!("{current}%");
            options.push(settings::Segment { value: current, text: text.clone(), text_size: 12.0, name: text });
        }
        settings::segmented(ui, &mut state.prefs.font_size, options);
    });

    settings::group(ui, &t!("pref-group-ribbon"));
    let before = state.prefs.ribbon_collapsed;
    let label = t!("pref-ribbon-collapsed");
    settings::switch_setting(ui, &label, Some(&t!("pref-ribbon-collapsed-hint")), &mut state.prefs.ribbon_collapsed);
    if state.prefs.ribbon_collapsed != before {
        state.ribbon_collapsed = state.prefs.ribbon_collapsed;
    }
    let label = t!("pref-detailed-tooltips");
    let hint = t!("pref-detailed-tooltips-hint");
    settings::switch_setting(ui, &label, Some(&hint), &mut state.prefs.detailed_tooltips);

    settings::group(ui, &t!("pref-group-panels"));
    settings::setting(ui, &t!("pref-panel-layout"), Some(&t!("panel-drag-hint")), |ui| {
        let default = crate::side_panels::PanelLayout::default();
        let changed = state.prefs.panels != default;
        if settings::button(ui, changed, &t!("panel-reset-layout")).clicked() {
            state.prefs.panels = default;
        }
    });
}

/// Object snaps (issue #317) and how the views move.
fn drawing(ui: &mut Ui, state: &mut AppState) {
    let snaps = &mut state.prefs.snaps;
    settings::group(ui, &t!("pref-group-snaps"));
    settings::switch_setting(ui, &t!("pref-snap-enabled"), Some(&t!("pref-snap-enabled-hint")), &mut snaps.enabled);
    ui.add_enabled_ui(snaps.enabled, |ui| {
        let label = t!("pref-snap-elements");
        settings::switch_setting(ui, &label, Some(&t!("pref-snap-elements-hint")), &mut snaps.elements);
        settings::setting_row(ui, &t!("pref-snap-kinds"), Some(&t!("pref-snap-kinds-hint")), |_| ());
        // The kinds of point in two columns.
        let kinds = crate::snaps::SnapOption::KINDS;
        let half = kinds.len().div_ceil(2);
        let width = ui.available_width();
        ui.horizontal_top(|ui| {
            ui.spacing_mut().item_spacing = egui::vec2(0.0, 8.0);
            for column in [&kinds[..half], &kinds[half..]] {
                ui.allocate_ui_with_layout(egui::vec2(width / 2.0, 0.0), Layout::top_down(Align::Min), |ui| {
                    ui.set_width(width / 2.0);
                    for option in column {
                        let label = crate::i18n::tr(option.key());
                        let value = snaps.get_mut(*option);
                        let r =
                            table::labeled_checkbox(ui, value, RichText::new(&label).size(13.0 * theme::font_scale()));
                        let checked = *value;
                        r.widget_info(|| egui::WidgetInfo::selected(egui::WidgetType::Checkbox, true, checked, &label));
                    }
                });
            }
        });
        ui.add_space(14.0);
        settings::divider(ui);
    });

    settings::group(ui, &t!("pref-group-navigation"));
    let label = t!("pref-select-drag-3d");
    settings::switch_setting(ui, &label, Some(&t!("pref-select-drag-3d-hint")), &mut state.prefs.select_drag_3d);
    let label = t!("pref-trackpad-pans");
    settings::switch_setting(ui, &label, Some(&t!("pref-trackpad-pans-hint")), &mut state.prefs.trackpad_pans);
}

/// Pick a folder, starting in `current` when it exists.
fn pick_folder(current: Option<&Path>) -> Option<PathBuf> {
    let mut picker = rfd::FileDialog::new();
    if let Some(dir) = current.filter(|p| p.is_dir()) {
        picker = picker.set_directory(dir);
    }
    picker.pick_folder()
}

/// A folder setting: its name, what it is for, Browse… and Clear, and the
/// folder under them.
fn folder_setting(ui: &mut Ui, label: &str, hint: Option<&str>, value: &mut Option<PathBuf>) {
    settings::setting_row(ui, label, hint, |ui| {
        if settings::button(ui, value.is_some(), &t!("pref-clear")).clicked() {
            *value = None;
        }
        if settings::button(ui, true, &t!("pref-browse")).clicked()
            && let Some(folder) = pick_folder(value.as_deref())
        {
            *value = Some(folder);
        }
    });
    settings::path_box(ui, value.as_deref(), &t!("pref-not-set"), &t!("pref-missing"));
    ui.add_space(12.0);
    settings::divider(ui);
}

fn file_locations(ui: &mut Ui, state: &mut AppState) {
    let prefs = &mut state.prefs;
    settings::group(ui, &t!("pref-group-folders"));
    folder_setting(ui, &t!("pref-projects-folder"), Some(&t!("pref-folders-hint")), &mut prefs.projects_folder);
    folder_setting(ui, &t!("pref-macro-folder"), None, &mut prefs.macro_folder);

    settings::group(ui, &t!("lib-folders"));
    let folders = &mut prefs.library_folders;
    if folders.is_empty() {
        settings::note(ui, &t!("lib-empty"));
    }
    let mut remove = None;
    for (i, folder) in folders.iter().enumerate() {
        ui.add_space(8.0);
        ui.allocate_ui_with_layout(
            egui::vec2(ui.available_width(), 30.0),
            Layout::right_to_left(Align::Center),
            |ui| {
                let remove_label = t!("action-remove");
                let r = settings::button(ui, true, &remove_label);
                r.widget_info(|| {
                    let label = format!("{remove_label} {}", folder.display());
                    egui::WidgetInfo::labeled(egui::WidgetType::Button, true, label)
                });
                if r.clicked() {
                    remove = Some(i);
                }
                settings::path_box(ui, Some(folder), "", &t!("pref-missing"));
            },
        );
    }
    if let Some(i) = remove
        && i < folders.len()
    {
        folders.remove(i);
    }
    ui.add_space(10.0);
    let add = format!("+ {}", t!("lib-add-folder"));
    if settings::button(ui, folders.len() < MAX_LIBRARY_FOLDERS, &add).clicked()
        && let Some(folder) = rfd::FileDialog::new().pick_folder()
        && !folders.contains(&folder)
    {
        folders.push(folder);
    }

    settings::group(ui, &t!("pref-group-plugins"));
    let default = crate::recovery::data_dir().map(|d| d.join("plugins").display().to_string()).unwrap_or_default();
    let hint = t!("pref-plugin-folder-hint", "path" => default);
    folder_setting(ui, &t!("pref-plugin-folder"), Some(&hint), &mut prefs.plugin_folder);
    let label = t!("pref-plugin-dev");
    settings::switch_setting(ui, &label, Some(&t!("pref-plugin-dev-hint")), &mut prefs.plugin_dev_mode);
    let hint = t!("pref-plugin-dev-folder-hint");
    folder_setting(ui, &t!("pref-plugin-dev-folder"), Some(&hint), &mut prefs.plugin_dev_folder);
}

fn saving(ui: &mut Ui, state: &mut AppState) {
    settings::group(ui, &t!("pref-group-recovery"));
    settings::setting(ui, &t!("pref-autosave"), Some(&t!("pref-autosave-hint")), |ui| {
        let current = state.prefs.autosave_minutes;
        let options = AUTOSAVE_MINUTES.into_iter().chain((!AUTOSAVE_MINUTES.contains(&current)).then_some(current));
        let label = |m: u32| if m == 0 { t!("pref-autosave-off") } else { t!("pref-autosave-every", "count" => m) };
        settings::combo(ui, "autosave", COMBO_WIDTH, &mut state.prefs.autosave_minutes, options, label);
    });
    settings::group(ui, &t!("pref-group-home"));
    settings::setting(ui, &t!("pref-recent-count"), Some(&t!("pref-recent-count-hint")), |ui| {
        let current = state.prefs.recent_count;
        let options = RECENT_COUNTS.into_iter().chain((!RECENT_COUNTS.contains(&current)).then_some(current));
        settings::combo(ui, "recent", COMBO_WIDTH, &mut state.prefs.recent_count, options, |n| n.to_string());
    });
}

/// The keys `command` has with the custom ones in `custom`.
fn keys_of(custom: &BTreeMap<String, String>, command: UiCommand) -> Option<String> {
    let default = command.default_shortcut();
    if !command.customizable() {
        return default.map(str::to_string);
    }
    match custom.get(&command.id()) {
        Some(keys) if keys.is_empty() => None,
        Some(keys) => Some(keys.clone()),
        None => default.map(str::to_string),
    }
}

/// Give `command` the two-letter `keys` (empty: none), or tell which command
/// has them already.
fn set_keys(custom: &mut BTreeMap<String, String>, command: UiCommand, keys: &str) -> Result<(), UiCommand> {
    if !keys.is_empty()
        && let Some(other) =
            UiCommand::all().into_iter().find(|c| *c != command && keys_of(custom, *c).as_deref() == Some(keys))
    {
        return Err(other);
    }
    if command.default_shortcut().unwrap_or_default() == keys {
        custom.remove(&command.id());
    } else {
        custom.insert(command.id(), keys.to_string());
    }
    Ok(())
}

/// The Shortcuts page: every command with its keys; two-letter keys are
/// typed in place, Ctrl chords are fixed.
fn shortcuts(ui: &mut Ui, state: &mut AppState, editor: &mut ShortcutEditor) {
    // The search field, framed, with its icon inside.
    let (rect, _) = ui.allocate_exact_size(egui::vec2(ui.available_width(), 32.0), egui::Sense::hover());
    let field_ui =
        &mut ui.new_child(egui::UiBuilder::new().max_rect(rect).layout(Layout::left_to_right(Align::Center)));
    settings::framed(field_ui);
    field_ui.visuals_mut().widgets.inactive.bg_stroke = egui::Stroke::new(1.0, PALETTE.border_strong);
    let search = egui::TextEdit::singleline(&mut editor.filter)
        .hint_text(t!("pref-shortcuts-search"))
        .margin(egui::Margin { left: 30, right: 8, top: 7, bottom: 7 })
        .desired_width(rect.width());
    let r = field_ui.add(search);
    let icon = egui::Rect::from_center_size(egui::pos2(rect.left() + 16.0, rect.center().y), egui::vec2(15.0, 15.0));
    icons::paint(ui.painter(), icon, Icon::Search, PALETTE.text_muted);
    r.widget_info(|| egui::WidgetInfo::text_edit(true, "", &editor.filter, t!("pref-shortcuts-search")));
    if !editor.searched {
        editor.searched = true;
        r.request_focus();
    }
    ui.add_space(8.0);
    let filter = editor.filter.trim().to_lowercase();
    let custom = state.prefs.shortcut_keys.clone();
    let commands: Vec<(UiCommand, String)> = UiCommand::all()
        .into_iter()
        .filter(|c| {
            filter.is_empty()
                || c.label(state).to_lowercase().contains(&filter)
                || keys_of(&custom, *c).is_some_and(|k| k.to_lowercase().contains(&filter))
        })
        .map(|c| (c, c.label(state)))
        .collect();
    let mut change: Option<(UiCommand, String)> = None;
    egui::ScrollArea::vertical().id_salt("pref-shortcuts").auto_shrink([false, false]).show(ui, |ui| {
        ui.spacing_mut().item_spacing.y = 0.0;
        if commands.is_empty() {
            settings::note(ui, &t!("pref-shortcuts-none"));
        }
        for (command, label) in commands {
            let keys = keys_of(&custom, command);
            let changed = custom.contains_key(&command.id());
            settings::list_row(ui, &label, |ui| {
                // Right to left: Default, then what the keys were or why they
                // were refused, then the keys.
                ui.spacing_mut().item_spacing.x = 8.0;
                if command.customizable() && changed {
                    let reset = settings::button(ui, changed, &t!("pref-shortcut-reset"));
                    reset.widget_info(|| {
                        let text = format!("{} {label}", t!("pref-shortcut-reset"));
                        egui::WidgetInfo::labeled(egui::WidgetType::Button, changed, text)
                    });
                    if reset.clicked() {
                        change = Some((command, command.default_shortcut().unwrap_or_default().to_string()));
                    }
                }
                if !command.customizable() {
                    crate::ribbon::key_cap(ui, &keys.unwrap_or_default()).on_hover_text(t!("pref-shortcut-fixed"));
                    return;
                }
                let editing = editor.editing.as_ref().filter(|(c, _, _)| *c == command);
                if let Some((_, _, Some(why))) = editing {
                    ui.label(RichText::new(why).size(11.5).color(PALETTE.danger));
                } else if changed {
                    let default = command.default_shortcut().unwrap_or("—");
                    let text = t!("pref-shortcut-default", "keys" => default);
                    ui.label(RichText::new(text).size(11.5).color(PALETTE.text_faint));
                }
                let mut text = editing.map_or_else(|| keys.clone().unwrap_or_default(), |e| e.1.clone());
                let field = egui::TextEdit::singleline(&mut text)
                    .id(ui.id().with(("shortcut", command.id())))
                    .char_limit(2)
                    .desired_width(44.0)
                    .horizontal_align(Align::Center)
                    .font(egui::FontId::new(12.5, theme::semibold()))
                    .hint_text("—");
                settings::framed(ui);
                let r = ui.add(field);
                r.widget_info(|| egui::WidgetInfo::text_edit(true, keys.clone().unwrap_or_default(), &text, &label));
                let text: String =
                    text.chars().filter(char::is_ascii_alphabetic).map(|c| c.to_ascii_uppercase()).collect();
                if r.gained_focus() {
                    // Typed letters replace the keys.
                    if let Some(mut s) = egui::text_edit::TextEditState::load(ui.ctx(), r.id) {
                        let all = egui::text_selection::CCursorRange::two(
                            egui::text::CCursor::new(0),
                            egui::text::CCursor::new(text.chars().count()),
                        );
                        s.cursor.set_char_range(Some(all));
                        s.store(ui.ctx(), r.id);
                    }
                }
                if r.gained_focus() || (r.has_focus() && editor.editing.as_ref().is_none_or(|e| e.0 != command)) {
                    editor.editing = Some((command, text.clone(), None));
                } else if r.changed()
                    && let Some(e) = editor.editing.as_mut()
                {
                    e.1 = text.clone();
                    e.2 = None;
                    if text.len() == 2 {
                        change = Some((command, text.clone()));
                    }
                }
                if r.lost_focus() {
                    // Leaving the field empty takes the keys away; one letter keeps the old ones.
                    if text.is_empty() && keys.is_some() {
                        change = Some((command, String::new()));
                    }
                    if editor.editing.as_ref().is_some_and(|e| e.0 == command && e.2.is_none()) {
                        editor.editing = None;
                    }
                }
            });
        }
    });
    if let Some((command, keys)) = change {
        match set_keys(&mut state.prefs.shortcut_keys, command, &keys) {
            Ok(()) => {
                if editor.editing.as_ref().is_some_and(|e| e.0 == command) {
                    editor.editing = Some((command, keys, None));
                }
            }
            Err(other) => {
                let why = t!("pref-shortcut-taken", "keys" => keys.clone(), "command" => other.label(state));
                editor.editing = Some((command, keys, Some(why)));
            }
        }
    }
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
            trackpad_pans: false,
            cad_blocks: true,
            snaps: crate::snaps::SnapSettings { midpoint: false, elements: false, ..Default::default() },
            shortcut_keys: BTreeMap::from([("Move".to_string(), "MO".to_string()), ("Pin".to_string(), String::new())]),
            library_folders: vec![PathBuf::from("/lib/a"), PathBuf::from("C:\\lib b")],
            macro_folder: Some(PathBuf::from("/macros")),
            projects_folder: Some(PathBuf::from("/projects")),
            autosave_minutes: 0,
            recent_count: 20,
            plugin_folder: Some(PathBuf::from("/plugins")),
            plugins_off: BTreeSet::from(["old_tool".to_string()]),
            plugins_files: BTreeSet::from(["exporter".to_string(), "importer".to_string()]),
            plugins_tools: BTreeSet::from(["simulator".to_string()]),
            plugins_network: BTreeSet::from(["ikea_browser".to_string()]),
            plugin_dev_mode: true,
            plugin_dev_folder: Some(PathBuf::from("/dev/plugins")),
            panels: crate::side_panels::PanelLayout {
                left: vec![],
                right: vec![vec![crate::side_panels::Pane::Properties], vec![crate::side_panels::Pane::Browser]],
            },
            structure_material: cc_core::StructuralMaterial::Timber,
            quick_access: Some(vec!["Measure".to_string(), "Text".to_string()]),

            new_project_system: cc_core::MeasurementSystem::Imperial,
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
            (PLUGINS_OFF_KEY, "good_id\nBad Id\n../x\n".to_string()),
            (NEW_PROJECT_KEY, "nautical".to_string()),
        ]
        .into();
        let prefs = Preferences::read(|k| map.get(k).cloned(), f32::NAN);
        assert_eq!(prefs.ui_scale, 200);
        assert_eq!(prefs.font_size, 80);
        assert_eq!(prefs.autosave_minutes, 1, "unparsable: the default");
        assert_eq!(prefs.recent_count, 1);
        assert_eq!(prefs.theme, ThemeChoice::System);
        assert_eq!(prefs.new_project_system, cc_core::MeasurementSystem::Metric, "unknown template: metric");
        assert_eq!(prefs.plugins_off, BTreeSet::from(["good_id".to_string()]), "invalid plugin ids dropped");
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

    /// Stored shortcuts are hostile input: unknown commands, fixed Ctrl
    /// chords and keys that are not two letters are dropped.
    #[test]
    fn stored_shortcuts_are_checked() {
        let keys = shortcut_lines(
            "Move=mo\nPin=\nNoSuchCommand=XX\nSave=SS\nRotate=R1\nUndo=ZZ\r\nbroken line\nCopyElements= cp ",
        );
        let expected = [("CopyElements", "CP"), ("Move", "MO"), ("Pin", "")];
        assert_eq!(keys, expected.map(|(a, b)| (a.to_string(), b.to_string())).into_iter().collect());
        let many: String = (0..2000).map(|_| "Move=MO\n").collect();
        assert_eq!(shortcut_lines(&many).len(), 1);
    }

    /// Keys of another command are refused; the default keys clear the change.
    #[test]
    fn shortcut_changes_check_conflicts() {
        let mut custom = BTreeMap::new();
        assert_eq!(set_keys(&mut custom, UiCommand::Move, "WA"), Err(UiCommand::Place(cc_core::Category::Wall)));
        assert!(custom.is_empty());
        assert_eq!(set_keys(&mut custom, UiCommand::Move, "QQ"), Ok(()));
        assert_eq!(keys_of(&custom, UiCommand::Move).as_deref(), Some("QQ"));
        // Its old keys are free now, and another command may take them.
        assert_eq!(set_keys(&mut custom, UiCommand::Rotate, "MV"), Ok(()));
        assert_eq!(set_keys(&mut custom, UiCommand::Move, "MV"), Err(UiCommand::Rotate));
        assert_eq!(set_keys(&mut custom, UiCommand::Rotate, "RO"), Ok(()));
        assert_eq!(set_keys(&mut custom, UiCommand::Move, "MV"), Ok(()));
        assert!(custom.is_empty(), "back to the defaults");
        assert_eq!(set_keys(&mut custom, UiCommand::Pin, ""), Ok(()));
        assert_eq!(keys_of(&custom, UiCommand::Pin), None);
        assert_eq!(keys_of(&custom, UiCommand::Save).as_deref(), Some("Ctrl+S"), "fixed");
    }
}
