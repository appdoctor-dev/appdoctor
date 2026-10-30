//! Exports a design file's video, for checking speed and output by eye:
//! `cargo run --release -p appdoctor-video --example export -- <design.json> <out.mp4> [header|search|iphone|ipad]`
use std::path::PathBuf;
use std::time::Instant;

use appdoctor_screenshots::{Design, Target, Video};

fn main() {
    let mut args = std::env::args().skip(1);
    let path = PathBuf::from(args.next().expect("design.json"));
    let out = PathBuf::from(args.next().expect("out.mp4"));
    let mut design = Design::load(&path);
    if let Some(target) = args.next().and_then(|t| Target::from_slug(&t)) {
        design.target = target;
    }
    design.video.get_or_insert_with(Video::default);
    let started = Instant::now();
    appdoctor_video::export(&design, path.parent().unwrap(), &out, |_| true).expect("export");
    println!("{:.1} s for {:.1} s of video", started.elapsed().as_secs_f32(), appdoctor_screenshots::motion::duration(&design.slides));
}
