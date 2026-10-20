//! ConstructCAD — open source BIM authoring.

// Release builds on Windows open no console window.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod actions;
mod app;
mod browser;
mod canvas;
mod clipboard;
mod commands;
mod component_editor;
mod dialog;
mod dialogs;
mod dim_edit;
mod engine_edit;
mod home;
mod i18n;
mod icons;
mod material_browser;
mod modify;
mod paint;
mod palette;
mod properties;
mod ribbon;
mod schedule_ui;
mod sheet;
mod state;
mod systems_dialog;
mod table;
mod text_tool;
mod theme;
mod thumbnails;
mod tools;
mod type_dialog;
#[cfg(test)]
mod ui_tests;
mod units_dialog;
mod view3d;
mod view_cube;

fn main() -> eframe::Result<()> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("warn")).init();
    let args = parse_args(std::env::args().skip(1));
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1440.0, 900.0])
            .with_min_inner_size([900.0, 600.0])
            .with_title("ConstructCAD")
            .with_icon(app_icon()),
        renderer: eframe::Renderer::Wgpu,
        ..Default::default()
    };
    eframe::run_native("ConstructCAD", options, Box::new(|cc| Ok(Box::new(app::ConstructApp::new(cc, args)))))
}

/// Window and taskbar icon.
fn app_icon() -> egui::IconData {
    eframe::icon_data::from_png_bytes(theme::APP_ICON_PNG).unwrap_or_default()
}

/// `constructcad [project.ccad] [--view NAME]... [--lang pt-BR|en]`
fn parse_args(args: impl Iterator<Item = String>) -> app::Args {
    let mut out = app::Args::default();
    let mut args = args.peekable();
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--view" => out.views.extend(args.next()),
            "--lang" => out.language = args.next().and_then(|c| i18n::Language::from_code(&c)),
            _ if out.path.is_none() => out.path = Some(std::path::PathBuf::from(arg)),
            _ => {}
        }
    }
    out
}
