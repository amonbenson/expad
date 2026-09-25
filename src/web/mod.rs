mod interface;
mod server;

pub use interface::{
    ArmPull, Color, INTERFACE, JACK_COUNT, JackSettings, JackStatus, Settings, Status, WiperContact,
};
pub use server::spawn_web_server;
