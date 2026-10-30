//! Motion: a design played as a video. Its slides become scenes shown one
//! after another, joined by a transition, and each layer can enter, leave
//! and drift over its scene. [`animate`] gives a layer as it looks at a
//! moment, so the editor's preview and the export agree.

use serde::{Deserialize, Serialize};

use crate::{Background, Layer, LayerKind, Slide};

/// How long a slide shows when it has no length of its own, in seconds.
pub const DEFAULT_SCENE: f32 = 3.;

/// Video settings; a design with these exports as a video too.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Video {
    #[serde(default = "thirty")]
    pub fps: u32,
    #[serde(default)]
    pub transition: Transition,
    /// Seconds a transition takes, at the end of the scene it leaves.
    #[serde(default = "half")]
    pub transition_length: f32,
}

impl Default for Video {
    fn default() -> Self {
        Video {
            fps: 30,
            transition: Transition::Push,
            transition_length: 0.5,
        }
    }
}

fn thirty() -> u32 {
    30
}

fn half() -> f32 {
    0.5
}

/// How one scene gives way to the next.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Transition {
    /// The next scene pushes in from the right, as screenshots swipe.
    /// Layers spanning the two stay joined.
    #[default]
    Push,
    /// The next scene fades in over this one.
    Fade,
    Cut,
}

impl Transition {
    pub const ALL: [Transition; 3] = [Transition::Push, Transition::Fade, Transition::Cut];

    pub fn label(self) -> &'static str {
        match self {
            Transition::Push => "Push",
            Transition::Fade => "Fade",
            Transition::Cut => "Cut",
        }
    }

    pub fn slug(self) -> &'static str {
        match self {
            Transition::Push => "push",
            Transition::Fade => "fade",
            Transition::Cut => "cut",
        }
    }

    pub fn from_slug(slug: &str) -> Option<Transition> {
        Transition::ALL.into_iter().find(|t| t.slug() == slug)
    }
}

/// How a layer comes in or goes out.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Effect {
    /// Simply there (or gone) at its time.
    #[default]
    None,
    Fade,
    /// Up into place from below, fading in; going out, it sinks away.
    Rise,
    /// In from the right; going out, away to the left.
    Slide,
    /// Grows into place from a little smaller.
    Zoom,
    /// Springs in from small with a slight overshoot.
    Pop,
    /// Text appears word by word over the effect's length (other layers
    /// fade).
    Words,
}

impl Effect {
    pub const ALL: [Effect; 7] = [
        Effect::None,
        Effect::Fade,
        Effect::Rise,
        Effect::Slide,
        Effect::Zoom,
        Effect::Pop,
        Effect::Words,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Effect::None => "None",
            Effect::Fade => "Fade",
            Effect::Rise => "Rise",
            Effect::Slide => "Slide",
            Effect::Zoom => "Zoom",
            Effect::Pop => "Pop",
            Effect::Words => "Words",
        }
    }

    pub fn slug(self) -> &'static str {
        match self {
            Effect::None => "none",
            Effect::Fade => "fade",
            Effect::Rise => "rise",
            Effect::Slide => "slide",
            Effect::Zoom => "zoom",
            Effect::Pop => "pop",
            Effect::Words => "words",
        }
    }

    pub fn from_slug(slug: &str) -> Option<Effect> {
        Effect::ALL.into_iter().find(|e| e.slug() == slug)
    }
}

/// A slow move over the scene, so still layers feel alive.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Drift {
    #[default]
    None,
    Up,
    Down,
    Left,
    Right,
    /// Grows slowly, like a camera pushing in.
    ZoomIn,
    ZoomOut,
}

impl Drift {
    pub const ALL: [Drift; 7] = [
        Drift::None,
        Drift::Up,
        Drift::Down,
        Drift::Left,
        Drift::Right,
        Drift::ZoomIn,
        Drift::ZoomOut,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Drift::None => "None",
            Drift::Up => "Up",
            Drift::Down => "Down",
            Drift::Left => "Left",
            Drift::Right => "Right",
            Drift::ZoomIn => "Zoom in",
            Drift::ZoomOut => "Zoom out",
        }
    }

    pub fn slug(self) -> &'static str {
        match self {
            Drift::None => "none",
            Drift::Up => "up",
            Drift::Down => "down",
            Drift::Left => "left",
            Drift::Right => "right",
            Drift::ZoomIn => "zoom-in",
            Drift::ZoomOut => "zoom-out",
        }
    }

    pub fn from_slug(slug: &str) -> Option<Drift> {
        Drift::ALL.into_iter().find(|d| d.slug() == slug)
    }
}

