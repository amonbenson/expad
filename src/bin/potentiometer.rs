#![no_std]
#![no_main]

use defmt::info;
use embassy_executor::Spawner;
use embassy_rp::bind_interrupts;
use embassy_rp::peripherals::PIO0;
use embassy_rp::pio::InterruptHandler;
use expad::hal::adc::{AdcChain, AdcChainConfig};
use expad::hal::buf::{QuadBufferChain, ShiftRegisterChain, TriState};
use expad::hal::led::{LedStrip, RGB8, Ws2812Chain};

use {defmt_rtt as _, panic_probe as _};

const CHANNELS: usize = 1;
const LED_COUNT: usize = 8;

/// Buffer output (and matching ADC input) pulled to the low rail.
const LOW_CHANNEL: u8 = 1;
/// Buffer output (and matching ADC input) left floating on the potentiometer wiper.
const WIPER_CHANNEL: u8 = 2;
/// Buffer output (and matching ADC input) pulled to the high rail.
const HIGH_CHANNEL: u8 = 3;

bind_interrupts!(struct Irqs {
    PIO0_IRQ_0 => InterruptHandler<PIO0>;
});

#[unsafe(link_section = ".bi_entries")]
#[used]
pub static PICOTOOL_ENTRIES: [embassy_rp::binary_info::EntryAddr; 4] = [
    embassy_rp::binary_info::rp_program_name!(c"Potentiometer"),
    embassy_rp::binary_info::rp_program_description!(
        c"Measures a potentiometer wiper between buffered low/high rails and shows its position on the LED strip"
    ),
    embassy_rp::binary_info::rp_cargo_version!(),
    embassy_rp::binary_info::rp_program_build_attribute!(),
];

fn indicator_color() -> RGB8 {
    (0, 128, 255).into()
}

/// Scales an RGB8 color by an intensity factor in `0.0..=1.0`.
fn scale_color(color: RGB8, factor: f32) -> RGB8 {
    let factor = factor.clamp(0.0, 1.0);
    RGB8::new(
        (color.r as f32 * factor) as u8,
        (color.g as f32 * factor) as u8,
        (color.b as f32 * factor) as u8,
    )
}

/// Lights the two LEDs adjacent to `position` (`0.0..=1.0` across the strip),
/// splitting brightness between them in proportion to how close `position` is
/// to each, so the indicator interpolates smoothly as the wiper turns.
fn show_position(leds: &mut LedStrip<'_, PIO0, LED_COUNT>, position: f32) {
    let scaled_position = position.clamp(0.0001, 0.9999) * (LED_COUNT - 1) as f32;
    let low_index = scaled_position as usize;
    let high_index = (low_index + 1).min(LED_COUNT - 1);
    let high_weight = scaled_position - low_index as f32;

    for index in 0..LED_COUNT {
        leds.set_color(index, RGB8::default());
    }
    leds.set_color(low_index, scale_color(indicator_color(), 1.0 - high_weight));
    leds.set_color(high_index, scale_color(indicator_color(), high_weight));
}

#[embassy_executor::main]
async fn main(_spawner: Spawner) {
    let p = embassy_rp::init(Default::default());

    info!("Initializing tristate buffers");
    let sr = ShiftRegisterChain::<CHANNELS>::new(p.SPI1, p.PIN_14, p.PIN_15, p.PIN_11, p.PIN_13);
    let mut buffers = QuadBufferChain::new(sr);
    buffers.clear().unwrap();
    buffers.set_output(0, LOW_CHANNEL, TriState::Low);
    buffers.set_output(0, HIGH_CHANNEL, TriState::High);
    buffers.update().unwrap();

    info!("Initializing ADCs");
    let mut adcs =
        AdcChain::<CHANNELS>::new(p.SPI0, p.PIN_18, p.PIN_19, p.PIN_16, [p.PIN_17], [p.PIN_21]);
    adcs.init(AdcChainConfig::default()).await.unwrap();

    info!("Initializing WS2812B strip");
    let chain = Ws2812Chain::<_, LED_COUNT>::new(p.PIO0, Irqs, p.DMA_CH0, p.PIN_6);
    let mut leds = LedStrip::new(chain);

    info!("Measuring potentiometer wiper position");
    loop {
        let low = adcs.measure_channel(0, LOW_CHANNEL).await.unwrap();
        let wiper = adcs.measure_channel(0, WIPER_CHANNEL).await.unwrap();
        let high = adcs.measure_channel(0, HIGH_CHANNEL).await.unwrap();
        let position = (wiper - low) / (high - low);

        info!(
            "wiper voltage: {}V (low {}V, high {}V, position {}%)",
            wiper,
            low,
            high,
            position * 100.0
        );

        // The LED strip is wired in the opposite direction to the wiper, so invert
        // the position before mapping it onto LED indices.
        show_position(&mut leds, 1.0 - position);
        leds.update().await;
    }
}
