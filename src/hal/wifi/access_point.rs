use core::net::Ipv4Addr;

use cyw43::{NetDriver, PowerManagementMode, SpiBus, aligned_bytes};
use cyw43_pio::{PioSpi, RM2_CLOCK_DIVIDER};
use defmt::{info, unwrap};
use embassy_executor::Spawner;
use embassy_net::{Ipv4Cidr, Stack, StackResources, StaticConfigV4};
use embassy_rp::Peri;
use embassy_rp::clocks::RoscRng;
use embassy_rp::dma::{self, ChannelInstance};
use embassy_rp::gpio::{Level, Output};
use embassy_rp::interrupt::typelevel::{Binding, PIO1_IRQ_0};
use embassy_rp::peripherals::{PIN_23, PIN_24, PIN_25, PIN_29, PIO1};
use embassy_rp::pio::{self, Pio};
use static_cell::StaticCell;

use super::dhcp::run_dhcp_server;

/// TCP and UDP sockets the network stack can hold at once, shared by every service on it.
const SOCKET_COUNT: usize = 8;

type WifiSpi = SpiBus<Output<'static>, PioSpi<'static, PIO1, 0>>;

#[derive(Debug, Clone, Copy)]
pub struct AccessPointConfig {
    pub ssid: &'static str,
    /// WPA2 password (8-63 printable ASCII characters).
    pub password: &'static str,
    pub channel: u8,
    /// Address of the device itself; DHCP clients are leased addresses from the same /24 subnet.
    pub address: Ipv4Addr,
}

impl Default for AccessPointConfig {
    fn default() -> Self {
        Self {
            ssid: "Expression Adapter",
            // Validated by build.rs, see .env.example
            password: env!("EXPAD_WIFI_PASSWORD"),
            channel: 6,
            address: Ipv4Addr::new(192, 168, 4, 1),
        }
    }
}

/// The Pico 2 W's hard-wired CYW43439 connections.
pub struct AccessPointPeripherals<D: ChannelInstance> {
    pub pio: Peri<'static, PIO1>,
    pub dma: Peri<'static, D>,
    pub power: Peri<'static, PIN_23>,
    pub data: Peri<'static, PIN_24>,
    pub chip_select: Peri<'static, PIN_25>,
    pub clock: Peri<'static, PIN_29>,
}

#[embassy_executor::task]
async fn run_wifi(runner: cyw43::Runner<'static, WifiSpi>) -> ! {
    runner.run().await
}

#[embassy_executor::task]
async fn run_network(mut runner: embassy_net::Runner<'static, NetDriver<'static>>) -> ! {
    runner.run().await
}

/// Opens a WPA2-protected WiFi access point on the Pico 2 W and returns its network stack, with a DHCP
/// server already handing out addresses to clients. Panics if called twice.
pub async fn start_access_point<D: ChannelInstance>(
    spawner: Spawner,
    peripherals: AccessPointPeripherals<D>,
    irqs: impl Binding<PIO1_IRQ_0, pio::InterruptHandler<PIO1>>
    + Binding<D::Interrupt, dma::InterruptHandler<D>>
    + 'static,
    config: AccessPointConfig,
) -> Stack<'static> {
    let mut pio = Pio::new(peripherals.pio, irqs);
    let spi = PioSpi::new(
        &mut pio.common,
        pio.sm0,
        RM2_CLOCK_DIVIDER,
        pio.irq0,
        Output::new(peripherals.chip_select, Level::High),
        peripherals.data,
        peripherals.clock,
        dma::Channel::new(peripherals.dma, irqs),
    );
    let power = Output::new(peripherals.power, Level::Low);

    static WIFI_STATE: StaticCell<cyw43::State> = StaticCell::new();
    let firmware = aligned_bytes!("../../../firmware/cyw43/43439A0.bin");
    let nvram = aligned_bytes!("../../../firmware/cyw43/nvram_rp2040.bin");
    let (net_driver, mut control, wifi_runner) = cyw43::new(
        WIFI_STATE.init(cyw43::State::new()),
        power,
        spi,
        firmware,
        nvram,
    )
    .await;
    spawner.spawn(unwrap!(run_wifi(wifi_runner)));

    control
        .init(include_bytes!("../../../firmware/cyw43/43439A0_clm.bin"))
        .await;
    control
        .set_power_management(PowerManagementMode::PowerSave)
        .await;

    let network_config = embassy_net::Config::ipv4_static(StaticConfigV4 {
        address: Ipv4Cidr::new(config.address, 24),
        gateway: None,
        dns_servers: Default::default(),
    });
    static NETWORK_RESOURCES: StaticCell<StackResources<SOCKET_COUNT>> = StaticCell::new();
    let (stack, network_runner) = embassy_net::new(
        net_driver,
        network_config,
        NETWORK_RESOURCES.init(StackResources::new()),
        RoscRng.next_u64(),
    );
    spawner.spawn(unwrap!(run_network(network_runner)));

    control
        .start_ap_wpa2(config.ssid, config.password, config.channel)
        .await;
    spawner.spawn(unwrap!(run_dhcp_server(stack, config.address)));

    info!(
        "WiFi access point '{}' up at {}",
        config.ssid, config.address
    );
    stack
}
