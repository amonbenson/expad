use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::watch::Watch;
use serde::{Deserialize, Serialize};

use crate::hal::buf::TriState;
use crate::topology::ArmResistances;

/// Expression pedal jacks on the board (J2-J5).
pub const JACK_COUNT: usize = 4;

/// Receivers each shared value supports: one for the firmware plus one per browser session.
const MAX_RECEIVERS: usize = super::server::MAX_SESSIONS + 1;

pub type SharedValue<T> = Watch<CriticalSectionRawMutex, T, MAX_RECEIVERS>;

/// Rail an arm is driven to while it is measured, mirroring [`TriState`] for the web protocol.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum ArmPull {
    Up,
    Down,
    Floating,
}

impl From<TriState> for ArmPull {
    fn from(state: TriState) -> Self {
        match state {
            TriState::High => ArmPull::Up,
            TriState::Low => ArmPull::Down,
            TriState::HiZ => ArmPull::Floating,
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize)]
pub struct JackStatus {
    /// Expression value in `0.0..=1.0` that is sent to the host.
    pub value: f32,
    pub resistances: ArmResistances,
    /// Voltage measured at each arm's tap, in volts.
    pub voltages: [f32; 3],
    /// Rail each arm is driven to while measuring.
    pub pulls: [ArmPull; 3],
}

impl JackStatus {
    pub const DISCONNECTED: Self = Self {
        value: 0.0,
        resistances: ArmResistances::DISCONNECTED,
        voltages: [f32::NAN; 3],
        pulls: [ArmPull::Floating; 3],
    };
}

/// Live measurements the firmware reports to the web interface.
#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Status {
    pub uptime_seconds: u64,
    pub jacks: [JackStatus; JACK_COUNT],
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, defmt::Format)]
#[serde(rename_all = "camelCase")]
pub struct JackSettings {
    /// Zero-based MIDI channel (`0..=15`).
    pub midi_channel: u8,
    /// MIDI control change number (`0..=127`).
    pub midi_controller: u8,
    pub inverted: bool,
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
        led_brightness: u8::MAX / 10,
        jacks: [JackSettings {
            midi_channel: 0,
            midi_controller: Self::EXPRESSION_CONTROLLER,
            inverted: false,
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
