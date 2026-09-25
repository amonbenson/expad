//! Wiring of the expression controller PCB (hardware/ExpressionController.kicad_pro): which
//! pins, chips and channels every jack's contacts are connected to. Every application builds
//! its drivers through here, so this module is the only place that knows the board.
//!
//! Each jack J2-J5 (left to right) has its own analog channel block: a 74HC595 whose outputs
//! drive two TMUX1511 quad switches, one connecting each contact to a shared 1 kΩ pull-up to
//! the 2.5 V reference, the other to a shared 1 kΩ pull-down to ground. Every contact is also
//! read through a 10 kΩ / 10 nF filter by one of two AD7718s. The WS2812B LEDs D4-D7 sit
//! below the jacks in the same order.

use embassy_rp::Peri;
use embassy_rp::dma::{self, ChannelInstance};
use embassy_rp::gpio::AnyPin;
use embassy_rp::interrupt::typelevel::Binding;
use embassy_rp::peripherals::{
    PIN_6, PIN_11, PIN_13, PIN_14, PIN_15, PIN_16, PIN_17, PIN_18, PIN_19, PIN_20, PIN_21, PIN_22,
    SPI0, SPI1,
};
use embassy_rp::pio::{self, Instance};

use crate::hal::adc::{AdcChain, ChannelCount};
use crate::hal::buf::{PullSwitchChain, ShiftRegisterChain};
use crate::hal::led::{LedStrip, Ws2812Chain};
use crate::topology::ARM_COUNT;
use crate::topology::scanner::JackConfig;

/// Expression pedal jacks on the board (J2-J5).
pub const JACK_COUNT: usize = 4;

/// 74HC595 shift registers in the pull switch chain, one per jack.
pub const SWITCH_CHIPS: usize = JACK_COUNT;

/// AD7718 converters, each reading two jacks.
pub const ADC_CHIPS: usize = 2;

/// WS2812B LEDs, one below each jack.
pub const LED_COUNT: usize = JACK_COUNT;

/// Input mode every AD7718 has to run in: the second jack on each chip uses AIN9.
pub const ADC_CHANNEL_COUNT: ChannelCount = ChannelCount::Ten;

/// ADC inputs tied to ground (AIN6) and to the reference (AIN10) on every chip, which make
/// a free self-test of the whole conversion path.
pub const ADC_GROUND_CHANNEL: u8 = 5;
pub const ADC_REFERENCE_CHANNEL: u8 = 9;

/// Voltage of the ADCs' reference (REFIN1), which is also the rail every pull-up switches to,
/// so every reading is ratiometric to the pull-ups.
pub const REFERENCE_VOLTAGE: f32 = 2.5;

/// Shared pull-up and pull-down resistance of every jack (0.1%), in kΩ.
pub const PULL_RESISTANCE: f32 = 1.0;

/// Series resistance and capacitance of every ADC input's filter, in kΩ and nF.
pub const INPUT_FILTER_RESISTANCE: f32 = 10.0;
pub const INPUT_FILTER_CAPACITANCE: f32 = 10.0;

/// A contact of a jack. Its discriminant is the tap it is switched through on its jack's
/// 74HC595.
#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, defmt::Format)]
pub enum Contact {
    Tip = 0,
    Ring = 1,
    Sleeve = 2,
    /// The tip's normalling contact, which touches the tip while no plug is inserted.
    TipSwitch = 3,
}

impl Contact {
    pub const ALL: [Contact; 4] = [
        Contact::Tip,
        Contact::Ring,
        Contact::Sleeve,
        Contact::TipSwitch,
    ];

    /// The contacts the solver measures as a jack's arms, in arm order.
    pub const ARMS: [Contact; ARM_COUNT] = [Contact::Tip, Contact::Ring, Contact::Sleeve];

    pub const fn tap(self) -> u8 {
        self as u8
    }
}

/// Where one jack's contacts are wired.
#[derive(Clone, Copy, Debug)]
pub struct JackWiring {
    /// The jack's 74HC595, counted from the microcontroller along the chain.
    pub switch_chip: usize,
    pub adc_chip: usize,
    /// ADC input reading every contact, indexed by [`Contact`] (AIN number minus one).
    pub adc_channels: [u8; 4],
}

impl JackWiring {
    pub const fn adc_channel(&self, contact: Contact) -> u8 {
        self.adc_channels[contact as usize]
    }

