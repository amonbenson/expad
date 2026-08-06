#![no_std]
#![no_main]


use defmt::info;
use embassy_executor::Spawner;
use embassy_time::Timer;
use crate::sr::ShiftRegisterChain;
use crate::quadbuf::{QuadBufferChain, TriState};

use {defmt_rtt as _, panic_probe as _};

mod sr;
mod quadbuf;

const CHANNELS: usize = 1;

#[unsafe(link_section = ".bi_entries")]
#[used]
pub static PICOTOOL_ENTRIES: [embassy_rp::binary_info::EntryAddr; 4] = [
    embassy_rp::binary_info::rp_program_name!(c"Blinky Example"),
    embassy_rp::binary_info::rp_program_description!(
        c"This example tests the RP Pico on board LED, connected to gpio 25"
    ),
    embassy_rp::binary_info::rp_cargo_version!(),
    embassy_rp::binary_info::rp_program_build_attribute!(),
];

#[embassy_executor::main]
async fn main(_spawner: Spawner) {
    let p = embassy_rp::init(Default::default());

    let sr = ShiftRegisterChain::<CHANNELS>::new(
        p.SPI1,
        p.PIN_14,
        p.PIN_15,
        p.PIN_11,
        p.PIN_13,
    );
    let mut buffers = QuadBufferChain::new(sr);
    buffers.clear().unwrap();

    info!("Starting loop");
    loop {
        buffers.set_output(0, 0, TriState::Low);
        buffers.set_output(0, 1, TriState::High);
        buffers.set_output(0, 2, TriState::HiZ);
        buffers.update().unwrap();
        Timer::after_millis(10).await;

        buffers.set_output(0, 0, TriState::HiZ);
        buffers.set_output(0, 1, TriState::High);
        buffers.set_output(0, 2, TriState::Low);
        buffers.update().unwrap();
        Timer::after_millis(10).await;
    }
}
