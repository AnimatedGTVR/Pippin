//! Event Manager: routes timer and application events through IPC ports.

use core::sync::atomic::{AtomicU16, Ordering};

use crate::{ipc, sched};

pub const TIMER: u16 = 1;
pub const APPLICATION: u16 = 2;

static BOOT_PORT: AtomicU16 = AtomicU16::new(0);

pub fn set_boot_port(port: u16) {
    BOOT_PORT.store(port, Ordering::Relaxed);
}

pub fn post(port: u16, kind: u16, value: u64) -> bool {
    ipc::send(port, kind, value)
}

pub fn poll() -> Option<ipc::Event> {
    ipc::recv(sched::current_slot().event_port)
}

/// Called from the 1000 Hz timer interrupt. Publish a 10 Hz desktop tick.
pub fn on_tick(tick: u64) {
    if tick % 100 != 0 { return; }
    let port = BOOT_PORT.load(Ordering::Relaxed);
    if port != 0 { let _ = ipc::send_system(port, TIMER, tick); }
}