    /// The jack's wiring as the scanner takes it: tip, ring and sleeve as the three arms,
    /// then the tip switch - the contact order of `expad-topology`.
    pub const fn config(&self) -> JackConfig {
        JackConfig {
            switch_chip: self.switch_chip,
            switch_taps: [
                Contact::Tip.tap(),
                Contact::Ring.tap(),
                Contact::Sleeve.tap(),
                Contact::TipSwitch.tap(),
            ],
            adc_chip: self.adc_chip,
            adc_channels: [
                self.adc_channel(Contact::Tip),
                self.adc_channel(Contact::Ring),
                self.adc_channel(Contact::Sleeve),
                self.adc_channel(Contact::TipSwitch),
            ],
        }
    }
}

/// Tip, ring, sleeve and tip switch of the first jack on each AD7718: AIN8, AIN1, AIN7, AIN2.
const FIRST_JACK_ADC_CHANNELS: [u8; 4] = [7, 0, 6, 1];

/// The same for the second jack on each AD7718: AIN3, AIN9, AIN4, AIN5.
const SECOND_JACK_ADC_CHANNELS: [u8; 4] = [2, 8, 3, 4];

/// Every jack's wiring, J2 to J5.
pub const JACKS: [JackWiring; JACK_COUNT] = [
    JackWiring {
        switch_chip: 0,
        adc_chip: 0,
        adc_channels: FIRST_JACK_ADC_CHANNELS,
    },
    JackWiring {
        switch_chip: 1,
        adc_chip: 0,
        adc_channels: SECOND_JACK_ADC_CHANNELS,
    },
    JackWiring {
        switch_chip: 2,
        adc_chip: 1,
        adc_channels: FIRST_JACK_ADC_CHANNELS,
    },
    JackWiring {
        switch_chip: 3,
        adc_chip: 1,
        adc_channels: SECOND_JACK_ADC_CHANNELS,
    },
];

/// The pull switch chain on SPI1: clock GPIO14, data GPIO15, output enable GPIO11 (pulled
/// high on the board, so the switches stay open until the first write) and latch GPIO13.
pub fn pull_switches<'d>(
    spi: Peri<'d, SPI1>,
    clock: Peri<'d, PIN_14>,
    data: Peri<'d, PIN_15>,
    output_enable: Peri<'d, PIN_11>,
    latch: Peri<'d, PIN_13>,
) -> PullSwitchChain<'d, SWITCH_CHIPS> {
    let shift_registers = ShiftRegisterChain::new(spi, clock, data, output_enable, latch);
    PullSwitchChain::new(shift_registers)
}

/// Both AD7718s on SPI0: clock GPIO18, TX GPIO19, RX GPIO16, chip selects GPIO17 and GPIO20,
/// ready signals GPIO21 and GPIO22. Still has to be initialized with [`ADC_CHANNEL_COUNT`]
/// and [`REFERENCE_VOLTAGE`].
#[allow(clippy::too_many_arguments)]
pub fn adcs<'d>(
    spi: Peri<'d, SPI0>,
    clock: Peri<'d, PIN_18>,
    tx: Peri<'d, PIN_19>,
    rx: Peri<'d, PIN_16>,
    first_chip_select: Peri<'d, PIN_17>,
    second_chip_select: Peri<'d, PIN_20>,
    first_ready: Peri<'d, PIN_21>,
    second_ready: Peri<'d, PIN_22>,
) -> AdcChain<'d, ADC_CHIPS> {
    let chip_selects: [Peri<'d, AnyPin>; ADC_CHIPS] =
        [first_chip_select.into(), second_chip_select.into()];
    let ready_signals: [Peri<'d, AnyPin>; ADC_CHIPS] = [first_ready.into(), second_ready.into()];

    AdcChain::new(spi, clock, tx, rx, chip_selects, ready_signals)
}

/// The LED strip on GPIO6, through a 74AHCT1G125 level shifter.
pub fn leds<'d, P: Instance, D: ChannelInstance>(
    pio: Peri<'d, P>,
    irqs: impl Binding<P::Interrupt, pio::InterruptHandler<P>>
    + Binding<D::Interrupt, dma::InterruptHandler<D>>
    + 'd,
    dma: Peri<'d, D>,
    data: Peri<'d, PIN_6>,
) -> LedStrip<'d, P, LED_COUNT> {
    LedStrip::new(Ws2812Chain::new(pio, irqs, dma, data))
}
