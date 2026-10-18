//! ConstructCAD — open source BIM authoring.

mod actions;
mod app;
mod browser;
mod canvas;
mod dialogs;
mod i18n;
mod paint;
mod properties;
mod ribbon;
mod schedule_ui;
mod sheet;
mod state;
mod tools;
mod view3d;

fn main() -> eframe::Result<()> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("warn")).init();
    let args = parse_args(std::env::args().skip(1));
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1440.0, 900.0])
            .with_min_inner_size([900.0, 600.0])
            .with_title("ConstructCAD"),
        renderer: eframe::Renderer::Wgpu,
        ..Default::default()
    };
    eframe::run_native("ConstructCAD", options, Box::new(|cc| Ok(Box::new(app::ConstructApp::new(cc, args)))))
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
