#![no_std]

//! Identifies the three arm resistances of a star-connected resistor network from tap
//! voltages alone, by driving pairs of arms to the high and low rail while the third one
//! floats.
//!
//! This crate is the arithmetic half of the topology solver and knows nothing about ADCs
//! or pull switches: [`SolveSequence`] hands out the pair to measure next,
//! [`PairMeasurement::from_voltages`] turns the voltages that pair produced into a
//! measurement, and the sequence resolves them into [`ArmResistances`]. The hardware half
//! lives in `expad::topology::solver`, which drives the real pull switches and ADC chain; tests
//! drive the same sequence from a simulated network instead.
//!
//! All resistances are in kΩ and all currents in mA throughout the crate (kΩ × mA = V, so
//! voltage formulas need no unit conversion).
//!
//! The first arm stays driven high for every measurement while the other two take turns
//! being driven low, which halves the number of output changes compared to rotating
//! through every pair. Both measurements therefore share the first arm, giving it two
//! independent resistance estimates that must agree; the other two arms get one each. A
//! third measurement, between the second and third arms, is only needed to tell "the first
//! arm is isolated" apart from "nothing is connected at all", since neither of the first
//! two measurements conducts in either case.

mod config;
mod measurement;
mod monitor;
mod resistances;
mod resolve;
mod sequence;

pub use config::SolverConfig;
pub use measurement::{ArmDrive, PairMeasurement, PairVoltages};
pub use monitor::{
    CONTACT_COUNT, Drive, JackMode, JackMonitor, JackReport, MonitorConfig, Reading, TIP,
    TIP_SWITCH,
};
pub use resistances::{ARM_COUNT, ArmResistances, GROUNDED_END, Potentiometer, track_ends};
pub use sequence::{PAIR_SEQUENCE, SolveError, SolveSequence, SolveStep};
