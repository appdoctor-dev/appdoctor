//! AppDoctor desktop: local-first App Store Optimization.
//!
//! Startup order matters for gpui: open everything that can fail (data
//! directory, database) before the platform starts, then build the window,
//! and only then start async work, from the window handle (see the gpui
//! traps in Cap's desktop-gpui README).

mod assets;
mod discover;
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
    // stdout belongs to the MCP protocol in `appdoctor mcp`; logs go to stderr.
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "appdoctor=info,warn".into()),
        )
        .with_writer(std::io::stderr)
        .init();

    if std::env::args().nth(1).as_deref() == Some("mcp") {
        serve_mcp();
        return;
    }

    // Text layers measure with the system fonts; scanning them takes a
    // moment, so it happens before anyone opens the screenshot designer.
    std::thread::spawn(appdoctor_screenshots::warm_fonts);

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

/// `appdoctor mcp`: the local MCP server for the user's own AI agent, over
/// stdin and stdout. No window, and no network.
fn serve_mcp() {
    let data_dir = match services::data_dir() {
        Ok(dir) => dir,
        Err(error) => {
            eprintln!("AppDoctor couldn't find its data: {error:#}");
            std::process::exit(1);
        }
    };
    tracing::info!(data_dir = %data_dir.display(), "serving MCP on stdio");
    let server = appdoctor_mcp::Server::new(data_dir);
    if let Err(error) = server.serve(std::io::stdin().lock(), std::io::stdout().lock()) {
        eprintln!("AppDoctor's MCP server stopped: {error}");
        std::process::exit(1);
    }
}
