use core::net::{Ipv4Addr, SocketAddr};
use core::time::Duration;

use defmt::{Debug2Format, warn};
use edge_captive::io::run;
use edge_nal_embassy::{Udp, UdpBuffers};
use embassy_net::Stack;

const DNS_PORT: u16 = 53;

/// How long clients may cache an answer. Short, since every name resolves to this device only
/// while they are on its network.
const ANSWER_TTL: Duration = Duration::from_secs(60);

/// Answers every DNS query for an IPv4 address with `address`, and every other query with no
/// address at all. Phones and computers look up a fixed host after joining a network to check
/// for internet access; reaching this device instead makes them take it for a captive portal
/// and open the web interface.
#[embassy_executor::task]
pub async fn run_dns_server(stack: Stack<'static>, address: Ipv4Addr) -> ! {
    let buffers = UdpBuffers::<1>::new();
    let udp = Udp::new(stack, &buffers);
    let bind_address = SocketAddr::new(Ipv4Addr::UNSPECIFIED.into(), DNS_PORT);
    let mut tx_buffer = [0; 512];
    let mut rx_buffer = [0; 512];
    loop {
        let result = run(
            &udp,
            bind_address,
            &mut tx_buffer,
            &mut rx_buffer,
            address,
            ANSWER_TTL,
        );
        if let Err(error) = result.await {
            warn!("DNS server error: {}", Debug2Format(&error));
        }
    }
}
