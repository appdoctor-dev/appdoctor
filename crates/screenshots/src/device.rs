//! Device frames: the models a screenshot can sit in, their finishes, and
//! where each part goes. The editor's canvas and the SVG export both draw
//! from [`device_geometry`], so they agree.
//!
//! Every frame is drawn from proportions (no Apple artwork), so they can
//! ship with the app.

use serde::{Deserialize, Serialize};

use crate::{Layer, Rect, Target};

/// The shape of a device.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum DeviceModel {
    /// iPhone 14 Pro and later: thin bezels and the Dynamic Island.
    #[default]
    IphoneIsland,
    /// iPhone X to 14: the notch.
    IphoneNotch,
    /// iPhone SE and 8: a home button below a 16:9 screen.
    IphoneClassic,
    /// iPad Pro and Air: even, thin bezels.
    IpadPro,
    /// iPad with a home button.
    IpadClassic,
}

/// A model's proportions, as shares of the body's width.
struct Spec {
    /// Screen height / width.
    screen_aspect: f32,
    /// The metal band around the front glass.
    edge: f32,
    /// Glass between the edge and the screen, on every side.
    bezel: f32,
    /// Extra glass above and below the screen, for the home button.
    chin: f32,
    body_radius: f32,
    /// 0 for the square screens of home-button models.
    screen_radius: f32,
}

impl DeviceModel {
    pub const ALL: [DeviceModel; 5] = [
        DeviceModel::IphoneIsland,
        DeviceModel::IphoneNotch,
        DeviceModel::IphoneClassic,
        DeviceModel::IpadPro,
        DeviceModel::IpadClassic,
    ];

    pub fn label(self) -> &'static str {
        match self {
            DeviceModel::IphoneIsland => "iPhone with Dynamic Island",
            DeviceModel::IphoneNotch => "iPhone with notch",
            DeviceModel::IphoneClassic => "iPhone with home button",
            DeviceModel::IpadPro => "iPad Pro",
            DeviceModel::IpadClassic => "iPad with home button",
        }
    }

    /// The word for it in a segmented control.
    pub fn short_label(self) -> &'static str {
        match self {
            DeviceModel::IphoneIsland => "Island",
            DeviceModel::IphoneNotch => "Notch",
            DeviceModel::IphoneClassic | DeviceModel::IpadClassic => "Classic",
            DeviceModel::IpadPro => "Pro",
        }
    }

    /// The name used in saved designs and by the MCP tools.
    pub fn slug(self) -> &'static str {
        match self {
            DeviceModel::IphoneIsland => "iphone-island",
            DeviceModel::IphoneNotch => "iphone-notch",
            DeviceModel::IphoneClassic => "iphone-classic",
            DeviceModel::IpadPro => "ipad-pro",
            DeviceModel::IpadClassic => "ipad-classic",
        }
    }

    pub fn from_slug(slug: &str) -> Option<DeviceModel> {
        DeviceModel::ALL.into_iter().find(|model| model.slug() == slug)
    }

    pub fn target(self) -> Target {
        match self {
            DeviceModel::IphoneIsland | DeviceModel::IphoneNotch | DeviceModel::IphoneClassic => Target::Iphone,
            DeviceModel::IpadPro | DeviceModel::IpadClassic => Target::Ipad,
        }
    }

    pub fn models_for(target: Target) -> impl Iterator<Item = DeviceModel> {
        DeviceModel::ALL.into_iter().filter(move |model| model.target() == target)
    }

    fn spec(self) -> Spec {
        match self {
            DeviceModel::IphoneIsland => Spec {
                screen_aspect: 2.173,
                edge: 0.011,
                bezel: 0.023,
                chin: 0.,
                body_radius: 0.135,
                screen_radius: 0.105,
            },
            DeviceModel::IphoneNotch => Spec {
                screen_aspect: 2.164,
                edge: 0.012,
                bezel: 0.026,
                chin: 0.,
                body_radius: 0.13,
                screen_radius: 0.098,
            },
            DeviceModel::IphoneClassic => Spec {
                screen_aspect: 1.778,
                edge: 0.01,
                bezel: 0.055,
                chin: 0.19,
                body_radius: 0.13,
                screen_radius: 0.,
            },
            DeviceModel::IpadPro => Spec {
                screen_aspect: 1.3333,
                edge: 0.008,
                bezel: 0.034,
                chin: 0.,
                body_radius: 0.075,
                screen_radius: 0.045,
            },
            DeviceModel::IpadClassic => Spec {
                screen_aspect: 1.3333,
                edge: 0.008,
                bezel: 0.07,
                chin: 0.075,
                body_radius: 0.07,
                screen_radius: 0.,
            },
        }
    }

    /// Screen height / width: the screenshot's shape, or 16:9 for
    /// home-button iPhones (their screenshots are cropped to fit).
    pub fn screen_aspect(self) -> f32 {
        self.spec().screen_aspect
    }

    /// Body height / width, so the screen inside has [`Self::screen_aspect`].
    pub fn aspect(self) -> f32 {
        let spec = self.spec();
        let inset = spec.edge + spec.bezel;
        (1. - 2. * inset) * spec.screen_aspect + 2. * inset + 2. * spec.chin
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
        let classic = matches!(model, DeviceModel::IphoneClassic | DeviceModel::IpadClassic);
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
    /// A circle in the top bezel.
    pub camera: Option<Rect>,
    /// Side buttons, drawn behind the body and poking out past its edges.
    pub buttons: Vec<Rect>,
}

