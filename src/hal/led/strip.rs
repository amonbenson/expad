use embassy_rp::pio::Instance;

use super::ws2812::{RGB8, Ws2812Chain};

/// Global brightness defaults to 50% of the maximum scale (`u8::MAX`).
const DEFAULT_BRIGHTNESS: u8 = u8::MAX / 2;

/// Tracks the desired color of every LED on a WS2812B strip plus a global
/// brightness scale, and pushes that state out to the hardware on `update`.
pub struct LedStrip<'d, P: Instance, const N: usize> {
    chain: Ws2812Chain<'d, P, N>,
    colors: [RGB8; N],
    brightness: u8,
}

impl<'d, P: Instance, const N: usize> LedStrip<'d, P, N> {
    pub fn new(chain: Ws2812Chain<'d, P, N>) -> Self {
        Self {
            chain,
            colors: [RGB8::default(); N],
            brightness: DEFAULT_BRIGHTNESS,
        }
    }

    pub fn color(&self, index: usize) -> RGB8 {
        self.colors[index]
    }

    pub fn set_color(&mut self, index: usize, color: RGB8) {
        self.colors[index] = color;
    }

    pub fn brightness(&self) -> u8 {
        self.brightness
    }

    /// Sets the global brightness scale, where `0` is off and `u8::MAX` is full brightness.
    pub fn set_brightness(&mut self, brightness: u8) {
        self.brightness = brightness;
    }

    /// Writes the current LED colors, scaled by the global brightness, to the strip.
    pub async fn update(&mut self) {
        let scaled_colors = self.colors.map(|color| scale_color(color, self.brightness));
        self.chain.write(&scaled_colors).await;
    }
}

fn scale_color(color: RGB8, brightness: u8) -> RGB8 {
    RGB8::new(
        scale_channel(color.r, brightness),
        scale_channel(color.g, brightness),
        scale_channel(color.b, brightness),
    )
}

fn scale_channel(value: u8, brightness: u8) -> u8 {
    (value as u16 * brightness as u16 / u8::MAX as u16) as u8
}
