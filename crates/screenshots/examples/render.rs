//! Renders a sample slide to a PNG, for checking layout by eye:
//! `cargo run -p appdoctor-screenshots --example render -- <out.png> [screenshot.png]`
use std::path::PathBuf;

use appdoctor_screenshots::{CanvasSize, Frame, Slide, render_png};

fn main() {
    let mut args = std::env::args().skip(1);
    let out = PathBuf::from(args.next().expect("usage: render <out.png> [screenshot.png]"));
    let mut slide = Slide::default();
    slide.headline.text = "Plan your day in seconds".into();
    slide.subheadline.text = "Tasks, projects and reminders in one place".into();
    let mut dir = std::env::temp_dir();
    if let Some(shot) = args.next().map(PathBuf::from) {
        dir = shot.parent().unwrap().to_path_buf();
        slide.device.screenshot = Some(shot.file_name().unwrap().to_string_lossy().into_owned());
    }
    slide.device.frame = Frame::Black;
    let png = render_png(&slide, CanvasSize::Iphone69, &dir, 0.5).expect("render");
    std::fs::write(&out, png).expect("write");
}
