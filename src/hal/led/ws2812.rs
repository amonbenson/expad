use embassy_rp::Peri;
use embassy_rp::dma::Channel;
use embassy_rp::interrupt::typelevel::Binding;
use embassy_rp::pio::{Common, Instance, InterruptHandler, Pio, PioPin};
use embassy_rp::pio_programs::ws2812::{Grb, PioWs2812, PioWs2812Program};

pub use smart_leds::RGB8;

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
    pub fn new(
        pio: Peri<'d, P>,
        irq: impl Binding<P::Interrupt, InterruptHandler<P>>,
        dma: Peri<'d, impl Channel>,
        data: Peri<'d, impl PioPin>,
    ) -> Self {
        let Pio {
            mut common, sm0, ..
        } = Pio::new(pio, irq);
        let program = PioWs2812Program::new(&mut common);
        let driver = PioWs2812::new(&mut common, sm0, dma, data, &program);

        Self { common, driver }
    }

    /// Sends a full frame of colors to the strip. Blocks (asynchronously) for the
    /// WS2812B latch delay after the data has been shifted out.
    pub async fn write(&mut self, colors: &[RGB8; N]) {
        self.driver.write(colors).await;
    }
}
