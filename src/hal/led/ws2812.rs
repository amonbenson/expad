use embassy_rp::Peri;
use embassy_rp::dma::{self, ChannelInstance};
use embassy_rp::interrupt::typelevel::Binding;
use embassy_rp::pio::{self, Common, Instance, Pio, PioPin};
use embassy_rp::pio_programs::ws2812::{Grb, PioWs2812, PioWs2812Program};

pub use smart_leds::RGB8;

/// Highest duty cycle any LED channel is ever driven at, out of `u8::MAX`: about 20%.
///
/// The strip runs from the board's 5 V linear regulator (200 mA, shared with the Pico), which
/// cannot supply every LED at full white. Every frame is scaled into this range right before
/// it is sent, so no caller can exceed it - full scale for a caller means this much.
pub const MAX_CHANNEL_VALUE: u8 = 51;

/// Drives a WS2812B ("NeoPixel") LED strip over one PIO block's state machine 0.
///
/// `N` is the number of LEDs wired in series on the strip.
pub struct Ws2812Chain<'d, P: Instance, const N: usize> {
    // Dropping `Common` releases the PIO block's claimed pins back to `FUNCSEL::NULL`
    // once no other state machine on the block is still alive, so it must be kept
    // alive for as long as the driver needs the data pin to stay in PIO mode.
    common: Common<'d, P>,
    driver: PioWs2812<'d, P, 0, N, Grb>,
}

impl<'d, P: Instance, const N: usize> Ws2812Chain<'d, P, N> {
    pub fn new<D: ChannelInstance>(
        pio: Peri<'d, P>,
        irqs: impl Binding<P::Interrupt, pio::InterruptHandler<P>>
        + Binding<D::Interrupt, dma::InterruptHandler<D>>
        + 'd,
        dma: Peri<'d, D>,
        data: Peri<'d, impl PioPin>,
    ) -> Self {
        let Pio {
            mut common, sm0, ..
        } = Pio::new(pio, irqs);
        let program = PioWs2812Program::new(&mut common);
        let driver = PioWs2812::new(&mut common, sm0, dma, irqs, data, &program);

        Self { common, driver }
    }

    /// Sends a full frame of colors to the strip, scaled down to at most
    /// [`MAX_CHANNEL_VALUE`]. Blocks (asynchronously) for the WS2812B latch delay after the
    /// data has been shifted out.
    pub async fn write(&mut self, colors: &[RGB8; N]) {
        let limited_colors = colors.map(limit_color);
        self.driver.write(&limited_colors).await;
    }
}

fn limit_color(color: RGB8) -> RGB8 {
    RGB8::new(
        limit_channel(color.r),
        limit_channel(color.g),
        limit_channel(color.b),
    )
}

fn limit_channel(value: u8) -> u8 {
    (value as u16 * MAX_CHANNEL_VALUE as u16 / u8::MAX as u16) as u8
}
