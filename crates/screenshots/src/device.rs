//! Device frames: every iPhone and iPad a screenshot can sit in, their
//! finishes, and where each part goes. The editor's canvas and the SVG
//! export both draw from [`device_geometry`], so they agree.
//!
//! Each model is drawn from Apple's published measurements (body size in
//! millimetres, screen resolution and pixel density, screen corner radius
//! in points), not from Apple artwork, so the frames can ship with the app.

use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::{Layer, Rect, Target};

/// What sits in the glass above the screen.
#[derive(Debug, Clone, Copy, PartialEq)]
enum Cutout {
    /// The Dynamic Island.
    Island,
    /// A notch this many millimetres wide.
    Notch(f32),
    /// A round camera hole in the screen (iPhone Duo).
    Punch,
    /// A camera in the bezel: above the screen, or (iPads whose camera
    /// sits on the long edge) beside it.
    Camera { side: bool },
    /// A home button below the screen and a camera above it.
    Home,
}

/// Where the side buttons go.
#[derive(Debug, Clone, Copy, PartialEq)]
enum Buttons {
    Iphone,
    IphoneClassic,
    Ipad,
}

/// One model's measurements.
#[derive(Debug)]
struct Spec {
    slug: &'static str,
    label: &'static str,
    target: Target,
    /// Body width and height in millimetres, as held for the screenshot.
    body: (f32, f32),
    /// Screen resolution in pixels, and its pixel density.
    pixels: (u32, u32),
    ppi: f32,
    /// Pixels per point: 3 on most iPhones, 2 on iPads and older ones.
    scale: f32,
    /// The screen's corner radius in points; 0 for square screens.
    radius_pt: f32,
    cutout: Cutout,
    buttons: Buttons,
    /// A foldable's crease down the middle of the screen.
    crease: bool,
}

const fn phone(
    slug: &'static str,
    label: &'static str,
    body: (f32, f32),
    pixels: (u32, u32),
    ppi: f32,
    radius_pt: f32,
    cutout: Cutout,
) -> Spec {
    Spec {
        slug,
        label,
        target: Target::Iphone,
        body,
        pixels,
        ppi,
        scale: 3.,
        radius_pt,
        cutout,
        buttons: Buttons::Iphone,
        crease: false,
    }
}

const fn tablet(
    slug: &'static str,
    label: &'static str,
    body: (f32, f32),
    pixels: (u32, u32),
    ppi: f32,
    radius_pt: f32,
    cutout: Cutout,
) -> Spec {
    Spec {
        slug,
        label,
        target: Target::Ipad,
        body,
        pixels,
        ppi,
        scale: 2.,
        radius_pt,
        cutout,
        buttons: Buttons::Ipad,
        crease: false,
    }
}

/// Notch widths: iPhone X to 12, then the narrower one from iPhone 13.
const WIDE_NOTCH: Cutout = Cutout::Notch(34.7);
const NOTCH: Cutout = Cutout::Notch(27.8);
const SIDE_CAMERA: Cutout = Cutout::Camera { side: true };
const TOP_CAMERA: Cutout = Cutout::Camera { side: false };

