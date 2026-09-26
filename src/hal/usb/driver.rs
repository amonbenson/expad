//! embassy-rp's USB driver, guarded against the bus handling faults of embassy-rp 0.10.
//!
//! The device runs on the same executor as everything else, so a bus that reports events back
//! to back would starve every other task, and a suspend latched before a bus reset makes the
//! device ignore the host's requests after it. Remove the stale suspend fix once an embassy-rp
//! release includes embassy-rs/embassy#6693.

use defmt::{debug, info, warn};
use embassy_rp::pac;
use embassy_rp::peripherals::USB;
use embassy_rp::usb::{Bus, ControlPipe, Driver, Endpoint, In, Out};
use embassy_time::{Duration, Instant, Timer};
use embassy_usb::driver::{self, EndpointAddress, EndpointAllocError, EndpointType, Event};

/// Bus events closer together than this count as one burst.
const BURST_EVENT_GAP: Duration = Duration::from_millis(2);

/// Events in one burst from which the bus counts as flooding. Enumeration reports only a
/// handful, so a flood means the controller keeps reporting the same condition.
const FLOOD_EVENT_COUNT: u32 = 16;

/// Pause before each poll of a flooding bus, short enough to keep up with a real host but long
/// enough to leave the executor to the other tasks.
const FLOOD_BACKOFF: Duration = Duration::from_millis(1);

/// USB driver for [`embassy_usb`], passing everything but bus events straight to embassy-rp.
pub struct GuardedDriver(Driver<'static, USB>);

impl GuardedDriver {
    pub fn new(driver: Driver<'static, USB>) -> Self {
        Self(driver)
    }
}

impl driver::Driver<'static> for GuardedDriver {
    type EndpointOut = Endpoint<'static, USB, Out>;
    type EndpointIn = Endpoint<'static, USB, In>;
    type ControlPipe = ControlPipe<'static, USB>;
    type Bus = GuardedBus;

    fn alloc_endpoint_out(
        &mut self,
        endpoint_type: EndpointType,
        address: Option<EndpointAddress>,
        max_packet_size: u16,
        interval_ms: u8,
    ) -> Result<Self::EndpointOut, EndpointAllocError> {
        self.0
            .alloc_endpoint_out(endpoint_type, address, max_packet_size, interval_ms)
    }

    fn alloc_endpoint_in(
        &mut self,
        endpoint_type: EndpointType,
        address: Option<EndpointAddress>,
        max_packet_size: u16,
        interval_ms: u8,
    ) -> Result<Self::EndpointIn, EndpointAllocError> {
        self.0
            .alloc_endpoint_in(endpoint_type, address, max_packet_size, interval_ms)
    }

    fn start(self, control_max_packet_size: u16) -> (Self::Bus, Self::ControlPipe) {
        let (bus, control_pipe) = self.0.start(control_max_packet_size);
        let guarded_bus = GuardedBus {
            bus,
            last_event: Instant::MIN,
            burst_events: 0,
        };
        (guarded_bus, control_pipe)
    }
}

/// The embassy-rp bus, logging every event and pacing a flood of them.
pub struct GuardedBus {
    bus: Bus<'static, USB>,
    last_event: Instant,
    burst_events: u32,
}

impl GuardedBus {
    fn flooding(&self) -> bool {
        self.burst_events >= FLOOD_EVENT_COUNT
    }

    /// Counts `event` into the current burst, or starts a new one after a quiet gap.
    fn record(&mut self, event: Event) {
        let now = Instant::now();
        if now - self.last_event > BURST_EVENT_GAP {
            if self.flooding() {
                info!("USB bus flood over after {} events", self.burst_events);
            }
            self.burst_events = 0;
        }
        self.last_event = now;
        self.burst_events = self.burst_events.saturating_add(1);

        if self.burst_events == FLOOD_EVENT_COUNT {
            warn!(
                "USB bus floods events (last {}, SIE status {=u32:#010x}), polling it every {} ms",
                event,
                pac::USB.sie_status().read().0,
                FLOOD_BACKOFF.as_millis()
            );
        } else if !self.flooding() {
            debug!("USB bus event: {}", event);
        }
    }
}

/// Clears a suspend the controller latched before a bus reset, typically while no host was
/// connected. embassy-rp reports it right after the reset, and the device then waits for a
/// resume while the host tries to enumerate it.
fn clear_stale_suspend() {
    pac::USB
        .sie_status()
        .write(|status| status.set_suspended(true));
}

impl driver::Bus for GuardedBus {
    async fn enable(&mut self) {
        self.bus.enable().await;
    }

    async fn disable(&mut self) {
        self.bus.disable().await;
    }

    /// Cancel safe like the bus it wraps: the backoff happens before an event is taken.
    async fn poll(&mut self) -> Event {
        if self.flooding() {
            Timer::after(FLOOD_BACKOFF).await;
        }

        let event = self.bus.poll().await;
        if matches!(event, Event::Reset) {
            clear_stale_suspend();
        }
        self.record(event);
        event
    }

    fn endpoint_set_enabled(&mut self, address: EndpointAddress, enabled: bool) {
        self.bus.endpoint_set_enabled(address, enabled);
    }

    fn endpoint_set_stalled(&mut self, address: EndpointAddress, stalled: bool) {
        self.bus.endpoint_set_stalled(address, stalled);
    }

    fn endpoint_is_stalled(&mut self, address: EndpointAddress) -> bool {
        self.bus.endpoint_is_stalled(address)
    }

    fn force_reset(&mut self) -> Result<(), driver::Unsupported> {
        self.bus.force_reset()
    }

    async fn remote_wakeup(&mut self) -> Result<(), driver::Unsupported> {
        self.bus.remote_wakeup().await
    }
}
