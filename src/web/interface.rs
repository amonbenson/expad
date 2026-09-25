use core::fmt;

use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::watch::Watch;
use serde::de::{self, Visitor};
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::hal::led::RGB8;

use crate::topology::{ARM_COUNT, ArmResistances, Drive, JackMode};

pub use crate::board::JACK_COUNT;

/// Receivers each shared value supports: one for the firmware plus one per browser session.
const MAX_RECEIVERS: usize = super::server::MAX_SESSIONS + 1;

pub type SharedValue<T> = Watch<CriticalSectionRawMutex, T, MAX_RECEIVERS>;

/// Rail an arm is driven to while it is measured, mirroring [`Drive`] for the web protocol.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum ArmPull {
    Up,
    Down,
    Floating,
}

impl From<Drive> for ArmPull {
    fn from(drive: Drive) -> Self {
        match drive {
            Drive::High => ArmPull::Up,
            Drive::Low => ArmPull::Down,
            Drive::Floating => ArmPull::Floating,
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize)]
pub struct JackStatus {
    pub mode: JackMode,
    /// Wiper position in `0.0..=1.0` as measured, before the jack's range and inversion
    /// settings apply; `None` while no potentiometer is plugged in.
    pub position: Option<f32>,
    /// Expression value in `0.0..=1.0` that is sent to the host.
    pub value: f32,
    pub resistances: ArmResistances,
    /// Voltage measured at each arm's tap, in volts.
    pub voltages: [f32; ARM_COUNT],
    /// Rail each arm is driven to while measuring.
    pub pulls: [ArmPull; ARM_COUNT],
}

impl JackStatus {
    pub const DISCONNECTED: Self = Self {
        mode: JackMode::Empty,
        position: None,
        value: 0.0,
        resistances: ArmResistances::DISCONNECTED,
        voltages: [f32::NAN; ARM_COUNT],
        pulls: [ArmPull::Floating; ARM_COUNT],
    };
}

/// Live measurements the firmware reports to the web interface.
#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Status {
    pub jacks: [JackStatus; JACK_COUNT],
}

/// Which contact of a jack a potentiometer's wiper is on, for the one end stop where the
/// measurement cannot tell: ring and sleeve shorted together.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, defmt::Format)]
#[serde(rename_all = "camelCase")]
pub enum WiperContact {
    /// The wiper last seen in the jack, or the ring.
    Auto,
    Tip,
    Ring,
    Sleeve,
}

impl WiperContact {
    /// Arm index of the contact, `None` for [`Auto`](Self::Auto).
    pub fn arm(self) -> Option<usize> {
        match self {
            Self::Auto => None,
            Self::Tip => Some(0),
            Self::Ring => Some(1),
            Self::Sleeve => Some(2),
        }
    }
}

/// A jack's accent color, shown by its LED and in the web interface. Sent as `"#RRGGBB"`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, defmt::Format)]
pub struct Color {
    pub red: u8,
    pub green: u8,
    pub blue: u8,
}

impl Color {
    pub const fn new(red: u8, green: u8, blue: u8) -> Self {
        Self { red, green, blue }
    }

    /// Parses `"#RRGGBB"`, in either case.
    fn from_hex(text: &str) -> Option<Self> {
        let digits = text.strip_prefix('#')?;
        if digits.len() != 6 || !digits.bytes().all(|digit| digit.is_ascii_hexdigit()) {
            return None;
        }
        let channel = |start: usize| u8::from_str_radix(&digits[start..start + 2], 16).ok();

        Some(Self::new(channel(0)?, channel(2)?, channel(4)?))
    }

    /// Formats the color as `"#RRGGBB"`.
    fn to_hex(self) -> [u8; 7] {
        const DIGITS: &[u8; 16] = b"0123456789ABCDEF";
        let mut hex = [b'#'; 7];
        for (index, channel) in [self.red, self.green, self.blue].into_iter().enumerate() {
            hex[1 + 2 * index] = DIGITS[(channel >> 4) as usize];
            hex[2 + 2 * index] = DIGITS[(channel & 0x0F) as usize];
        }
        hex
    }
}

impl From<Color> for RGB8 {
    fn from(color: Color) -> Self {
        RGB8::new(color.red, color.green, color.blue)
    }
}

impl Serialize for Color {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let hex = self.to_hex();
        // Only ASCII digits and `#` are ever written.
        serializer.serialize_str(core::str::from_utf8(&hex).unwrap_or_default())
    }
}

impl<'de> Deserialize<'de> for Color {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct HexColor;