/// Newest first within each family; the pickers list them in this order.
static CATALOG: &[Spec] = &[
    phone("iphone-18-pro-max", "iPhone 18 Pro Max", (78.0, 163.4), (1320, 2868), 460., 62., Cutout::Island),
    phone("iphone-18-pro", "iPhone 18 Pro", (71.9, 150.0), (1206, 2622), 460., 62., Cutout::Island),
    Spec {
        slug: "iphone-duo",
        label: "iPhone Duo (closed)",
        ..phone("", "", (84.1, 117.8), (1398, 2034), 460., 55., Cutout::Punch)
    },
    Spec {
        slug: "iphone-duo-open",
        label: "iPhone Duo (open)",
        buttons: Buttons::Ipad,
        crease: true,
        ..phone("", "", (164.6, 117.8), (2670, 1878), 430., 40., Cutout::Punch)
    },
    phone("iphone-air", "iPhone Air", (74.7, 156.2), (1260, 2736), 460., 62., Cutout::Island),
    phone("iphone-17-pro-max", "iPhone 17 Pro Max", (78.0, 163.4), (1320, 2868), 460., 62., Cutout::Island),
    phone("iphone-17-pro", "iPhone 17 Pro", (71.9, 150.0), (1206, 2622), 460., 62., Cutout::Island),
    phone("iphone-17", "iPhone 17", (71.5, 149.6), (1206, 2622), 460., 62., Cutout::Island),
    phone("iphone-17e", "iPhone 17e", (71.5, 146.7), (1170, 2532), 460., 47.33, NOTCH),
    phone("iphone-16-pro-max", "iPhone 16 Pro Max", (77.6, 163.0), (1320, 2868), 460., 62., Cutout::Island),
    phone("iphone-16-pro", "iPhone 16 Pro", (71.5, 149.6), (1206, 2622), 460., 62., Cutout::Island),
    phone("iphone-16-plus", "iPhone 16 Plus", (77.8, 160.9), (1290, 2796), 460., 55., Cutout::Island),
    phone("iphone-16", "iPhone 16", (71.6, 147.6), (1179, 2556), 460., 55., Cutout::Island),
    phone("iphone-16e", "iPhone 16e", (71.5, 146.7), (1170, 2532), 460., 47.33, NOTCH),
    phone("iphone-15-pro-max", "iPhone 15 Pro Max", (76.7, 159.9), (1290, 2796), 460., 55., Cutout::Island),
    phone("iphone-15-pro", "iPhone 15 Pro", (70.6, 146.6), (1179, 2556), 460., 55., Cutout::Island),
    phone("iphone-15-plus", "iPhone 15 Plus", (77.8, 160.9), (1290, 2796), 460., 55., Cutout::Island),
    phone("iphone-15", "iPhone 15", (71.6, 147.6), (1179, 2556), 460., 55., Cutout::Island),
    phone("iphone-14-pro-max", "iPhone 14 Pro Max", (77.6, 160.7), (1290, 2796), 460., 55., Cutout::Island),
    phone("iphone-14-pro", "iPhone 14 Pro", (71.5, 147.5), (1179, 2556), 460., 55., Cutout::Island),
    phone("iphone-14-plus", "iPhone 14 Plus", (78.1, 160.8), (1284, 2778), 458., 53.33, NOTCH),
    phone("iphone-14", "iPhone 14", (71.5, 146.7), (1170, 2532), 460., 47.33, NOTCH),
    phone("iphone-13-mini", "iPhone 13 mini", (64.2, 131.5), (1080, 2340), 476., 44., NOTCH),
    phone("iphone-12", "iPhone 12", (71.5, 146.7), (1170, 2532), 460., 47.33, WIDE_NOTCH),
    Spec {
        scale: 2.,
        ..phone("iphone-11", "iPhone 11", (75.7, 150.9), (828, 1792), 326., 41.5, WIDE_NOTCH)
    },
    phone("iphone-x", "iPhone X and XS", (70.9, 143.6), (1125, 2436), 458., 39., WIDE_NOTCH),
    Spec {
        scale: 2.,
        buttons: Buttons::IphoneClassic,
        ..phone("iphone-se", "iPhone SE", (67.3, 138.4), (750, 1334), 326., 0., Cutout::Home)
    },
    Spec {
        buttons: Buttons::IphoneClassic,
        ..phone("iphone-8-plus", "iPhone 8 Plus", (78.1, 158.4), (1080, 1920), 401., 0., Cutout::Home)
    },
    tablet("ipad-pro-13", "iPad Pro 13-inch (M4 and later)", (215.5, 281.6), (2064, 2752), 264., 18., SIDE_CAMERA),
    tablet("ipad-pro-11", "iPad Pro 11-inch (M4 and later)", (177.5, 249.7), (1668, 2420), 264., 18., SIDE_CAMERA),
    tablet("ipad-air-13", "iPad Air 13-inch", (214.9, 280.6), (2048, 2732), 264., 18., SIDE_CAMERA),
    tablet("ipad-air-11", "iPad Air 11-inch", (178.5, 247.6), (1640, 2360), 264., 18., SIDE_CAMERA),
    tablet("ipad-11", "iPad 11-inch (A16)", (179.5, 248.6), (1640, 2360), 264., 18., SIDE_CAMERA),
    tablet("ipad-mini", "iPad mini (A17 Pro)", (134.8, 195.4), (1488, 2266), 326., 21.5, TOP_CAMERA),
    tablet("ipad-pro-12-9", "iPad Pro 12.9-inch (2018–2022)", (214.9, 280.6), (2048, 2732), 264., 18., TOP_CAMERA),
    tablet("ipad-9", "iPad (9th generation)", (174.1, 250.6), (1620, 2160), 264., 0., Cutout::Home),
];

