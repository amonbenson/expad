//! What the LED below each jack shows: nothing while the jack is empty, a dim white while it
//! holds a plug with no known pedal behind it, and for a pedal it recognises, first the pedal's
//! own color for [`ANNOUNCEMENT_DURATION`], then the value it sends, from dark red at 0 through
//! red, orange and yellow to white at full scale.
//!
//! Colors are given at full scale; [`LedStrip`] scales them by the global brightness and the
//! strip's power limit. WS2812B green looks far brighter than red or blue at the same duty cycle,
//! so the mixed colors are weighted against it.

use embassy_rp::pio::Instance;
use embassy_time::{Duration, Instant};

use crate::hal::led::{LedStrip, RGB8};
use crate::topology::JackMode;

/// How long a newly recognised pedal's color is shown before its value.
const ANNOUNCEMENT_DURATION: Duration = Duration::from_secs(1);

const OFF: RGB8 = rgb(0x00, 0x00, 0x00);
const DIM_WHITE: RGB8 = rgb(0x10, 0x10, 0x10);
const PURPLE: RGB8 = rgb(0x80, 0x00, 0xFF);
const LIME: RGB8 = rgb(0x60, 0xFF, 0x00);
const DARK_RED: RGB8 = rgb(0x40, 0x00, 0x00);
const DEEP_RED: RGB8 = rgb(0xFF, 0x00, 0x00);
const ORANGE: RGB8 = rgb(0xFF, 0x30, 0x00);
const YELLOW: RGB8 = rgb(0xFF, 0xA0, 0x00);
const WHITE: RGB8 = rgb(0xFF, 0xFF, 0xFF);

/// Colors a pedal's value runs through from 0 to full scale, evenly spaced.
const VALUE_GRADIENT: [RGB8; 5] = [DARK_RED, DEEP_RED, ORANGE, YELLOW, WHITE];

const fn rgb(r: u8, g: u8, b: u8) -> RGB8 {
    RGB8 { r, g, b }
}

/// A kind of pedal the jack monitor recognises and follows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Pedal {
    Expression,
    Footswitch,
    Rheostat,
}

impl Pedal {
    fn color(self) -> RGB8 {
        match self {
            Pedal::Expression => PURPLE,
            Pedal::Footswitch => LIME,
            Pedal::Rheostat => YELLOW,
        }
    }
}

/// What a jack holds, as far as its LED is concerned.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Contents {
    Nothing,
    /// A plug that is still being identified, has nothing conducting behind it, or leads to a
    /// network that is no known pedal.
    UnknownPlug,
    Pedal(Pedal),
}

impl From<JackMode> for Contents {
    fn from(mode: JackMode) -> Self {
        match mode {
            JackMode::Empty => Contents::Nothing,
            JackMode::Identifying | JackMode::Open | JackMode::Other => Contents::UnknownPlug,
            JackMode::Tracking => Contents::Pedal(Pedal::Expression),
            JackMode::Switch => Contents::Pedal(Pedal::Footswitch),
            JackMode::Rheostat => Contents::Pedal(Pedal::Rheostat),
        }
    }
}

/// The state behind one jack's LED.
#[derive(Debug, Clone, Copy)]
struct JackIndicator {
    contents: Contents,
    /// The pedal last announced since the plug went in. A pedal the monitor briefly loses
    /// track of and identifies again is not announced a second time.
    announced: Option<Pedal>,
    announcement_ends: Instant,
    /// Value last sent for the pedal, in `0.0..=1.0`.
    value: Option<f32>,
}

impl JackIndicator {
    const EMPTY: Self = Self {
        contents: Contents::Nothing,
        announced: None,
        announcement_ends: Instant::MIN,
        value: None,
    };

    fn show(&mut self, contents: Contents, value: Option<f32>, now: Instant) {
        match contents {
            Contents::Nothing => self.announced = None,
            Contents::Pedal(pedal) if self.announced != Some(pedal) => {
                self.announced = Some(pedal);
                self.announcement_ends = now + ANNOUNCEMENT_DURATION;
            }
            Contents::UnknownPlug | Contents::Pedal(_) => {}
        }
        self.contents = contents;
        self.value = value;
    }

    fn color(&self, now: Instant) -> RGB8 {
        match self.contents {
            Contents::Nothing => OFF,
            Contents::UnknownPlug => DIM_WHITE,
            Contents::Pedal(pedal) if now < self.announcement_ends => pedal.color(),
            Contents::Pedal(pedal) => self.value.map_or(pedal.color(), value_color),
        }
    }
}

/// Where `value` in `0.0..=1.0` falls on [`VALUE_GRADIENT`].
fn value_color(value: f32) -> RGB8 {
    let last_segment = VALUE_GRADIENT.len() - 2;
    let position = value.clamp(0.0, 1.0) * (VALUE_GRADIENT.len() - 1) as f32;
    let segment = (position as usize).min(last_segment);
    let fraction = position - segment as f32;

    blend(
        VALUE_GRADIENT[segment],
        VALUE_GRADIENT[segment + 1],
        fraction,
    )
}

/// Mixes `from` and `to`, `fraction` of the way from one to the other.
fn blend(from: RGB8, to: RGB8, fraction: f32) -> RGB8 {
    let channel = |from: u8, to: u8| (from as f32 + (to as f32 - from as f32) * fraction) as u8;
    rgb(
        channel(from.r, to.r),
        channel(from.g, to.g),
        channel(from.b, to.b),
    )
}

/// The LED strip below the jacks, one LED per jack, each showing what its jack holds.
pub struct JackIndicators<'d, P: Instance, const N: usize> {
    leds: LedStrip<'d, P, N>,
    jacks: [JackIndicator; N],
}

impl<'d, P: Instance, const N: usize> JackIndicators<'d, P, N> {
    pub fn new(leds: LedStrip<'d, P, N>) -> Self {
        Self {
            leds,
            jacks: [JackIndicator::EMPTY; N],
        }
    }

    /// Tells the jack's LED what the monitor currently makes of the jack and the value last
    /// sent for it, in `0.0..=1.0`. A pedal it had not announced since the plug went in is
    /// announced from `now`.
    pub fn show(&mut self, jack: usize, mode: JackMode, value: Option<f32>, now: Instant) {
        self.jacks[jack].show(mode.into(), value, now);
    }

    /// Sets the global brightness every color is scaled by (see [`LedStrip::set_brightness`]).
    pub fn set_brightness(&mut self, brightness: u8) {
        self.leds.set_brightness(brightness);
    }

    /// Writes every jack's color as of `now` to the strip.
    pub async fn update(&mut self, now: Instant) {
        for (index, jack) in self.jacks.iter().enumerate() {
            self.leds.set_color(index, jack.color(now));
        }
        self.leds.update().await;
    }
}
