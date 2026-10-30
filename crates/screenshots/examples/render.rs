//! Renders every device model in a few finishes to PNGs, for checking the
//! frames by eye:
//! `cargo run -p appdoctor-screenshots --example render -- <out dir> [screenshot.png]`
use std::path::PathBuf;

use appdoctor_screenshots::{DeviceModel, Frame, LayerKind, Slide, render_png};

fn main() {
    let mut args = std::env::args().skip(1);
    let out = PathBuf::from(args.next().expect("usage: render <out dir> [screenshot.png]"));
    std::fs::create_dir_all(&out).expect("out dir");
    let shot = args.next().map(PathBuf::from);
    let dir = shot
        .as_ref()
        .and_then(|shot| shot.parent().map(PathBuf::from))
        .unwrap_or_else(std::env::temp_dir);
    for model in DeviceModel::all() {
        for frame in [Frame::Black, Frame::Silver] {
            let mut id = 0;
            let mut slide = Slide::starter(model.target(), "Plan your day in seconds", &mut id);
            let device = &mut slide.layers[2];
            device.height = device.width * model.aspect();
            device.kind = LayerKind::Device {
                model,
                frame,
                screenshot: shot
                    .as_ref()
                    .map(|shot| shot.file_name().unwrap().to_string_lossy().into_owned()),
                shadow: true,
                clip_start: 0.,
            };
            let size = model.target().sizes()[0];
            device.width = device.width.min(1000.);
            device.height = device.width * model.aspect();
            let png = render_png(std::slice::from_ref(&slide), 0, size, &dir, 0.35).expect("render");
            std::fs::write(out.join(format!("{}-{}.png", model.slug(), frame.slug())), png).expect("write");
        }
    }
}
