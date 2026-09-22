//! Minimal 8250/16550 UART driver for the COM1 serial console.
//!
//! Used for kernel logging before/without a framebuffer console. The driver
//! is intentionally tiny; a proper MMIO proxy and ring buffer live in the
//! Milestone-1 serial plan (docs/kernel.md).

use core::fmt;

use crate::cpu;

const COM1: u16 = 0x3F8;
const THR: u16 = COM1 + 0; // transmit holding register
const LSR: u16 = COM1 + 5; // line status register

static READY: core::sync::atomic::AtomicBool = core::sync::atomic::AtomicBool::new(false);

/// Initialise COM1 at 38400 baud, 8N1, FIFO on.
pub fn init(base: u16) {
    unsafe {
        cpu::outb(base + 1, 0x00); // disable interrupts
        cpu::outb(base + 3, 0x80); // DLAB on
        cpu::outb(base + 0, 0x03); // divisor low -> 38400 baud
        cpu::outb(base + 1, 0x00); // divisor high
        cpu::outb(base + 3, 0x03); // 8N1
        cpu::outb(base + 2, 0xC7); // enable FIFO, 14-byte threshold
        cpu::outb(base + 4, 0x0B); // IRQs on, RTS/DSR set
    }
    READY.store(true, core::sync::atomic::Ordering::SeqCst);
}

/// True once [`init`] has run; the panic handler uses this to avoid
/// touching unconfigured hardware.
pub fn is_ready() -> bool {
    READY.load(core::sync::atomic::Ordering::SeqCst)
}

fn write_byte(byte: u8) {
    if !is_ready() {
        return;
    }
    // Wait until the transmit holding register is empty.
    unsafe {
        while cpu::inb(LSR) & 0x20 == 0 {}
        cpu::outb(THR, byte);
    }
}

/// Zero-sized object implementing [`fmt::Write`] so `writeln!` works.
pub struct Console;

/// Get a handle for `write!`/`writeln!` output to the serial console.
pub fn stdout() -> Console {
    Console
}

impl fmt::Write for Console {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        for &byte in s.as_bytes() {
            write_byte(byte);
        }
        Ok(())
    }
}