/// Earlier names for models, still read from saved designs.
const ALIASES: &[(&str, &str)] = &[
    ("iphone-island", "iphone-18-pro-max"),
    ("iphone-notch", "iphone-14"),
    ("iphone-classic", "iphone-se"),
    ("ipad-pro", "ipad-pro-13"),
    ("ipad-classic", "ipad-9"),
];

/// The metal band visible around the front glass, in millimetres.
const EDGE_MM: f32 = 0.8;

/// A device model from the catalog. Saved by its slug.
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct DeviceModel(u16);

impl std::fmt::Debug for DeviceModel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.slug())
    }
}

impl Default for DeviceModel {
    fn default() -> Self {
        Target::Iphone.default_model()
    }
}

impl Serialize for DeviceModel {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(self.slug())
    }
}

impl<'de> Deserialize<'de> for DeviceModel {
    /// A model this version doesn't know becomes the default phone, so a
    /// design from a newer AppDoctor still opens.
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let slug = String::deserialize(deserializer)?;
        Ok(DeviceModel::from_slug(&slug).unwrap_or_default())
    }
}

impl DeviceModel {
    /// Every model, iPhones then iPads, newest first.
    pub fn all() -> impl Iterator<Item = DeviceModel> {
        (0..CATALOG.len() as u16).map(DeviceModel)
    }

    pub fn models_for(target: Target) -> impl Iterator<Item = DeviceModel> {
        DeviceModel::all().filter(move |model| model.target() == target)
    }

    /// A model by its slug, or by a name earlier versions used.
    pub fn from_slug(slug: &str) -> Option<DeviceModel> {
        let slug = ALIASES
            .iter()
            .find(|(old, _)| *old == slug)
            .map_or(slug, |(_, new)| *new);
        CATALOG.iter().position(|spec| spec.slug == slug).map(|ix| DeviceModel(ix as u16))
    }

    fn spec(self) -> &'static Spec {
        &CATALOG[self.0 as usize]
    }

    /// The name used in saved designs and by the MCP tools.
    pub fn slug(self) -> &'static str {
        self.spec().slug
    }

    pub fn label(self) -> &'static str {
        self.spec().label
    }

    /// iPhone or iPad.
    pub fn target(self) -> Target {
        self.spec().target
    }

    pub fn has_home_button(self) -> bool {
        self.spec().cutout == Cutout::Home
    }

    /// Screen height / width: the shape a screenshot is cropped to.
    pub fn screen_aspect(self) -> f32 {
        let (w, h) = self.spec().pixels;
        h as f32 / w as f32
    }

    /// Body height / width.
    pub fn aspect(self) -> f32 {
        let (w, h) = self.spec().body;
        h / w
    }

    /// The screen's size in millimetres, from its pixels and density.
    fn screen_mm(self) -> (f32, f32) {
        let spec = self.spec();
        let mm = |px: u32| px as f32 / spec.ppi * 25.4;
        (mm(spec.pixels.0), mm(spec.pixels.1))
    }
}

/// The device's finish: the colour of its edge and buttons, or no frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Frame {
    Black,
    Silver,
    Titanium,
    Gold,
    Blue,
    Orange,
    Pink,
    /// Matte white all over, front included, like a clay mock-up.
    Clay,
    /// The screenshot with rounded corners, no device.
    None,
}

impl Frame {
    pub const ALL: [Frame; 9] = [
        Frame::Black,
        Frame::Silver,
        Frame::Titanium,
        Frame::Gold,
        Frame::Blue,
        Frame::Orange,
        Frame::Pink,
        Frame::Clay,
        Frame::None,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Frame::Black => "Black",
            Frame::Silver => "Silver",
            Frame::Titanium => "Titanium",
            Frame::Gold => "Gold",
            Frame::Blue => "Blue",
            Frame::Orange => "Orange",
            Frame::Pink => "Pink",
            Frame::Clay => "Clay",
            Frame::None => "No frame",
        }
    }

    pub fn slug(self) -> &'static str {
        match self {
            Frame::Black => "black",
            Frame::Silver => "silver",
            Frame::Titanium => "titanium",
            Frame::Gold => "gold",
            Frame::Blue => "blue",
            Frame::Orange => "orange",
            Frame::Pink => "pink",
            Frame::Clay => "clay",
            Frame::None => "none",
        }
    }

    pub fn from_slug(slug: &str) -> Option<Frame> {
        Frame::ALL.into_iter().find(|frame| frame.slug() == slug)
    }

    /// The metal edge and buttons; `None` for no frame.
    pub fn edge_color(self) -> Option<&'static str> {
        Some(match self {
            Frame::Black => "#3a3a3d",
            Frame::Silver => "#d7d8dc",
            Frame::Titanium => "#bdb7ad",
            Frame::Gold => "#e3cfac",
            Frame::Blue => "#3a475e",
            Frame::Orange => "#e0692c",
            Frame::Pink => "#efc3cc",
            Frame::Clay => "#e9e6e1",
            Frame::None => return None,
        })
    }

    /// The buttons: a shade darker than the edge, as they catch less light.
    pub fn button_color(self) -> Option<&'static str> {
        Some(match self {
            Frame::Black => "#2a2a2c",
            Frame::Silver => "#babcc2",
            Frame::Titanium => "#a49e94",
            Frame::Gold => "#cdb994",
            Frame::Blue => "#2c374a",
            Frame::Orange => "#c45722",
            Frame::Pink => "#d9aab4",
            Frame::Clay => "#d6d2cc",
            Frame::None => return None,
        })
    }

    /// The front glass around the screen. Black, except a clay finish, and
    /// light finishes on home-button models, which had white fronts.
    pub fn glass_color(self, model: DeviceModel) -> &'static str {
        let classic = model.has_home_button();
        match self {
            Frame::Clay => "#e9e6e1",
            Frame::Silver | Frame::Gold | Frame::Pink | Frame::Titanium if classic => "#f4f4f2",
            _ => "#0b0b0d",
        }
    }
}