/// A camera move: the scene zooms in on a point, holds, and zooms back out.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Focus {
    /// Seconds into the scene that the move starts.
    pub at: f32,
    /// The point to zoom in on, in canvas pixels.
    pub x: f32,
    pub y: f32,
    /// How far in: 2 shows half the width.
    pub zoom: f32,
    /// Seconds held at the point before zooming back out.
    #[serde(default = "one_second")]
    pub hold: f32,
    /// Seconds each move (in, and back out) takes.
    #[serde(default = "move_time")]
    pub length: f32,
}

fn one_second() -> f32 {
    1.
}

fn move_time() -> f32 {
    0.7
}

/// Where the camera looks `time` seconds into the scene: the canvas point
/// at the centre of the frame, and the zoom. Kept inside the canvas.
pub fn camera(slide: &Slide, time: f32, canvas: (f32, f32)) -> (f32, f32, f32) {
    let centre = (canvas.0 / 2., canvas.1 / 2.);
    let Some((focus, e)) = slide.camera.iter().find_map(|focus| {
        let length = focus.length.max(0.05);
        let t = time - focus.at;
        let end = length * 2. + focus.hold.max(0.);
        (0. ..end).contains(&t).then(|| {
            let e = if t < length {
                ease_in_out(t / length)
            } else if t < length + focus.hold {
                1.
            } else {
                1. - ease_in_out((t - length - focus.hold) / length)
            };
            (focus, e)
        })
    }) else {
        return (centre.0, centre.1, 1.);
    };
    let zoom = 1. + (focus.zoom.max(1.) - 1.) * e;
    let (half_w, half_h) = (canvas.0 / 2. / zoom, canvas.1 / 2. / zoom);
    let x = (centre.0 + (focus.x - centre.0) * e).clamp(half_w, canvas.0 - half_w);
    let y = (centre.1 + (focus.y - centre.1) * e).clamp(half_h, canvas.1 - half_h);
    (x, y, zoom)
}

/// The slide's background `time` seconds in: a gradient turns by its spin.
pub fn background_at(slide: &Slide, time: f32) -> Background {
    match &slide.background {
        Background::Gradient { from, to, angle } if slide.spin != 0. => Background::Gradient {
            from: from.clone(),
            to: to.clone(),
            angle: (angle + slide.spin * time).rem_euclid(360.),
        },
        background => background.clone(),
    }
}

/// A layer as it shows at a moment.
#[derive(Debug, Clone, PartialEq)]
pub struct Shown {
    pub layer: Layer,
    /// How far into a device's screen recording, in seconds.
    pub clip: f32,
    /// For the Words effect: how many words show (the next one partly).
    /// `None` shows them all.
    pub words: Option<f32>,
}

/// A layer's timing within its scene.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Motion {
    /// Seconds into the scene that the layer comes in.
    pub start: f32,
    /// Seconds into the scene that it starts to leave; `None` stays.
    pub leave: Option<f32>,
    pub enter: Effect,
    pub exit: Effect,
    /// Seconds each effect takes.
    pub length: f32,
    pub drift: Drift,
}

impl Default for Motion {
    fn default() -> Self {
        Motion {
            start: 0.,
            leave: None,
            enter: Effect::None,
            exit: Effect::None,
            length: 0.6,
            drift: Drift::None,
        }
    }
}

impl Motion {
    /// Nothing moves: saved designs leave it out.
    pub fn is_still(&self) -> bool {
        *self == Motion::default()
    }
}

/// Where a moment falls in the video.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Moment {
    pub scene: usize,
    /// Seconds into that scene.
    pub time: f32,
    /// The scene coming in and how far the transition has got (0 to 1,
    /// eased).
    pub next: Option<(usize, f32)>,
}

/// The video's length: its scenes end to end.
pub fn duration(slides: &[Slide]) -> f32 {
    slides.iter().map(|slide| slide.duration).sum()
}

/// Where the video is `time` seconds in.
pub fn moment(slides: &[Slide], video: &Video, time: f32) -> Moment {
    let mut start = 0.;
    let last = slides.len().saturating_sub(1);
    for (scene, slide) in slides.iter().enumerate() {
        let end = start + slide.duration;
        if time < end || scene == last {
            let local = (time - start).clamp(0., slide.duration);
            let length = video.transition_length.min(slide.duration / 2.);
            let next = (scene < last && video.transition != Transition::Cut && length > 0.)
                .then(|| (local - (slide.duration - length)) / length)
                .filter(|p| *p > 0.)
                .map(|p| (scene + 1, ease_in_out(p.min(1.))));
            return Moment {
                scene,
                time: local,
                next,
            };
        }
        start = end;
    }
    Moment {
        scene: 0,
        time: 0.,
        next: None,
    }
}

