use core::net::{Ipv4Addr, SocketAddr};

use defmt::{Debug2Format, unwrap, warn};
use edge_dhcp::io::{DEFAULT_SERVER_PORT, server};
use edge_dhcp::server::{Server, ServerOptions};
use edge_nal::UdpBind;
use edge_nal_embassy::{Udp, UdpBuffers};
use embassy_net::Stack;

/// Clients that can hold a lease at the same time.
const MAX_LEASES: usize = 8;

/// Leases addresses `.50`-`.200` of the server's /24 subnet to every client on the network.
#[embassy_executor::task]
pub async fn run_dhcp_server(stack: Stack<'static>, address: Ipv4Addr) -> ! {
    let buffers = UdpBuffers::<1>::new();
    let udp = Udp::new(stack, &buffers);
    let bind_address = SocketAddr::new(Ipv4Addr::UNSPECIFIED.into(), DEFAULT_SERVER_PORT);
    let mut socket = unwrap!(udp.bind(bind_address).await);

    let options = ServerOptions::new(address, None);
    let mut dhcp_server = Server::<_, MAX_LEASES>::new_with_et(address);
    let mut packet_buffer = [0; 1500];
    loop {
        let result = server::run(&mut dhcp_server, &options, &mut socket, &mut packet_buffer);
        if let Err(error) = result.await {
            warn!("DHCP server error: {}", Debug2Format(&error));
        }
    }
}
