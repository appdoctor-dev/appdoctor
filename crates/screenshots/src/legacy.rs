//! Reading saved designs, including the first format (a fixed headline,
//! subheadline and device per slide), which converts to layers.

use serde::Deserialize;

use crate::{BASE_HEIGHT, BASE_WIDTH, Background, Design, Frame, Layer, LayerKind, Slide, Target};

/// The first format's phone shape.
const V1_DEVICE_ASPECT: f32 = 2.167;

#[derive(Deserialize)]
struct Current {
    #[serde(default)]
    target: Target,
    #[serde(default)]
    video: Option<crate::Video>,
    slides: Vec<Slide>,
}

#[derive(Deserialize)]
struct V1 {
    slides: Vec<V1Slide>,
}

#[derive(Deserialize)]
struct V1Slide {
    background: Background,
    headline: V1Caption,
    subheadline: V1Caption,
    device: V1Device,
}

#[derive(Deserialize)]
struct V1Caption {
    text: String,
    color: String,
    size: f32,
    weight: u16,
}

#[derive(Deserialize)]
struct V1Device {
    frame: Frame,
    scale: f32,
    offset: f32,
    screenshot: Option<String>,
}

pub(crate) fn parse(bytes: &[u8]) -> Option<Design> {
    if let Ok(current) = serde_json::from_slice::<Current>(bytes) {
        return Some(Design {
            version: 2,
            target: current.target,
            video: current.video,
            slides: current.slides,
        });
    }
    let v1: V1 = serde_json::from_slice(bytes).ok()?;
    let mut next_id = 0;
    let slides = v1
        .slides
        .into_iter()
        .map(|slide| {
            let mut id = || {
                next_id += 1;
                next_id
            };
            let headline = caption(id(), &slide.headline, 215.);
            let subheadline = caption(id(), &slide.subheadline, headline.y + headline.height + 20.);
            let width = BASE_WIDTH * 0.78 * slide.device.scale.clamp(0.5, 1.1);
            let mut device = Layer::device(
                Target::Iphone,
                id(),
                subheadline.y + subheadline.height + 86. + slide.device.offset * BASE_HEIGHT,
            );
            device.width = width;
            device.height = width * V1_DEVICE_ASPECT;
            device.x = (BASE_WIDTH - width) / 2.;
            device.kind = LayerKind::Device {
                model: Target::Iphone.default_model(),
                frame: slide.device.frame,
                screenshot: slide.device.screenshot,
                shadow: true,
                clip_start: 0.,
            };
            Slide {
                background: slide.background,
                layers: vec![headline, subheadline, device],
                duration: crate::motion::DEFAULT_SCENE,
            }
        })
        .collect();
    Some(Design {
        version: 2,
        target: Target::Iphone,
        video: None,
        slides,
    })
}

fn caption(id: u64, caption: &V1Caption, y: f32) -> Layer {
    Layer::text(Target::Iphone, id, &caption.text, y, caption.size, caption.weight, &caption.color)
}
