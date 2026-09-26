use core::future::Future;

use defmt::info;
use embassy_futures::select::{Either, select};
use embassy_rp::Peri;
use embassy_rp::interrupt::typelevel::{Binding, USBCTRL_IRQ};
use embassy_rp::peripherals::USB;
use embassy_rp::usb::{Driver, InterruptHandler};
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::watch::{Receiver, Watch};
use embassy_usb::class::midi::MidiClass;
use embassy_usb::driver::EndpointError;
use embassy_usb::{Builder, Handler, UsbDevice};
use static_cell::ConstStaticCell;
use usbd_midi::class::MAX_PACKET_SIZE;
use usbd_midi::{CableNumber, Message, UsbMidiEventPacket, UsbMidiPacketReader};

use super::driver::GuardedDriver;

/// The USB device behind a [`UsbMidi`] port. Its `run` future must be polled for as long as
/// the port is used, e.g. from its own task.
pub type UsbMidiDevice = UsbDevice<'static, GuardedDriver>;

#[derive(Debug, Clone, Copy)]
pub struct UsbMidiConfig {
    pub vendor_id: u16,
    pub product_id: u16,
    pub manufacturer: &'static str,
    pub product: &'static str,
}

impl Default for UsbMidiConfig {
    fn default() -> Self {
        // pid.codes test IDs, reserved for development use only.
        Self {
            vendor_id: 0x1209,
            product_id: 0x0001,
            manufacturer: "Amon Benson",
            product: "Expression Adapter",
        }
    }
}

/// Whether a host has configured the device and is not suspending it. Without VBUS sensing, a
/// host going away shows up as a suspend too, since the bus falls idle.
static HOST_CONNECTED: Watch<CriticalSectionRawMutex, bool, 1> = Watch::new_with(false);

type HostConnectedReceiver = Receiver<'static, CriticalSectionRawMutex, bool, 1>;

/// Follows the device state embassy-usb reports into [`HOST_CONNECTED`].
struct ConnectionTracker {
    configured: bool,
    suspended: bool,
}

impl ConnectionTracker {
    const fn new() -> Self {
        Self {
            configured: false,
            suspended: false,
        }
    }

    fn publish(&self) {
        let connected = self.configured && !self.suspended;
        HOST_CONNECTED.sender().send_if_modified(|previous| {
            let changed = *previous != Some(connected);
            if changed {
                info!(
                    "USB MIDI host {}",
                    if connected {
                        "connected"
                    } else {
                        "disconnected"
                    }
                );
            }
            *previous = Some(connected);
            changed
        });
    }
}

impl Handler for ConnectionTracker {
    fn enabled(&mut self, enabled: bool) {
        if !enabled {
            self.configured = false;
        }
        self.publish();
    }

    fn reset(&mut self) {
        self.configured = false;
        self.suspended = false;
        self.publish();
    }

    fn configured(&mut self, configured: bool) {
        self.configured = configured;
        self.publish();
    }

    fn suspended(&mut self, suspended: bool) {
        self.suspended = suspended;
        self.publish();
    }
}

/// Size of the buffer control transfers are answered from. embassy-usb also builds string
/// descriptors in it, and panics mid-enumeration on one that does not fit, so it holds the
/// longest a descriptor can be (255 bytes).
const CONTROL_BUFFER_SIZE: usize = 256;

/// UTF-16 code units a string descriptor can hold in [`CONTROL_BUFFER_SIZE`], after its two
/// header bytes and the spare two embassy-usb insists on.
const MAX_STRING_LENGTH: usize = (CONTROL_BUFFER_SIZE - 4) / 2;

struct UsbStorage {
    config_descriptor: [u8; 256],
    bos_descriptor: [u8; 256],
    control: [u8; CONTROL_BUFFER_SIZE],
    connection_tracker: ConnectionTracker,
}