pub fn device_geometry(layer: &Layer, model: DeviceModel, frame: Frame) -> DeviceGeometry {
    let spec = model.spec();
    let w = layer.width;
    let body = layer.rect();

    if frame == Frame::None {
        let radius = if spec.screen_radius > 0. {
            spec.screen_radius * 1.15
        } else {
            0.04
        } * w;
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
            buttons: Vec::new(),
        };
    }

    let body_radius = spec.body_radius * w;
    let edge = spec.edge * w;
    let glass = inset(body, edge, edge);
    let glass_radius = (body_radius - edge).max(0.);
    let side = (spec.edge + spec.bezel) * w;
    let screen = inset(body, side, side + spec.chin * w);
    let screen_radius = spec.screen_radius * w;
    let centre_x = body.x + body.width / 2.;
    // Centre of the glass above and below the screen.
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
        buttons: Vec::new(),
    };
    match model {
        DeviceModel::IphoneIsland => {
            let (width, height) = (w * 0.29, w * 0.083);
            geometry.island = Some(Rect {
                x: centre_x - width / 2.,
                y: screen.y + w * 0.026,
                width,
                height,
            });
        }
        DeviceModel::IphoneNotch => {
            // Starts inside the bezel so it reads as part of it.
            let width = w * 0.5;
            let top = screen.y - spec.bezel * w * 0.5;
            geometry.notch = Some(Rect {
                x: centre_x - width / 2.,
                y: top,
                width,
                height: screen.y - top + w * 0.072,
            });
            geometry.notch_radius = w * 0.04;
        }
        DeviceModel::IphoneClassic => {
            let (width, height) = (w * 0.16, w * 0.016);
            geometry.speaker = Some(Rect {
                x: centre_x - width / 2.,
                y: top_band - height / 2.,
                width,
                height,
            });
            geometry.camera = Some(circle(centre_x - w * 0.15, top_band, w * 0.034));
            geometry.home_button = Some(circle(centre_x, bottom_band, w * 0.165));
        }
        DeviceModel::IpadPro => {
            geometry.camera = Some(circle(centre_x, top_band, w * 0.013));
        }
        DeviceModel::IpadClassic => {
            geometry.camera = Some(circle(centre_x, top_band, w * 0.016));
            geometry.home_button = Some(circle(centre_x, bottom_band, w * 0.072));
        }
    }
    geometry.buttons = buttons(model, body);
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
/// (left and right edges) or width (the top edge).
fn buttons(model: DeviceModel, body: Rect) -> Vec<Rect> {
    enum Side {
        Left,
        Right,
        Top,
    }
    use Side::*;
    let (thickness, layout): (f32, &[(Side, f32, f32)]) = match model {
        DeviceModel::IphoneIsland => (
            0.011,
            &[(Left, 0.155, 0.04), (Left, 0.225, 0.07), (Left, 0.31, 0.07), (Right, 0.255, 0.11)],
        ),
        DeviceModel::IphoneNotch => (
            0.011,
            &[(Left, 0.14, 0.035), (Left, 0.205, 0.07), (Left, 0.29, 0.07), (Right, 0.24, 0.11)],
        ),
        DeviceModel::IphoneClassic => (
            0.011,
            &[(Left, 0.13, 0.03), (Left, 0.19, 0.055), (Left, 0.26, 0.055), (Right, 0.19, 0.065)],
        ),
        DeviceModel::IpadPro => (0.007, &[(Top, 0.8, 0.08), (Right, 0.06, 0.05), (Right, 0.125, 0.05)]),
        DeviceModel::IpadClassic => (0.007, &[(Top, 0.8, 0.06), (Right, 0.17, 0.045), (Right, 0.23, 0.045)]),
    };
    let w = body.width;
    let (thick, out) = (w * thickness, w * thickness * 0.6);
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
                x: body.x + w * start,
                y: body.y - out,
                width: w * length,
                height: thick,
            },
        })
        .collect()
}
