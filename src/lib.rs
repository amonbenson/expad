#![no_std]
// picoserve's router and WebSocket futures nest deeper than the default limit of 128.
#![recursion_limit = "256"]

// `hal` and `topology` expose a broader register/API surface than any binary in
// src/bin/ currently drives; see AGENTS.md's Extensibility Hooks for the parts
// still awaiting callers.
#[allow(dead_code)]
pub mod hal;
#[allow(dead_code)]
pub mod topology;
#[cfg(feature = "web")]
pub mod web;
