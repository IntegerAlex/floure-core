mod app;
mod theme;
mod views;

use gpui::{
    App, AppContext, Bounds, QuitMode, WindowBounds, WindowDecorations, WindowOptions, px, size,
};
use gpui_component::Root;

use floure_core::config::{AppConfig, history_db_path};
use floure_core::{Engine, Paths, migrate};

fn main() {
    if std::env::args().any(|arg| arg == "--version") {
        println!("floure {}", floure_core::version());
        return;
    }

    // Migrate before loading config: the bridge writes values into config.json.
    let config_path = floure_core::config::config_path();
    let report = migrate(&history_db_path(), &config_path);
    if let Err(error) = &report {
        eprintln!("[migrate] {error}");
    }
    let config = AppConfig::load();

    let engine = match Engine::new(config.clone(), Paths::from_config(&config)) {
        Ok(engine) => Some(engine),
        Err(error) => {
            eprintln!("[engine] {error}");
            None
        }
    };

    gpui_platform::application()
        .with_quit_mode(QuitMode::Explicit)
        .run(move |cx: &mut App| {
            gpui_component::init(cx);
            cx.bind_keys(app::key_bindings());

            let bounds = Bounds::centered(None, size(px(960.), px(700.)), cx);
            let config = config.clone();
            cx.open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(bounds)),
                    window_min_size: Some(size(px(720.), px(560.))),
                    titlebar: None,
                    window_decorations: Some(WindowDecorations::Client),
                    app_id: Some("floure".into()),
                    inactive_frame_interval: None,
                    ..Default::default()
                },
                move |window, cx| {
                    let config = config.clone();
                    let view = cx.new(|cx| {
                        app::FloureApp::new(window, cx, config, engine, floure_core::version())
                    });
                    cx.new(|cx| Root::new(view, window, cx))
                },
            )
            .expect("failed to open Floure window");
        });
}
