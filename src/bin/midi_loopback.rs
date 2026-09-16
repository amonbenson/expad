#![no_std]
#![no_main]

use defmt::{Debug2Format, info, warn};
use embassy_executor::Spawner;
use embassy_futures::join::join;
use embassy_rp::bind_interrupts;
use embassy_rp::peripherals::USB;
use embassy_rp::usb::InterruptHandler;
use expad::hal::usb::{EndpointError, MAX_PACKET_SIZE, Message, UsbMidi, UsbMidiConfig};

use {defmt_rtt as _, panic_probe as _};

bind_interrupts!(struct Irqs {
    USBCTRL_IRQ => InterruptHandler<USB>;
});

#[unsafe(link_section = ".bi_entries")]
#[used]
pub static PICOTOOL_ENTRIES: [embassy_rp::binary_info::EntryAddr; 4] = [
    embassy_rp::binary_info::rp_program_name!(c"MIDI Loopback"),
    embassy_rp::binary_info::rp_program_description!(
        c"Echoes every MIDI packet received over USB back to the host"
    ),
    embassy_rp::binary_info::rp_cargo_version!(),
    embassy_rp::binary_info::rp_program_build_attribute!(),
];

async fn echo_packets(midi: &mut UsbMidi) -> Result<(), EndpointError> {
    let mut buffer = [0; MAX_PACKET_SIZE];
    loop {
        for packet in midi.receive(&mut buffer).await?.flatten() {
            info!("echoing {}", Debug2Format(&Message::try_from(&packet)));
            midi.send_packet(&packet).await?;
        }
    }
}

#[embassy_executor::main]
async fn main(_spawner: Spawner) {
    let p = embassy_rp::init(Default::default());

    info!("Initializing USB MIDI");
    let (mut midi, mut device) = UsbMidi::new(p.USB, Irqs, UsbMidiConfig::default());

    let loopback = async {
        loop {
            midi.wait_connection().await;
            info!("USB MIDI connected");
            let error = echo_packets(&mut midi).await.unwrap_err();
            warn!("USB MIDI disconnected: {}", error);
        }
    };
    join(device.run(), loopback).await;
}