/// Where a device layer's parts go, for both the live canvas and export.
#[derive(Debug, Clone, PartialEq)]
pub struct DeviceGeometry {
    pub body: Rect,
    pub body_radius: f32,
    /// The front glass inside the metal edge.
    pub glass: Rect,
    pub glass_radius: f32,
    pub screen: Rect,
    pub screen_radius: f32,
    /// The Dynamic Island.
    pub island: Option<Rect>,
    /// The notch, hanging into the top of the screen. Its bottom corners
    /// round by [`DeviceGeometry::notch_radius`].
    pub notch: Option<Rect>,
    pub notch_radius: f32,
    /// A circle.
    pub home_button: Option<Rect>,
    /// A pill above the screen of home-button iPhones.
    pub speaker: Option<Rect>,
    /// A circle, in the bezel or (iPhone Duo) in the screen.
    pub camera: Option<Rect>,
    /// A foldable's crease: a hairline down the screen.
    pub crease: Option<Rect>,
    /// Side buttons, drawn behind the body and poking out past its edges.
    pub buttons: Vec<Rect>,
}

pub fn device_geometry(layer: &Layer, model: DeviceModel, frame: Frame) -> DeviceGeometry {
    let spec = model.spec();
    let body = layer.rect();
    // Canvas pixels per millimetre.
    let k = layer.width / spec.body.0;
    let (screen_w, screen_h) = model.screen_mm();
    let mm_per_pt = 25.4 / spec.ppi * spec.scale;
    let screen_radius_mm = spec.radius_pt * mm_per_pt;

    if frame == Frame::None {
        let radius = if screen_radius_mm > 0. {
            screen_radius_mm * k
        } else {
            layer.width * 0.04
        };
        return DeviceGeometry {
            body,
            body_radius: radius,
            glass: body,
            glass_radius: radius,
            screen: body,
            screen_radius: radius,
            island: None,
            notch: None,
            notch_radius: 0.,
            home_button: None,
            speaker: None,
            camera: None,
            crease: None,
            buttons: Vec::new(),
        };
    }

    // Bezels from the difference between body and screen; the screen stays
    // centred even when the layer's height was typed in by hand.
    let side_mm = ((spec.body.0 - screen_w) / 2.).max(EDGE_MM + 0.5);
    let top_mm = ((spec.body.1 - screen_h) / 2.).max(EDGE_MM + 0.5);
    let edge = EDGE_MM * k;
    let glass = inset(body, edge, edge);
    let screen = inset(body, side_mm * k, top_mm * k);
    let screen_radius = screen_radius_mm * k;
    // Concentric with the screen's corners; home-button phones are rounder.
    let body_radius = if screen_radius_mm > 0. {
        (screen_radius_mm + side_mm) * k
    } else {
        layer.width * if spec.target == Target::Ipad { 0.07 } else { 0.13 }
    };
    let glass_radius = (body_radius - edge).max(0.);
    let centre_x = body.x + body.width / 2.;
    let top_band = (glass.y + screen.y) / 2.;
    let bottom_band = (screen.y + screen.height + glass.y + glass.height) / 2.;
    let circle = |cx: f32, cy: f32, diameter: f32| Rect {
        x: cx - diameter / 2.,
        y: cy - diameter / 2.,
        width: diameter,
        height: diameter,
    };

    let mut geometry = DeviceGeometry {
        body,
        body_radius,
        glass,
        glass_radius,
        screen,
        screen_radius,
        island: None,
        notch: None,
        notch_radius: 0.,
        home_button: None,
        speaker: None,
        camera: None,
        crease: None,
        buttons: Vec::new(),
    };
    match spec.cutout {
        Cutout::Island => {
            let (width, height) = (20.5 * k, 6.1 * k);
            geometry.island = Some(Rect {
                x: centre_x - width / 2.,
                y: screen.y + 1.8 * k,
                width,
                height,
            });
        }
        Cutout::Notch(width_mm) => {
            // Starts inside the bezel so it reads as part of it.
            let width = width_mm * k;
            let top = screen.y - (top_mm - EDGE_MM) * k * 0.5;
            geometry.notch = Some(Rect {
                x: centre_x - width / 2.,
                y: top,
                width,
                height: screen.y - top + 5.2 * k,
            });
            geometry.notch_radius = 1.8 * k;
        }
        Cutout::Punch => {
            geometry.camera = Some(circle(screen.x + screen.width / 2., screen.y + 4. * k, 3.2 * k));
        }
        Cutout::Camera { side: false } => {
            geometry.camera = Some(circle(centre_x, top_band, 2.6 * k));
        }
        Cutout::Camera { side: true } => {
            // On the long edge, which is the right side when held upright.
            geometry.camera = Some(circle(
                (screen.x + screen.width + glass.x + glass.width) / 2.,
                body.y + body.height / 2.,
                2.6 * k,
            ));
        }
        Cutout::Home if spec.target == Target::Iphone => {
            let (width, height) = (11. * k, 1.2 * k);
            geometry.speaker = Some(Rect {
                x: centre_x - width / 2.,
                y: top_band - height / 2.,
                width,
                height,
            });
            geometry.camera = Some(circle(centre_x - 10. * k, top_band, 2.4 * k));
            geometry.home_button = Some(circle(centre_x, bottom_band, 11. * k));
        }
        Cutout::Home => {
            geometry.camera = Some(circle(centre_x, top_band, 2.8 * k));
            geometry.home_button = Some(circle(centre_x, bottom_band, 12.5 * k));
        }
    }
    if spec.crease {
        geometry.crease = Some(Rect {
            x: screen.x + screen.width / 2. - 0.25 * k,
            y: screen.y,
            width: 0.5 * k,
            height: screen.height,
        });
    }
    geometry.buttons = buttons(spec.buttons, body, k);
    geometry
}

