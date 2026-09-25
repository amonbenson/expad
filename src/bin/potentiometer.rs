#![no_std]
#![no_main]

use defmt::info;
use embassy_executor::Spawner;
use embassy_rp::bind_interrupts;
use embassy_rp::peripherals::{DMA_CH0, PIO0};
use embassy_rp::{dma, pio};
use expad::board::{self, Contact, JACKS, LED_COUNT};
use expad::hal::adc::AdcChainConfig;
use expad::hal::buf::TriState;
use expad::hal::led::{LedStrip, RGB8};

use {defmt_rtt as _, panic_probe as _};

/// Jack the potentiometer is plugged into (J2).
const JACK: usize = 0;

/// Contact pulled to the low rail, as on most expression pedals.
const LOW_CONTACT: Contact = Contact::Sleeve;
/// Contact left floating on the potentiometer wiper.
const WIPER_CONTACT: Contact = Contact::Tip;
/// Contact pulled to the high rail.
const HIGH_CONTACT: Contact = Contact::Ring;

bind_interrupts!(struct Irqs {
    PIO0_IRQ_0 => pio::InterruptHandler<PIO0>;
    DMA_IRQ_0 => dma::InterruptHandler<DMA_CH0>;
});

#[unsafe(link_section = ".bi_entries")]
#[used]
pub static PICOTOOL_ENTRIES: [embassy_rp::binary_info::EntryAddr; 4] = [
    embassy_rp::binary_info::rp_program_name!(c"Potentiometer"),
    embassy_rp::binary_info::rp_program_description!(
        c"Measures a pedal potentiometer wiper between the low/high rails and shows its position on the LED strip"
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

    let wiring = JACKS[JACK];

    info!("Initializing pull switches");
    let mut switches = board::pull_switches(p.SPI1, p.PIN_14, p.PIN_15, p.PIN_11, p.PIN_13);
    switches.set_output(wiring.switch_chip, LOW_CONTACT.tap(), TriState::Low);
    switches.set_output(wiring.switch_chip, HIGH_CONTACT.tap(), TriState::High);
    switches.update().unwrap();

    info!("Initializing ADCs");
    let mut adcs = board::adcs(
        p.SPI0, p.PIN_18, p.PIN_19, p.PIN_16, p.PIN_17, p.PIN_20, p.PIN_21, p.PIN_22,
    );
    let adc_config = AdcChainConfig::default()
        .with_channel_count(board::ADC_CHANNEL_COUNT)
        .with_reference_voltage(board::REFERENCE_VOLTAGE);
    adcs.init(adc_config).await.unwrap();

    info!("Initializing WS2812B strip");
    let mut leds = board::leds(p.PIO0, Irqs, p.DMA_CH0, p.PIN_6);

    info!("Measuring potentiometer wiper position");
    loop {
        let low = adcs
            .measure_channel(wiring.adc_chip, wiring.adc_channel(LOW_CONTACT))
            .await
            .unwrap();
        let wiper = adcs
            .measure_channel(wiring.adc_chip, wiring.adc_channel(WIPER_CONTACT))
            .await
            .unwrap();
        let high = adcs
            .measure_channel(wiring.adc_chip, wiring.adc_channel(HIGH_CONTACT))
            .await
            .unwrap();
        let position = (wiper - low) / (high - low);

        info!(
            "wiper voltage: {}V (low {}V, high {}V, position {}%)",
            wiper,
            low,
            high,
            position * 100.0
        );

        show_position(&mut leds, position);
        leds.update().await;
    }
}