static USB_STORAGE: ConstStaticCell<UsbStorage> = ConstStaticCell::new(UsbStorage {
    config_descriptor: [0; 256],
    bos_descriptor: [0; 256],
    control: [0; CONTROL_BUFFER_SIZE],
    connection_tracker: ConnectionTracker::new(),
});

/// USB MIDI port with one input and one output jack. Transfers end with
/// `EndpointError::Disabled` as soon as the host disconnects, so a caller never hangs on a host
/// that went away; wait for the next one with [`UsbMidi::wait_connection`].
pub struct UsbMidi {
    class: MidiClass<'static, GuardedDriver>,
    host_connected: HostConnectedReceiver,
}

impl UsbMidi {
    /// Creates the MIDI port together with the `UsbMidiDevice`, whose `run` future
    /// must be polled concurrently for the port to work. Panics if called twice, or if a
    /// name in `config` is longer than a string descriptor can hold.
    pub fn new(
        usb: Peri<'static, USB>,
        irq: impl Binding<USBCTRL_IRQ, InterruptHandler<USB>>,
        config: UsbMidiConfig,
    ) -> (Self, UsbMidiDevice) {
        // Checked here rather than when a host first asks for the string, which not every
        // host does.
        for name in [config.manufacturer, config.product] {
            assert!(
                name.encode_utf16().count() <= MAX_STRING_LENGTH,
                "USB string descriptor too long"
            );
        }

        let mut usb_config = embassy_usb::Config::new(config.vendor_id, config.product_id);
        usb_config.manufacturer = Some(config.manufacturer);
        usb_config.product = Some(config.product);

        let storage = USB_STORAGE.take();
        let mut builder = Builder::new(
            GuardedDriver::new(Driver::new(usb, irq)),
            usb_config,
            &mut storage.config_descriptor,
            &mut storage.bos_descriptor,
            &mut [],
            &mut storage.control,
        );
        builder.handler(&mut storage.connection_tracker);
        let class = MidiClass::new(&mut builder, 1, 1, MAX_PACKET_SIZE as u16);

        let port = Self {
            class,
            host_connected: HOST_CONNECTED
                .receiver()
                .expect("the USB MIDI port is created only once"),
        };
        (port, builder.build())
    }

    /// Waits until a host has configured the device and is not suspending it.
    pub async fn wait_connection(&mut self) {
        self.host_connected.get_and(|&connected| connected).await;
    }

    /// Waits for the next USB transfer and returns an iterator over the packets it contains.
    pub async fn receive<'b>(
        &mut self,
        buffer: &'b mut [u8; MAX_PACKET_SIZE],
    ) -> Result<UsbMidiPacketReader<'b>, EndpointError> {
        let length = while_connected(
            &mut self.host_connected,
            self.class.read_packet(buffer.as_mut_slice()),
        )
        .await?;
        Ok(UsbMidiPacketReader::new(buffer, length))
    }

    /// Sends one packet once the host has taken the previous one. The host only reads while
    /// an application has the port open, so this can take arbitrarily long.
    pub async fn send_packet(&mut self, packet: &UsbMidiEventPacket) -> Result<(), EndpointError> {
        while_connected(
            &mut self.host_connected,
            self.class.write_packet(packet.as_raw_bytes()),
        )
        .await
    }

    pub async fn send_message(
        &mut self,
        cable: CableNumber,
        message: Message,
    ) -> Result<(), EndpointError> {
        self.send_packet(&message.into_packet(cable)).await
    }
}

/// Runs `transfer` until it completes or the host disconnects. embassy-rp's endpoints do not
/// notice a disconnect themselves, their transfers would wait for the host forever.
async fn while_connected<T>(
    host_connected: &mut HostConnectedReceiver,
    transfer: impl Future<Output = Result<T, EndpointError>>,
) -> Result<T, EndpointError> {
    let disconnected = host_connected.get_and(|&connected| !connected);
    match select(transfer, disconnected).await {
        Either::First(result) => result,
        Either::Second(_) => Err(EndpointError::Disabled),
    }
}
