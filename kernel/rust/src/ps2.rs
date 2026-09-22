//! Basic PS/2 keyboard and mouse byte stream feeding Event Manager ports.

use crate::{cpu, ipc};

pub const KEY_EVENT: u16 = 0x100;
pub const MOUSE_EVENT: u16 = 0x101;

static mut MOUSE_BYTES: [u8; 4] = [0; 4];
static mut MOUSE_INDEX: usize = 0;
static mut MOUSE_PACKET_SIZE: usize = 3;

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

fn mouse_command(value: u8) -> bool {
    if !command(0xd4) || !data(value) || !wait_output_full() { return false; }
    unsafe { cpu::inb(0x60) == 0xfa }
}

fn mouse_sample_rate(rate: u8) -> bool {
    mouse_command(0xf3) && mouse_command(rate)
}

fn mouse_id() -> Option<u8> {
    if !mouse_command(0xf2) || !wait_output_full() { return None; }
    Some(unsafe { cpu::inb(0x60) })
}

/// Enable the second PS/2 port and packet streaming when a mouse responds.
///
/// Standard PS/2 mice use 3-byte packets. The 200/100/80 sample-rate sequence
/// asks IntelliMouse-compatible devices (including QEMU's PS/2 mouse) to expose
/// a fourth wheel byte. Failure is non-fatal: Pippin simply keeps 3-byte mode.
pub fn init_mouse() -> bool {
    if !command(0xa8) || !command(0x20) || !wait_output_full() { return false; }
    let config = unsafe { cpu::inb(0x60) };
    if !command(0x60) || !data((config | 2) & !0x20) { return false; }

    if !mouse_command(0xf6) { return false; } // defaults

    let wheel_mode =
        mouse_sample_rate(200)
        && mouse_sample_rate(100)
        && mouse_sample_rate(80)
        && matches!(mouse_id(), Some(3 | 4));

    unsafe {
        MOUSE_PACKET_SIZE = if wheel_mode { 4 } else { 3 };
        MOUSE_INDEX = 0;
    }

    mouse_command(0xf4)
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
                if MOUSE_INDEX == MOUSE_PACKET_SIZE {
                    MOUSE_INDEX = 0;

                    // IntelliMouse wheel movement is a signed 4-bit nibble.
                    let wheel = if MOUSE_PACKET_SIZE == 4 {
                        let nibble = MOUSE_BYTES[3] & 0x0f;
                        if nibble & 0x08 != 0 { nibble | 0xf0 } else { nibble }
                    } else {
                        0
                    };

                    let packet = u32::from(MOUSE_BYTES[0]) |
                        (u32::from(MOUSE_BYTES[1]) << 8) |
                        (u32::from(MOUSE_BYTES[2]) << 16) |
                        (u32::from(wheel) << 24);
                    if ipc::send_system(port, MOUSE_EVENT, packet as u64) { events += 1; }
                }
            }
        }
    }
    events
}
