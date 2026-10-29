//! AppDoctor desktop: local-first App Store Optimization.
//!
//! Startup order matters for gpui: open everything that can fail (data
//! directory, database) before the platform starts, then build the window,
//! and only then start async work, from the window handle (see the gpui
//! traps in Cap's desktop-gpui README).

mod assets;
mod entitlement;
mod icons;
mod rank_check;
mod runtime;
mod services;
mod state;
mod theme;
mod ui;

use gpui_kit::component::TitleBar;
use gpui_kit::*;

actions!(appdoctor, [Quit]);

fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "appdoctor=info,warn".into()),
        )
        .init();

    runtime::init();
    let services = match services::Services::open() {
        Ok(services) => services,
        Err(error) => {
            eprintln!("AppDoctor couldn't open its data: {error:#}");
            std::process::exit(1);
        }
    };
    tracing::info!(data_dir = %services.data_dir.display(), "starting");

    gpui_kit::application().with_assets(assets::Assets).run(move |cx| {
        gpui_kit::init(cx);
        theme::apply(None, cx);

        cx.on_action(|_: &Quit, cx| cx.quit());
        cx.bind_keys([KeyBinding::new(
            if cfg!(target_os = "macos") { "cmd-q" } else { "ctrl-q" },
            Quit,
            None,
        )]);
        cx.set_menus(vec![Menu {
            name: "AppDoctor".into(),
            items: vec![MenuItem::action("Quit AppDoctor", Quit)],
            disabled: false,
        }]);
        cx.on_window_closed(|cx, _| {
            if cx.windows().is_empty() {
                cx.quit();
            }
        })
        .detach();

        let options = WindowOptions {
            window_bounds: Some(WindowBounds::centered(size(px(1240.), px(800.)), cx)),
            window_min_size: Some(size(px(960.), px(600.))),
            titlebar: Some(TitleBar::title_bar_options()),
            app_id: Some("app.appdoctor.desktop".into()),
            ..Default::default()
        };
        let state = cx.new(|cx| state::AppState::new(services, cx));
        let opened = gpui_kit::open_window(options, cx, {
            let state = state.clone();
            move |window, cx| cx.new(|cx| ui::Shell::new(state, window, cx))
        });
        if let Err(error) = opened {
            eprintln!("AppDoctor couldn't open its window: {error:#}");
            cx.quit();
            return;
        }
        state.update(cx, |state, cx| state.start(cx));
        cx.activate(true);
    });
}
