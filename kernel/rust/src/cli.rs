//! M4 bootstrap command shell over VGA text and the serial terminal.

use core::fmt::{self, Write};
use alloc::vec::Vec;
use crate::window_server::Event;

use crate::{bridge, desktop, display, ffi, file, interrupts, mem, serial};

const LINE_CAPACITY: usize = 128;

pub struct Terminal {
    screen: Option<display::TextConsole>,
    screen_enabled: bool,
}

impl Terminal {
    pub fn new(vga: bool) -> Self {
        Self { screen: if vga { Some(display::TextConsole::new()) } else { None },
               screen_enabled: true }
    }

    fn set_screen_enabled(&mut self, enabled: bool) { self.screen_enabled = enabled; }

    fn clear(&mut self) {
        if self.screen_enabled {
            if let Some(screen) = &mut self.screen { screen.clear(); }
        }
        let _ = serial::stdout().write_str("\x1b[2J\x1b[H");
    }

    fn backspace(&mut self) {
        if self.screen_enabled {
            if let Some(screen) = &mut self.screen { let _ = screen.write_str("\x08"); }
        }
        let _ = serial::stdout().write_str("\x08 \x08");
    }
}

impl Write for Terminal {
    fn write_str(&mut self, text: &str) -> fmt::Result {
        serial::stdout().write_str(text)?;
        if self.screen_enabled {
            if let Some(screen) = &mut self.screen { screen.write_str(text)?; }
        }
        Ok(())
    }
}

pub struct Shell {
    terminal: Terminal,
    line: [u8; LINE_CAPACITY],
    len: usize,
    shift: bool,
    caps: bool,
    extended: bool,
    last_was_cr: bool,
    bundle: Option<file::Bundle<'static>>,
    desktop: Option<desktop::Desktop>,
    bridge: bridge::Bridge,
}

impl Shell {
    pub fn new(terminal: Terminal, bundle: Option<file::Bundle<'static>>) -> Self {
        let mut shell = Self {
            terminal, line: [0; LINE_CAPACITY], len: 0,
            shift: false, caps: false, extended: false, last_was_cr: false,
            bundle, desktop: None, bridge: bridge::Bridge::new(),
        };
        let _ = writeln!(shell.terminal, "Pippin command shell (M4 preview)");
        let _ = writeln!(shell.terminal, "Type 'help' for commands.\n");
        shell.prompt();
        shell
    }

    fn prompt(&mut self) { let _ = self.terminal.write_str("pippin> "); }

    pub fn poll_serial(&mut self) {
        for _ in 0..32 {
            let Some(byte) = serial::try_read() else { break; };
            if byte == b'\n' && self.last_was_cr { self.last_was_cr = false; continue; }
            self.last_was_cr = byte == b'\r';
            self.input(byte);
        }
    }

    pub fn poll_bridge(&mut self) {
        let desktop = &mut self.desktop;
        let mut opened = false;
        let mut events: Vec<Event> = Vec::new();
        self.bridge.poll(|line| {
            if (line.starts_with("S|") || line.starts_with("WC|")) && desktop.is_none() {
                *desktop = desktop::Desktop::open();
                opened = desktop.is_some();
            }
            if let Some(desktop) = desktop {
                let (accepted, output) = desktop.command(line);
                events.extend(output);
                accepted
            } else { false }
        });
        if opened { self.terminal.set_screen_enabled(false); }
        for event in events { self.bridge.event(event); }
    }

