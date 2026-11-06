mod app;
mod assets;
mod control_server;
mod hotkeys;
mod theme;
mod tray;
mod views;

use std::sync::Arc;

use gpui::{
    App, AppContext, Bounds, QuitMode, WindowBounds, WindowDecorations, WindowOptions, px, size,
};
use gpui_component::Root;

use floure_core::config::{AppConfig, history_db_path};
use floure_core::{Engine, Paths, migrate};
use hotkeys::HotkeyStatus;

fn main() {
    // Cap OpenMP before llama.cpp creates its pool. The GPU compositor needs
    // headroom; all-core inference is what got the old app marked unresponsive.
    let threads = floure_core::compute::inference_threads();
    // SAFETY: called before any other thread exists.
    unsafe { std::env::set_var("OMP_NUM_THREADS", threads.to_string()) };

    if handle_cli() {
        return;
    }
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
    let data_dir = floure_core::config::data_dir();
    if let Err(error) = std::fs::create_dir_all(&data_dir) {
        eprintln!("[data] {}: {error}", data_dir.display());
    }
    if let Err(error) = std::fs::create_dir_all(&config.model_dir) {
        eprintln!("[models] {}: {error}", config.model_dir.display());
    }

    let engine = match Engine::new(config.clone(), Paths::from_config(&config)) {
        Ok(engine) => Some(Arc::new(engine)),
        Err(error) => {
            eprintln!("[engine] {error}");
            None
        }
    };

    let hotkey_status = match &engine {
        Some(engine) => hotkeys::start(engine.clone(), &config),
        None => HotkeyStatus::Failed("engine unavailable".to_string()),
    };
    if let Some(engine) = &engine {
        // A second instance must not open a second audio engine. The bound
        // control port is the singleton check.
        if let Err(error) = control_server::spawn(engine.clone()) {
            eprintln!(
                "floure: another instance is already running ({error}). Use `floure --toggle`."
            );
            return;
        }
        tray::spawn(engine.clone());
    }

    gpui_platform::application()
        .with_assets(assets::Assets)
        .with_quit_mode(QuitMode::LastWindowClosed)
        .run(move |cx: &mut App| {
            if let Err(error) = cx.text_system().add_fonts(vec![assets::font()]) {
                eprintln!("[fonts] {error}");
            }
            gpui_component::init(cx);
            apply_component_theme(cx);
            cx.bind_keys(app::key_bindings());
            if let Some(engine) = engine.clone() {
                cx.on_app_quit(move |_| {
                    engine.stop_blocking();
                    async {}
                })
                .detach();
            }

            let bounds = Bounds::centered(None, size(px(960.), px(700.)), cx);
            let config = config.clone();
            let engine = engine.clone();
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
                    let engine = engine.clone();
                    let hotkey_status = hotkey_status.clone();
                    let view = cx.new(|cx| {
                        app::FloureApp::new(
                            window,
                            cx,
                            config,
                            engine.clone(),
                            floure_core::version(),
                            hotkey_status,
                        )
                    });
                    view.update(cx, |app, cx| app.attach_events(cx));
                    if let Some(engine) = engine {
                        engine.warm();
                    }
                    cx.new(|cx| Root::new(view, window, cx))
                },
            )
            .expect("failed to open Floure window");
        });
}

/// gpui-component's own widgets (search input, menus) follow the same
/// monochrome tokens as the shell instead of its default palettes.
fn apply_component_theme(cx: &mut App) {
    use gpui_component::{Theme, ThemeMode};
    Theme::change(ThemeMode::Dark, None, cx);
    Theme::update(cx, |theme| {
        theme.shadow = false;
        theme.font_family = theme::FONT_FAMILY.into();
        theme.font_size = px(15.0);
        theme.colors.background = theme::ground().into();
        theme.colors.foreground = theme::text().into();
        theme.colors.border = theme::border().into();
        theme.colors.input = theme::surface().into();
        theme.colors.muted = theme::surface().into();
        theme.colors.muted_foreground = theme::text_muted().into();
        theme.colors.caret = theme::text().into();
        theme.colors.primary = theme::accent().into();
        theme.colors.primary_foreground = theme::text().into();
    });
}

/// Handle the CLI trigger surface before any GPUI or audio initialisation, so
/// `--toggle` never opens a second window or engine.
fn handle_cli() -> bool {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.first().map(String::as_str) == Some("--diagnostics") {
        match floure_core::diagnostics::get_diagnostics() {
            Ok(json) => println!("{json}"),
            Err(error) => {
                eprintln!("floure: {error}");
                std::process::exit(1);
            }
        }
        return true;
    }
    let (method, path) = match args.first().map(String::as_str) {
        Some("--toggle") => ("POST", "/toggle"),
        Some("--start") => ("POST", "/start"),
        Some("--stop") => ("POST", "/stop"),
        Some("--status") => ("GET", "/status"),
        Some("--show") => ("POST", "/show"),
        _ => return false,
    };
    match control_server::request(method, path) {
        Ok(body) => println!("{body}"),
        Err(error) => {
            eprintln!("floure: {error}. Is Floure running?");
            std::process::exit(1);
        }
    }
    true
}
