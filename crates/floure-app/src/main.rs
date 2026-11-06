use gpui::{
    App, Bounds, Context, IntoElement, ParentElement, Render, Styled, Window, WindowBounds,
    WindowDecorations, WindowOptions, div, prelude::*, px, rgb, size,
};
use gpui_component::Root;

struct FloureApp;

impl Render for FloureApp {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .size_full()
            .bg(rgb(0x000000))
            .text_color(rgb(0xF5F5F7))
            .child("Floure")
    }
}

fn main() {
    if std::env::args().any(|arg| arg == "--version") {
        println!("floure {}", floure_core::version());
        return;
    }

    gpui_platform::application().run(|cx: &mut App| {
        gpui_component::init(cx);

        let bounds = Bounds::centered(None, size(px(960.), px(700.)), cx);
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                titlebar: None,
                window_decorations: Some(WindowDecorations::Client),
                app_id: Some("floure".into()),
                inactive_frame_interval: None,
                ..Default::default()
            },
            |window, cx| {
                let view = cx.new(|_| FloureApp);
                cx.new(|cx| Root::new(view, window, cx))
            },
        )
        .expect("failed to open Floure window");
    });
}