    pub fn key_scancode(&mut self, scan: u8) {
        if self.desktop.is_some() {
            let client_active = self.desktop.as_ref().is_some_and(|desktop| desktop.has_client_windows());
            if scan == 0x01 && !client_active {
                self.desktop.take();
                self.terminal.set_screen_enabled(true);
                let _ = writeln!(self.terminal, "\nReturned to the command shell.");
                self.prompt();
            } else if scan == 0x31 && !client_active {
                if let Some(desktop) = &mut self.desktop { desktop.open_test_window(); }
            }
            if scan == 0x2a || scan == 0x36 { self.shift = true; }
            if scan == 0xaa || scan == 0xb6 { self.shift = false; }
            if scan == 0x3a { self.caps = !self.caps; }
            let text = if scan & 0x80 == 0 {
                key_pair(scan).map(|(plain, shifted)| {
                    if plain.is_ascii_alphabetic() {
                        if self.shift ^ self.caps { plain.to_ascii_uppercase() } else { plain }
                    } else if self.shift { shifted } else { plain }
                })
            } else { None };
            if let Some(desktop) = &self.desktop {
                for event in desktop.key_scancode(scan, text) { self.bridge.event(event); }
            }
            return;
        }
        if scan == 0xe0 { self.extended = true; return; }
        if self.extended { self.extended = false; return; }
        match scan {
            0x2a | 0x36 => { self.shift = true; return; }
            0xaa | 0xb6 => { self.shift = false; return; }
            0x3a => { self.caps = !self.caps; return; }
            _ if scan & 0x80 != 0 => return,
            _ => {}
        }
        let Some((plain, shifted)) = key_pair(scan) else { return; };
        let byte = if plain.is_ascii_alphabetic() {
            if self.shift ^ self.caps { plain.to_ascii_uppercase() } else { plain }
        } else if self.shift { shifted } else { plain };
        self.input(byte);
    }

    pub fn mouse_packet(&mut self, packet: u32) {
        if let Some(desktop) = &mut self.desktop {
            let (action, events) = desktop.pointer_packet(packet);
            if let Some(action) = action { self.bridge.action(&action); }
            for event in events { self.bridge.event(event); }
        }
    }

    fn input(&mut self, byte: u8) {
        match byte {
            b'\r' | b'\n' => {
                let _ = self.terminal.write_str("\n");
                let mut line = [0u8; LINE_CAPACITY];
                line[..self.len].copy_from_slice(&self.line[..self.len]);
                let command = core::str::from_utf8(&line[..self.len]).unwrap_or("").trim();
                self.execute(command);
                self.len = 0;
                if self.desktop.is_none() { self.prompt(); }
            }
            8 | 127 if self.len > 0 => {
                self.len -= 1;
                self.terminal.backspace();
            }
            0x20..=0x7e if self.len < LINE_CAPACITY - 1 => {
                self.line[self.len] = byte;
                self.len += 1;
                let _ = self.terminal.write_char(byte as char);
            }
            _ => {}
        }
    }

    fn execute(&mut self, command: &str) {
        match command {
            "" => {}
            "help" => {
                let _ = writeln!(self.terminal,
                    "help  fetch  desktop  console  clear  uname  uptime  mem  pci  ls  cat  echo");
            }
            "desktop" => {
                if self.desktop.is_some() {
                    let _ = writeln!(self.terminal, "Desktop preview is already open.");
                } else {
                    let _ = writeln!(self.terminal, "Opening desktop preview; press Esc to return.");
                    self.desktop = desktop::Desktop::open();
                    if self.desktop.is_none() {
                        let _ = writeln!(self.terminal, "QEMU standard VGA framebuffer unavailable.");
                    } else {
                        self.terminal.set_screen_enabled(false);
                    }
                }
            }
            "console" => {
                self.desktop.take();
                self.terminal.set_screen_enabled(true);
                let _ = writeln!(self.terminal, "Command shell active.");
            }
            "fetch" | "neofetch" => self.fetch(),
            "clear" => self.terminal.clear(),
            "uname" => { let _ = writeln!(self.terminal, "Pippin OS 0.1.0 x86_64"); }
            "uptime" => {
                let ticks = interrupts::ticks();
                let _ = writeln!(self.terminal, "{}.{:03} seconds", ticks / 1000, ticks % 1000);
            }
            "mem" => {
                let frames = mem::free_frame_count();
                let _ = writeln!(self.terminal, "{} free frames ({} KiB)", frames, frames * 4);
            }
            "pci" => {
                let count = ffi::pci_device_count();
                let _ = writeln!(self.terminal, "{} PCI devices:", count);
                for index in 0..count.min(16) {
                    if let Some((vendor, product, class, subclass)) = ffi::pci_device(index) {
                        let _ = writeln!(self.terminal, "  {:02} {:04x}:{:04x} class {:02x}:{:02x} parent {}",
                            index, vendor, product, class, subclass, ffi::pci_parent(index));
                    }
                }
            }
            "ls" | "ls /apps" => {
                if self.bundle.is_some() {
                    let _ = writeln!(self.terminal, "hello.pipb");
                } else {
                    let _ = writeln!(self.terminal, "No app bundle loaded.");
                }
            }
            "cat hello.pipb" | "cat /apps/hello.pipb" => {
                if let Some(bundle) = self.bundle {
                    let _ = writeln!(self.terminal, "PIPPIN-BUNDLE/1\nname={}\nmessage={}",
                        bundle.name, bundle.message);
                } else {
                    let _ = writeln!(self.terminal, "cat: hello.pipb: not found");
                }
            }
            _ if command.starts_with("echo ") => {
                let _ = writeln!(self.terminal, "{}", &command[5..]);
            }
            _ => { let _ = writeln!(self.terminal, "{}: command not found", command); }
        }
    }

