//! Renders a starter slide to a PNG, for checking layout by eye:
//! `cargo run -p appdoctor-screenshots --example render -- <out.png> [screenshot.png]`
use std::path::PathBuf;

use appdoctor_screenshots::{CanvasSize, LayerKind, Slide, render_png};

fn main() {
    let mut args = std::env::args().skip(1);
    let out = PathBuf::from(args.next().expect("usage: render <out.png> [screenshot.png]"));
    let mut id = 0;
    let mut slide = Slide::starter("Plan your day in seconds", &mut id);
    let mut dir = std::env::temp_dir();
    if let Some(shot) = args.next().map(PathBuf::from) {
        dir = shot.parent().unwrap().to_path_buf();
        if let LayerKind::Device { screenshot, .. } = &mut slide.layers[2].kind {
            *screenshot = Some(shot.file_name().unwrap().to_string_lossy().into_owned());
        }
    }
    let png = render_png(std::slice::from_ref(&slide), 0, CanvasSize::Iphone69, &dir, 0.5).expect("render");
    std::fs::write(&out, png).expect("write");
}
