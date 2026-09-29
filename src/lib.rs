#![no_std]
// picoserve's router and WebSocket futures nest deeper than the default limit of 128.
#![recursion_limit = "256"]

pub mod board;

// `hal` and `topology` expose a broader register and driver surface (every AD7718 register
// field and mode, for instance) than the binaries in src/bin/ currently drive.
#[allow(dead_code)]
pub mod hal;
pub mod indicator;
#[allow(dead_code)]
pub mod topology;
#[cfg(feature = "web")]
pub mod web;