/// When scene `index` starts, in seconds.
pub fn scene_start(slides: &[Slide], index: usize) -> f32 {
    slides.iter().take(index).map(|slide| slide.duration).sum()
}

fn ease_out(t: f32) -> f32 {
    1. - (1. - t).powi(3)
}

fn ease_in(t: f32) -> f32 {
    t.powi(3)
}

fn ease_in_out(t: f32) -> f32 {
    if t < 0.5 {
        4. * t.powi(3)
    } else {
        1. - (-2. * t + 2.).powi(3) / 2.
    }
}

/// Overshoots a little before settling, for Pop.
fn ease_back(t: f32) -> f32 {
    let (c1, c3) = (1.70158, 2.70158);
    1. + c3 * (t - 1.).powi(3) + c1 * (t - 1.).powi(2)
}

/// An effect's pull on a layer at `amount` (1 = fully out of place, 0 =
/// in place): (dx, dy, scale, opacity).
fn pull(effect: Effect, amount: f32, entering: bool, canvas: (f32, f32)) -> (f32, f32, f32, f32) {
    let (width, height) = canvas;
    match effect {
        Effect::None => (0., 0., 1., 1.),
        Effect::Fade => (0., 0., 1., 1. - amount),
        Effect::Rise => (0., height * 0.05 * amount * if entering { 1. } else { -1. }, 1., 1. - amount),
        Effect::Slide => (width * 0.35 * amount * if entering { 1. } else { -1. }, 0., 1., 1. - amount),
        Effect::Zoom => (0., 0., 1. - 0.15 * amount, 1. - amount),
        Effect::Pop => (0., 0., 1. - 0.6 * amount, (1. - amount * 1.5).clamp(0., 1.)),
        // Text reveals its words instead; anything else fades.
        Effect::Words => (0., 0., 1., 1. - amount),
    }
}

/// `layer` as it looks `time` seconds into a scene `scene` seconds long on
/// a `canvas` (width, height): moved, scaled about its centre and faded.
/// `None` while it isn't on screen.
pub fn animate(layer: &Layer, time: f32, scene: f32, canvas: (f32, f32)) -> Option<Shown> {
    let m = &layer.motion;
    let length = m.length.max(0.01);
    if time < m.start {
        return None;
    }
    let leave = m.leave.unwrap_or(f32::INFINITY);
    let gone = if m.exit == Effect::None { leave } else { leave + length };
    if time >= gone {
        return None;
    }

    let (mut dx, mut dy, mut scale, mut opacity) = (0., 0., 1., 1.);
    let mut apply = |(x, y, s, o): (f32, f32, f32, f32)| {
        dx += x;
        dy += y;
        scale *= s;
        opacity *= o;
    };
    let text = matches!(layer.kind, LayerKind::Text { .. });
    let mut words = None;
    if m.enter == Effect::Words && text {
        let count = layer.words().iter().map(Vec::len).sum::<usize>() as f32;
        words = Some(((time - m.start) / length).clamp(0., 1.) * count);
    } else if m.enter != Effect::None {
        let p = ((time - m.start) / length).clamp(0., 1.);
        let eased = if m.enter == Effect::Pop { ease_back(p) } else { ease_out(p) };
        apply(pull(m.enter, 1. - eased, true, canvas));
    }
    if m.exit != Effect::None && time >= leave {
        let q = ((time - leave) / length).clamp(0., 1.);
        apply(pull(m.exit, ease_in(q), false, canvas));
    }
    let progress = (time / scene.max(0.01)).clamp(0., 1.);
    let (width, height) = canvas;
    apply(match m.drift {
        Drift::None => (0., 0., 1., 1.),
        Drift::Up => (0., -height * 0.04 * progress, 1., 1.),
        Drift::Down => (0., height * 0.04 * progress, 1., 1.),
        Drift::Left => (-width * 0.06 * progress, 0., 1., 1.),
        Drift::Right => (width * 0.06 * progress, 0., 1., 1.),
        Drift::ZoomIn => (0., 0., 1. + 0.08 * progress, 1.),
        Drift::ZoomOut => (0., 0., 1.08 - 0.08 * progress, 1.),
    });

    let mut moved = layer.clone();
    let (cx, cy) = (layer.x + layer.width / 2. + dx, layer.y + layer.height / 2. + dy);
    moved.width = layer.width * scale;
    moved.height = layer.height * scale;
    moved.x = cx - moved.width / 2.;
    moved.y = cy - moved.height / 2.;
    moved.opacity = (layer.opacity * opacity).clamp(0., 1.);
    let mut clip_time = 0.;
    match &mut moved.kind {
        LayerKind::Text { size, .. } => *size *= scale,
        LayerKind::Device {
            clip_start, clip_speed, ..
        } => clip_time = *clip_start + (time - m.start).max(0.) * clip_speed.max(0.05),
        _ => {}
    }
    Some(Shown {
        layer: moved,
        clip: clip_time,
        words,
    })
}

