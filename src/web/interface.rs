use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::watch::Watch;
use serde::{Deserialize, Serialize};

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
    pub uptime_seconds: u64,
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
}

impl JackSettings {
    /// The expression value a wiper `position` stands for: stretched from the jack's range
    /// to `0.0..=1.0` and inverted if the jack is.
    pub fn value(&self, position: f32) -> f32 {
        let range = self.maximum - self.minimum;
        let stretched = if range > 0.0 {
            (position - self.minimum) / range
        } else {
            position
        };
        let value = stretched.clamp(0.0, 1.0);

        if self.inverted { 1.0 - value } else { value }
    }
}

/// User-tweakable configuration, editable from the web interface.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, defmt::Format)]
#[serde(rename_all = "camelCase")]
pub struct Settings {
    pub led_brightness: u8,
    pub jacks: [JackSettings; JACK_COUNT],
}

impl Settings {
    /// MIDI CC 11 is the standard "Expression" controller.
    const EXPRESSION_CONTROLLER: u8 = 11;

    pub const DEFAULT: Self = Self {
        led_brightness: u8::MAX / 2,
        jacks: [JackSettings {
            midi_channel: 0,
            midi_controller: Self::EXPRESSION_CONTROLLER,
            inverted: false,
            minimum: 0.0,
            maximum: 1.0,
            wiper: WiperContact::Auto,
        }; JACK_COUNT],
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
