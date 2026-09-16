use embassy_rp::Peri;
use embassy_rp::interrupt::typelevel::{Binding, USBCTRL_IRQ};
use embassy_rp::peripherals::USB;
use embassy_rp::usb::{Driver, InterruptHandler};
use embassy_usb::class::midi::MidiClass;
use embassy_usb::driver::EndpointError;
use embassy_usb::{Builder, UsbDevice};
use static_cell::ConstStaticCell;
use usbd_midi::class::MAX_PACKET_SIZE;
use usbd_midi::{CableNumber, Message, UsbMidiEventPacket, UsbMidiPacketReader};

pub type UsbMidiDevice = UsbDevice<'static, Driver<'static, USB>>;

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
            manufacturer: "TU Berlin, Sensor and Actuator Systems",
            product: "Expression Adapter",
        }
    }
}

struct UsbBuffers {
    config_descriptor: [u8; 256],
    bos_descriptor: [u8; 256],
    control: [u8; 64],
}

static USB_BUFFERS: ConstStaticCell<UsbBuffers> = ConstStaticCell::new(UsbBuffers {
    config_descriptor: [0; 256],
    bos_descriptor: [0; 256],
    control: [0; 64],
});

/// USB MIDI port with one input and one output jack.
pub struct UsbMidi {
    class: MidiClass<'static, Driver<'static, USB>>,
}

impl UsbMidi {
    /// Creates the MIDI port together with the `UsbMidiDevice`, whose `run` future
    /// must be polled concurrently for the port to work. Panics if called twice.
    pub fn new(
        usb: Peri<'static, USB>,
        irq: impl Binding<USBCTRL_IRQ, InterruptHandler<USB>>,
        config: UsbMidiConfig,
    ) -> (Self, UsbMidiDevice) {
        let mut usb_config = embassy_usb::Config::new(config.vendor_id, config.product_id);
        usb_config.manufacturer = Some(config.manufacturer);
        usb_config.product = Some(config.product);

        let buffers = USB_BUFFERS.take();
        let mut builder = Builder::new(
            Driver::new(usb, irq),
            usb_config,
            &mut buffers.config_descriptor,
            &mut buffers.bos_descriptor,
            &mut [],
            &mut buffers.control,
        );
        let class = MidiClass::new(&mut builder, 1, 1, MAX_PACKET_SIZE as u16);

        (Self { class }, builder.build())
    }

    pub async fn wait_connection(&mut self) {
        self.class.wait_connection().await;
    }

    /// Waits for the next USB transfer and returns an iterator over the packets it contains.
    pub async fn receive<'b>(
        &mut self,
        buffer: &'b mut [u8; MAX_PACKET_SIZE],
    ) -> Result<UsbMidiPacketReader<'b>, EndpointError> {
        let length = self.class.read_packet(buffer).await?;
        Ok(UsbMidiPacketReader::new(buffer, length))
    }

    pub async fn send_packet(&mut self, packet: &UsbMidiEventPacket) -> Result<(), EndpointError> {
        self.class.write_packet(packet.as_raw_bytes()).await
    }

    pub async fn send_message(
        &mut self,
        cable: CableNumber,
        message: Message,
    ) -> Result<(), EndpointError> {
        self.send_packet(&message.into_packet(cable)).await
    }
}