fn inset(rect: Rect, dx: f32, dy: f32) -> Rect {
    Rect {
        x: rect.x + dx,
        y: rect.y + dy,
        width: (rect.width - dx * 2.).max(0.),
        height: (rect.height - dy * 2.).max(0.),
    }
}

/// The side buttons: (side, start, length) as shares of the body's height
/// (left and right edges) or width (the top edge). `k` is pixels per mm.
fn buttons(layout: Buttons, body: Rect, k: f32) -> Vec<Rect> {
    enum Side {
        Left,
        Right,
        Top,
    }
    use Side::*;
    let (thickness_mm, layout): (f32, &[(Side, f32, f32)]) = match layout {
        Buttons::Iphone => (
            0.85,
            &[(Left, 0.155, 0.04), (Left, 0.225, 0.07), (Left, 0.31, 0.07), (Right, 0.255, 0.11)],
        ),
        Buttons::IphoneClassic => (
            0.85,
            &[(Left, 0.13, 0.03), (Left, 0.19, 0.055), (Left, 0.26, 0.055), (Right, 0.19, 0.065)],
        ),
        Buttons::Ipad => (1.4, &[(Top, 0.8, 0.08), (Right, 0.06, 0.05), (Right, 0.125, 0.05)]),
    };
    let (thick, out) = (thickness_mm * k, thickness_mm * k * 0.6);
    layout
        .iter()
        .map(|(side, start, length)| match side {
            Left => Rect {
                x: body.x - out,
                y: body.y + body.height * start,
                width: thick,
                height: body.height * length,
            },
            Right => Rect {
                x: body.x + body.width + out - thick,
                y: body.y + body.height * start,
                width: thick,
                height: body.height * length,
            },
            Top => Rect {
                x: body.x + body.width * start,
                y: body.y - out,
                width: body.width * length,
                height: thick,
            },
        })
        .collect()
}
