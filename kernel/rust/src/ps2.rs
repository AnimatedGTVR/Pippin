//! Basic PS/2 keyboard and mouse byte stream feeding Event Manager ports.

use crate::{cpu, ipc};

pub const KEY_EVENT: u16 = 0x100;
pub const MOUSE_EVENT: u16 = 0x101;

static mut MOUSE_BYTES: [u8; 3] = [0; 3];
static mut MOUSE_INDEX: usize = 0;

fn wait_input_clear() -> bool {
    for _ in 0..100_000 {
        if unsafe { cpu::inb(0x64) } & 2 == 0 { return true; }
    }
    false
}

fn wait_output_full() -> bool {
    for _ in 0..100_000 {
        if unsafe { cpu::inb(0x64) } & 1 != 0 { return true; }
    }
    false
}

fn command(value: u8) -> bool {
    if !wait_input_clear() { return false; }
    unsafe { cpu::outb(0x64, value); }
    true
}

fn data(value: u8) -> bool {
    if !wait_input_clear() { return false; }
    unsafe { cpu::outb(0x60, value); }
    true
}

/// Enable the second PS/2 port and packet streaming when a mouse responds.
pub fn init_mouse() -> bool {
    if !command(0xa8) || !command(0x20) || !wait_output_full() { return false; }
    let config = unsafe { cpu::inb(0x60) };
    if !command(0x60) || !data((config | 2) & !0x20) { return false; }
    if !command(0xd4) || !data(0xf4) || !wait_output_full() { return false; }
    unsafe { cpu::inb(0x60) == 0xfa }
}

/// Drain pending controller bytes. Polling avoids IRQ routing assumptions.
pub fn poll(port: u16) -> usize {
    let mut events = 0;
    for _ in 0..32 {
        let status = unsafe { cpu::inb(0x64) };
        if status & 1 == 0 { break; }
        let byte = unsafe { cpu::inb(0x60) };
        if status & 0x20 == 0 {
            if ipc::send_system(port, KEY_EVENT, byte as u64) { events += 1; }
        } else {
            unsafe {
                if MOUSE_INDEX == 0 && byte & 8 == 0 { continue; }
                MOUSE_BYTES[MOUSE_INDEX] = byte;
                MOUSE_INDEX += 1;
                if MOUSE_INDEX == 3 {
                    MOUSE_INDEX = 0;
                    let packet = u32::from(MOUSE_BYTES[0]) |
                        (u32::from(MOUSE_BYTES[1]) << 8) |
                        (u32::from(MOUSE_BYTES[2]) << 16);
                    if ipc::send_system(port, MOUSE_EVENT, packet as u64) { events += 1; }
                }
            }
        }
    }
    events
}
