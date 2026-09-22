//! Bounded line protocol for the host C# shell on QEMU's second UART.

use crate::serial;
use crate::window_server::Event;

const MAX_LINE: usize = 384;

pub struct Bridge {
    line: [u8; MAX_LINE],
    len: usize,
    overflow: bool,
}

impl Bridge {
    pub fn new() -> Self {
        serial::init_port(serial::COM2);
        Self { line: [0; MAX_LINE], len: 0, overflow: false }
    }

    pub fn poll(&mut self, mut command: impl FnMut(&str) -> bool) {
        for _ in 0..128 {
            let Some(byte) = serial::try_read_port(serial::COM2) else { break; };
            if byte == b'\n' {
                if !self.overflow {
                    if let Ok(line) = core::str::from_utf8(&self.line[..self.len]) {
                        if line == "H|1" { serial::write_port(serial::COM2, b"R|1\n"); }
                        else if line == "H|2" { serial::write_port(serial::COM2, b"R|2\n"); }
                        else if command(line) { serial::write_port(serial::COM2, b"A\n"); }
                        else { serial::write_port(serial::COM2, b"N\n"); }
                    }
                }
                self.len = 0;
                self.overflow = false;
            } else if byte != b'\r' && !self.overflow {
                if self.len < MAX_LINE { self.line[self.len] = byte; self.len += 1; }
                else { self.overflow = true; }
            }
        }
    }

    pub fn action(&self, action: &str) {
        serial::write_port(serial::COM2, b"E|");
        serial::write_port(serial::COM2, action.as_bytes());
        serial::write_port(serial::COM2, b"\n");
    }

    pub fn event(&self, event: Event) {
        let line = alloc::format!("EV|{}|{}|{}|{}\n", event.id, event.kind, event.a, event.b);
        serial::write_port(serial::COM2, line.as_bytes());
    }
}