    fn fetch(&mut self) {
        let ticks = interrupts::ticks();
        let free_kib = mem::free_frame_count() * 4;
        let bundle = self.bundle.map(|bundle| bundle.name).unwrap_or("none");
        let _ = writeln!(self.terminal, "   .------.     Pippin OS 0.1.0");
        let _ = writeln!(self.terminal, "  /  .--.  \\    Architecture: x86_64");
        let _ = writeln!(self.terminal, " |  /    \\  |   Uptime: {}.{:03} s",
            ticks / 1000, ticks % 1000);
        let _ = writeln!(self.terminal, " |  \\____/  |   Free memory: {} KiB", free_kib);
        let _ = writeln!(self.terminal, "  \\        /    PCI devices: {}", ffi::pci_device_count());
        let _ = writeln!(self.terminal, "   '------'     App bundle: {}", bundle);
    }
}

/// PS/2 set-1 US keyboard map for the shell's printable keys.
fn key_pair(scan: u8) -> Option<(u8, u8)> {
    Some(match scan {
        0x02 => (b'1', b'!'), 0x03 => (b'2', b'@'), 0x04 => (b'3', b'#'),
        0x05 => (b'4', b'$'), 0x06 => (b'5', b'%'), 0x07 => (b'6', b'^'),
        0x08 => (b'7', b'&'), 0x09 => (b'8', b'*'), 0x0a => (b'9', b'('),
        0x0b => (b'0', b')'), 0x0c => (b'-', b'_'), 0x0d => (b'=', b'+'),
        0x0e => (8, 8), 0x10 => (b'q', b'Q'), 0x11 => (b'w', b'W'),
        0x12 => (b'e', b'E'), 0x13 => (b'r', b'R'), 0x14 => (b't', b'T'),
        0x15 => (b'y', b'Y'), 0x16 => (b'u', b'U'), 0x17 => (b'i', b'I'),
        0x18 => (b'o', b'O'), 0x19 => (b'p', b'P'), 0x1a => (b'[', b'{'),
        0x1b => (b']', b'}'), 0x1c => (b'\n', b'\n'),
        0x1e => (b'a', b'A'), 0x1f => (b's', b'S'), 0x20 => (b'd', b'D'),
        0x21 => (b'f', b'F'), 0x22 => (b'g', b'G'), 0x23 => (b'h', b'H'),
        0x24 => (b'j', b'J'), 0x25 => (b'k', b'K'), 0x26 => (b'l', b'L'),
        0x27 => (b';', b':'), 0x28 => (b'\'', b'"'), 0x29 => (b'`', b'~'),
        0x2b => (b'\\', b'|'), 0x2c => (b'z', b'Z'), 0x2d => (b'x', b'X'),
        0x2e => (b'c', b'C'), 0x2f => (b'v', b'V'), 0x30 => (b'b', b'B'),
        0x31 => (b'n', b'N'), 0x32 => (b'm', b'M'), 0x33 => (b',', b'<'),
        0x34 => (b'.', b'>'), 0x35 => (b'/', b'?'), 0x39 => (b' ', b' '),
        _ => return None,
    })
}
