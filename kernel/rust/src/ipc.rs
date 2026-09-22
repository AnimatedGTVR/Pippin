//! Fixed-capacity message ports for the Event Manager and kernel threads.

use crate::{cpu, sched};

const MAX_PORTS: usize = 8;
const QUEUE_LEN: usize = 16;

#[repr(C)]
#[derive(Clone, Copy)]
pub struct Event {
    pub kind: u16,
    pub source_pid: u32,
    pub value: u64,
}

const EMPTY_EVENT: Event = Event { kind: 0, source_pid: 0, value: 0 };

#[derive(Clone, Copy)]
struct Port {
    owner: u32,
    events: [Event; QUEUE_LEN],
    head: usize,
    len: usize,
}

impl Port {
    const fn empty() -> Self {
        Self { owner: 0, events: [EMPTY_EVENT; QUEUE_LEN], head: 0, len: 0 }
    }
}

static mut PORTS: [Port; MAX_PORTS] = [Port::empty(); MAX_PORTS];

/// Create a port owned by the current process slot. Port IDs start at 1.
pub fn create() -> Option<u16> {
    let flags = cpu::irq_save();
    let mut result = None;
    unsafe {
        for (i, port) in PORTS.iter_mut().enumerate() {
            if port.owner == 0 {
                *port = Port::empty();
                port.owner = sched::current_pid();
                result = Some((i + 1) as u16);
                break;
            }
        }
    }
    cpu::irq_restore(flags);
    result
}

/// Send an event to a port. The source PID is assigned by the kernel.
pub fn send(id: u16, kind: u16, value: u64) -> bool {
    send_as(id, kind, value, sched::current_pid())
}

fn send_as(id: u16, kind: u16, value: u64, source_pid: u32) -> bool {
    if id == 0 || id as usize > MAX_PORTS { return false; }
    let flags = cpu::irq_save();
    let port = unsafe { &mut PORTS[id as usize - 1] };
    let ok = if port.owner != 0 && port.len < QUEUE_LEN {
        let tail = (port.head + port.len) % QUEUE_LEN;
        port.events[tail] = Event { kind, source_pid, value };
        port.len += 1;
        true
    } else { false };
    cpu::irq_restore(flags);
    ok
}

/// Receive an event only when the caller owns the port.
pub fn recv(id: u16) -> Option<Event> {
    if id == 0 || id as usize > MAX_PORTS { return None; }
    let flags = cpu::irq_save();
    let port = unsafe { &mut PORTS[id as usize - 1] };
    let event = if port.owner == sched::current_pid() && port.len != 0 {
        let event = port.events[port.head];
        port.head = (port.head + 1) % QUEUE_LEN;
        port.len -= 1;
        Some(event)
    } else { None };
    cpu::irq_restore(flags);
    event
}

/// Send a kernel-generated event with system source PID 0.
pub fn send_system(id: u16, kind: u16, value: u64) -> bool {
    send_as(id, kind, value, 0)
}
