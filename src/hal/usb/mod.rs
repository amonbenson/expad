mod midi;

pub use embassy_usb::driver::EndpointError;
pub use midi::{UsbMidi, UsbMidiConfig, UsbMidiDevice};
pub use usbd_midi::class::MAX_PACKET_SIZE;
pub use usbd_midi::{CableNumber, Message, UsbMidiEventPacket};