/// Moves a layer through the camera: the point (x, y) at the centre of
/// the frame, zoomed by `zoom`.
fn through_camera(layer: &mut Layer, (x, y, zoom): (f32, f32, f32), canvas: (f32, f32)) {
    if zoom == 1. && x == canvas.0 / 2. && y == canvas.1 / 2. {
        return;
    }
    layer.x = canvas.0 / 2. + (layer.x - x) * zoom;
    layer.y = canvas.1 / 2. + (layer.y - y) * zoom;
    layer.width *= zoom;
    layer.height *= zoom;
    if let LayerKind::Text { size, .. } = &mut layer.kind {
        *size *= zoom;
    }
    if let LayerKind::Shape { soft, stroke_width, radius, .. } = &mut layer.kind {
        *soft *= zoom;
        *stroke_width *= zoom;
        *radius *= zoom;
    }
    if let LayerKind::Image { radius, .. } = &mut layer.kind {
        *radius *= zoom;
    }
}

/// The layers scene `index` shows `time` seconds in, as [`animate`] has
/// them and the scene's camera frames them, with each one's owning slide. Screenshot scenes
/// include the parts of neighbours' layers spanning in, at the owner's own
/// time (finished for earlier scenes, not started for later ones). Layers
/// owned by `skip` are left out: during a push both scenes are drawn and
/// share those.
pub fn scene_layers(
    slides: &[Slide],
    index: usize,
    time: f32,
    canvas: (f32, f32),
    spans: bool,
    skip: Option<usize>,
) -> Vec<(usize, Shown)> {
    let view = slides.get(index).map_or((canvas.0 / 2., canvas.1 / 2., 1.), |slide| camera(slide, time, canvas));
    let visible: Vec<(usize, Layer)> = if spans {
        crate::visible_layers(slides, index, canvas.0)
    } else {
        slides
            .get(index)
            .map(|slide| slide.layers.iter().filter(|l| !l.hidden).map(|l| (index, l.clone())).collect())
            .unwrap_or_default()
    };
    visible
        .into_iter()
        .filter(|(owner, _)| Some(*owner) != skip)
        .filter_map(|(owner, layer)| {
            let scene = slides.get(owner).map_or(DEFAULT_SCENE, |s| s.duration);
            let at = match owner.cmp(&index) {
                std::cmp::Ordering::Less => scene,
                std::cmp::Ordering::Equal => time,
                std::cmp::Ordering::Greater => 0.,
            };
            animate(&layer, at, scene, canvas).map(|mut shown| {
                through_camera(&mut shown.layer, view, canvas);
                (owner, shown)
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Target;

    fn slides(lengths: &[f32]) -> Vec<Slide> {
        let mut id = 0;
        lengths
            .iter()
            .map(|length| Slide {
                duration: *length,
                ..Slide::starter(Target::Iphone, "Scene", &mut id)
            })
            .collect()
    }

    #[test]
    fn moments_walk_the_scenes_and_their_transitions() {
        let slides = slides(&[3., 4.]);
        let video = Video::default();
        assert_eq!(duration(&slides), 7.);
        let early = moment(&slides, &video, 1.);
        assert_eq!((early.scene, early.time, early.next), (0, 1., None));
        let pushing = moment(&slides, &video, 2.75);
        assert_eq!(pushing.scene, 0);
        let (next, progress) = pushing.next.unwrap();
        assert_eq!(next, 1);
        assert!((progress - 0.5).abs() < 1e-3);
        let later = moment(&slides, &video, 5.);
        assert_eq!((later.scene, later.time), (1, 2.));
        assert!(later.next.is_none(), "no transition after the last scene");
        let cut = Video {
            transition: Transition::Cut,
            ..video
        };
        assert!(moment(&slides, &cut, 2.9).next.is_none());
        assert_eq!(scene_start(&slides, 1), 3.);
    }

    #[test]
    fn layers_enter_drift_and_leave() {
        let mut layer = Layer::shape(Target::Iphone, 1);
        layer.motion = Motion {
            start: 1.,
            leave: Some(2.),
            enter: Effect::Rise,
            exit: Effect::Fade,
            length: 0.5,
            drift: Drift::None,
        };
        let canvas = Target::Iphone.canvas();
        assert!(animate(&layer, 0.5, 3., canvas).is_none(), "not in yet");
        let entering = animate(&layer, 1.1, 3., canvas).unwrap().layer;
        assert!(entering.y > layer.y && entering.opacity < 1.);
        let settled = animate(&layer, 1.8, 3., canvas).unwrap().layer;
        assert!((settled.y - layer.y).abs() < 0.01 && (settled.opacity - 1.).abs() < 1e-4);
        let leaving = animate(&layer, 2.25, 3., canvas).unwrap().layer;
        assert!(leaving.opacity < 1. && leaving.opacity > 0.);
        assert!(animate(&layer, 2.6, 3., canvas).is_none(), "gone");

        layer.motion = Motion {
            drift: Drift::ZoomIn,
            ..Motion::default()
        };
        let start = animate(&layer, 0., 3., canvas).unwrap().layer;
        let end = animate(&layer, 3., 3., canvas).unwrap().layer;
        assert!(end.width > start.width);
        assert!(((end.x + end.width / 2.) - (layer.x + layer.width / 2.)).abs() < 0.01, "zooms about its centre");
    }

    #[test]
    fn screen_recordings_play_from_the_layers_start() {
        let mut device = Layer::device(Target::Iphone, 1, 0.);
        if let LayerKind::Device {
            clip_start, clip_speed, ..
        } = &mut device.kind
        {
            *clip_start = 2.;
            *clip_speed = 2.;
        }
        device.motion.start = 0.5;
        let clip = animate(&device, 1.5, 3., Target::Iphone.canvas()).unwrap().clip;
        assert!((clip - 4.).abs() < 1e-4, "{clip}");
    }

    #[test]
    fn words_come_in_one_by_one() {
        let mut layer = Layer::text(Target::Iphone, 1, "Sleep *better* tonight", 0., 100., 700, "#fff");
        layer.motion.enter = Effect::Words;
        layer.motion.length = 1.5;
        let canvas = Target::Iphone.canvas();
        let at = |t: f32| animate(&layer, t, 3., canvas).unwrap().words.unwrap();
        assert_eq!(at(0.), 0.);
        assert!((at(0.75) - 1.5).abs() < 1e-4);
        assert_eq!(at(2.), 3.);
        let words: Vec<_> = layer.words().concat();
        assert_eq!(words.iter().map(|w| w.text.as_str()).collect::<Vec<_>>(), ["Sleep", "better", "tonight"]);
        assert_eq!(words.iter().map(|w| w.accent).collect::<Vec<_>>(), [false, true, false]);
        assert_eq!(layer.lines().concat(), "Sleep better tonight");
    }

    #[test]
    fn the_camera_zooms_in_holds_and_returns() {
        let canvas = Target::Iphone.canvas();
        let mut id = 0;
        let mut slide = Slide::starter(Target::Iphone, "Zoom", &mut id);
        slide.camera.push(Focus {
            at: 1.,
            x: 660.,
            y: 2000.,
            zoom: 2.,
            hold: 1.,
            length: 0.5,
        });
        assert_eq!(camera(&slide, 0.5, canvas), (660., 1434., 1.));
        let (x, y, zoom) = camera(&slide, 2., canvas);
        assert_eq!((x, y, zoom), (660., 2000., 2.));
        assert_eq!(camera(&slide, 3.5, canvas).2, 1.);
        // Layers move through it: the focus point lands mid-frame.
        let shown = scene_layers(std::slice::from_ref(&slide), 0, 2., canvas, true, None);
        let device = &shown[2].1.layer;
        assert!(device.width > slide.layers[2].width * 1.9);
        slide.spin = 90.;
        assert!(matches!(background_at(&slide, 1.), Background::Gradient { angle, .. } if angle == 250.));
    }

    #[test]
    fn still_motion_stays_out_of_saved_designs() {
        let layer = Layer::shape(Target::Iphone, 1);
        let json = serde_json::to_string(&layer).unwrap();
        assert!(!json.contains("motion"));
    }
}
