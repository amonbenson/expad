#![no_std]
#![no_main]

use defmt::info;
use embassy_executor::Spawner;
use embassy_rp::bind_interrupts;
use embassy_rp::peripherals::{DMA_CH0, PIO0};
use embassy_rp::{dma, pio};
use embassy_time::{Duration, Ticker};
use expad::board::{self, LED_COUNT};
use expad::hal::led::RGB8;

use {defmt_rtt as _, panic_probe as _};

bind_interrupts!(struct Irqs {
    PIO0_IRQ_0 => pio::InterruptHandler<PIO0>;
    DMA_IRQ_0 => dma::InterruptHandler<DMA_CH0>;
});

#[unsafe(link_section = ".bi_entries")]
#[used]
pub static PICOTOOL_ENTRIES: [embassy_rp::binary_info::EntryAddr; 4] = [
    embassy_rp::binary_info::rp_program_name!(c"Rainbow"),
    embassy_rp::binary_info::rp_program_description!(
        c"Cycles a rainbow pattern across a WS2812B LED strip"
    ),
    embassy_rp::binary_info::rp_cargo_version!(),
    embassy_rp::binary_info::rp_program_build_attribute!(),
];

/// Maps a hue (0-255) to an RGB8 color, wrapping smoothly back to the start.
fn wheel(position: u8) -> RGB8 {
    let position = 255 - position;
    if position < 85 {
        (255 - position * 3, 0, position * 3).into()
    } else if position < 170 {
        let position = position - 85;
        (0, position * 3, 255 - position * 3).into()
    } else {
        let position = position - 170;
        (position * 3, 255 - position * 3, 0).into()
    }
}

#[embassy_executor::main]
async fn main(_spawner: Spawner) {
    let p = embassy_rp::init(Default::default());

    info!("Initializing WS2812B strip");
    let mut leds = board::leds(p.PIO0, Irqs, p.DMA_CH0, p.PIN_6);

    info!("Cycling rainbow pattern");
    let mut ticker = Ticker::every(Duration::from_millis(20));
    let mut offset: u8 = 0;
    loop {
        for index in 0..LED_COUNT {
            let hue = ((index * 256 / LED_COUNT) as u8).wrapping_add(offset);
            leds.set_color(index, wheel(hue));
        }

        leds.update().await;
        offset = offset.wrapping_add(1);
        ticker.next().await;
    }
}
