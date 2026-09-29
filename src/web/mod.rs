mod interface;
mod server;

pub use interface::{
    ArmPull, Color, INTERFACE, JackSettings, JackStatus, Settings, Status, WiperContact,
};
pub use server::spawn_web_server;