        impl Visitor<'_> for HexColor {
            type Value = Color;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("a color as \"#RRGGBB\"")
            }

            fn visit_str<E: de::Error>(self, text: &str) -> Result<Color, E> {
                Color::from_hex(text)
                    .ok_or_else(|| E::invalid_value(de::Unexpected::Str(text), &self))
            }
        }

        deserializer.deserialize_str(HexColor)
    }
}

/// Default accent of each jack, matching the web interface's first design.
pub const DEFAULT_JACK_COLORS: [Color; JACK_COUNT] = [
    Color::new(0xFF, 0x7E, 0x7E),
    Color::new(0xFF, 0xA2, 0x59),
    Color::new(0xFF, 0xCB, 0x56),
    Color::new(0xFF, 0xED, 0xB9),
];

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, defmt::Format)]
#[serde(rename_all = "camelCase")]
pub struct JackSettings {
    /// Zero-based MIDI channel (`0..=15`).
    pub midi_channel: u8,
    /// MIDI control change number (`0..=127`).
    pub midi_controller: u8,
    pub inverted: bool,
    /// Wiper positions the pedal's travel starts and ends at, in `0.0..=1.0`, which are
    /// stretched to the full expression range.
    pub minimum: f32,
    pub maximum: f32,
    pub wiper: WiperContact,
    /// Bend of the response curve in `-1.0..=1.0`, to counteract a pedal's nonlinear track:
    /// `0.0` is linear, positive values make the value rise early in the pedal's travel and
    /// negative ones late (see [`drive_curve`]).
    pub drive: f32,
    pub color: Color,
}

impl JackSettings {
    /// MIDI CC 11 is the standard "Expression" controller.
    const EXPRESSION_CONTROLLER: u8 = 11;

    pub const DEFAULT: Self = Self {
        midi_channel: 0,
        midi_controller: Self::EXPRESSION_CONTROLLER,
        inverted: false,
        minimum: 0.0,
        maximum: 1.0,
        wiper: WiperContact::Auto,
        drive: 0.0,
        color: DEFAULT_JACK_COLORS[0],
    };

    /// The expression value a wiper `position` stands for: stretched from the jack's range
    /// to `0.0..=1.0`, inverted if the jack is, then bent by its drive curve.
    pub fn value(&self, position: f32) -> f32 {
        let range = self.maximum - self.minimum;
        let stretched = if range > 0.0 {
            (position - self.minimum) / range
        } else {
            position
        };
        let value = stretched.clamp(0.0, 1.0);
        let travel = if self.inverted { 1.0 - value } else { value };

        drive_curve(travel, self.drive)
    }
}

/// How far the drive curve may bend at full drive: at `drive = ±1.0`, half the pedal's
/// travel gives `0.5 ± MAX_DRIVE_BEND / 2`.
const MAX_DRIVE_BEND: f32 = 0.9;

/// Bends `travel` in `0.0..=1.0` by `drive` in `-1.0..=1.0`, keeping both ends in place.
///
/// The curve is the rational bias curve `y = (1 + b) x / (1 - b + 2 b x)` with
/// `b = drive * MAX_DRIVE_BEND`. It maps half travel to `(1 + b) / 2`, so the knob reads as
/// where the pedal's midpoint lands, and `-drive` is the exact inverse of `drive`, so one knob
/// straightens both a logarithmic and an antilogarithmic track. It also needs no `powf`.
pub fn drive_curve(travel: f32, drive: f32) -> f32 {
    let bend = drive.clamp(-1.0, 1.0) * MAX_DRIVE_BEND;
    let travel = travel.clamp(0.0, 1.0);

    (1.0 + bend) * travel / (1.0 - bend + 2.0 * bend * travel)
}

/// User-tweakable configuration, editable from the web interface.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, defmt::Format)]
#[serde(rename_all = "camelCase")]
pub struct Settings {
    pub led_brightness: u8,
    pub jacks: [JackSettings; JACK_COUNT],
}

impl Settings {
    pub const DEFAULT: Self = {
        let mut jacks = [JackSettings::DEFAULT; JACK_COUNT];
        let mut jack = 0;
        while jack < JACK_COUNT {
            jacks[jack].color = DEFAULT_JACK_COLORS[jack];
            jack += 1;
        }

        Self {
            led_brightness: u8::MAX / 2,
            jacks,
        }
    };
}

impl Default for Settings {
    fn default() -> Self {
        Self::DEFAULT
    }
}

/// State shared between the firmware and every connected browser. The firmware publishes
/// `status` and reacts to `settings`; sending to `settings` from firmware updates all browsers.
pub struct Interface {
    pub status: SharedValue<Status>,
    pub settings: SharedValue<Settings>,
}

pub static INTERFACE: Interface = Interface {
    status: Watch::new(),
    settings: Watch::new_with(Settings::DEFAULT),
};
