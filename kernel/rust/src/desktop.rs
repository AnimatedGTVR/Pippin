//! QEMU VGA mode switch and text-console preservation for the Rust compositor.

use alloc::vec;
use alloc::vec::Vec;
use alloc::string::String;
use crate::window_server::Event;

use crate::{compositor, cpu, ffi, mm};

const WIDTH: usize = compositor::WIDTH;
const HEIGHT: usize = compositor::HEIGHT;
const BYTES: usize = WIDTH * HEIGHT * 4;
const LEGACY_VRAM_BYTES: usize = 256 * 1024;
const INDEX: u16 = 0x1ce;
const DATA: u16 = 0x1cf;

fn vbe_write(index: u16, value: u16) {
    unsafe { cpu::outw(INDEX, index); cpu::outw(DATA, value); }
}

fn vbe_read(index: u16) -> u16 {
    unsafe { cpu::outw(INDEX, index); cpu::inw(DATA) }
}

const CRTC_INDEXES: [u8; 7] = [0x01, 0x07, 0x09, 0x12, 0x13, 0x17, 0x18];
const GRAPHICS_INDEXES: [u8; 2] = [5, 6];

fn indexed_read(index_port: u16, data_port: u16, index: u8) -> u8 {
    unsafe { cpu::outb(index_port, index); cpu::inb(data_port) }
}

fn indexed_write(index_port: u16, data_port: u16, index: u8, value: u8) {
    unsafe { cpu::outb(index_port, index); cpu::outb(data_port, value); }
}

struct VgaTextState {
    crtc: [u8; CRTC_INDEXES.len()],
    graphics: [u8; GRAPHICS_INDEXES.len()],
    lock: u8,
    text: [u16; 80 * 25],
}

impl VgaTextState {
    fn save() -> Self {
        let mut state = Self {
            crtc: [0; CRTC_INDEXES.len()],
            graphics: [0; GRAPHICS_INDEXES.len()],
            lock: indexed_read(0x3d4, 0x3d5, 0x11),
            text: [0; 80 * 25],
        };
        for (slot, index) in state.crtc.iter_mut().zip(CRTC_INDEXES) {
            *slot = indexed_read(0x3d4, 0x3d5, index);
        }
        for (slot, index) in state.graphics.iter_mut().zip(GRAPHICS_INDEXES) {
            *slot = indexed_read(0x3ce, 0x3cf, index);
        }
        for (slot, offset) in state.text.iter_mut().zip(0..) {
            *slot = unsafe { (0xb8000 as *const u16).add(offset).read_volatile() };
        }
        state
    }

    fn restore(&self) {
        // VBE leaves several legacy VGA registers in graphics mode. Restore
        // the text-mode values that it changed, then repaint the saved cells.
        indexed_write(0x3d4, 0x3d5, 0x11, self.lock & !0x80);
        for (index, value) in CRTC_INDEXES.into_iter().zip(self.crtc) {
            indexed_write(0x3d4, 0x3d5, index, value);
        }
        indexed_write(0x3d4, 0x3d5, 0x11, self.lock);
        for (index, value) in GRAPHICS_INDEXES.into_iter().zip(self.graphics) {
            indexed_write(0x3ce, 0x3cf, index, value);
        }
        for (offset, cell) in self.text.iter().enumerate() {
            unsafe { (0xb8000 as *mut u16).add(offset).write_volatile(*cell); }
        }
    }
}

pub struct Desktop {
    compositor: compositor::Compositor,
    text_mode: VgaTextState,
    legacy_vram: Vec<u8>,
    framebuffer: *mut u8,
}

impl Desktop {
    pub fn open() -> Option<Self> {
        let bar = ffi::qemu_vga_bar();
        if bar == 0 || bar & 0xfff != 0 || bar > u32::MAX as u64
            || !(0xb0c0..=0xb0c5).contains(&vbe_read(0)) { return None; }

        // BAR0 is the linear framebuffer. Map only the pages used by our mode.
        for offset in (0..BYTES).step_by(4096) {
            let page = bar.checked_add(offset as u64)?;
            if page >= mm::INITIAL_MAP_SIZE { mm::map_mmio_page(page)?; }
        }

        let compositor = compositor::Compositor::new();
        let text_mode = VgaTextState::save();
        // The linear framebuffer overlaps the VGA text and font planes.
        // Preserve them so switching back does not show damaged glyphs.
        let mut legacy_vram = vec![0u8; LEGACY_VRAM_BYTES];
        for (offset, byte) in legacy_vram.iter_mut().enumerate() {
            *byte = unsafe { (bar as *const u8).add(offset).read_volatile() };
        }

        vbe_write(4, 0); // disable while changing the mode
        vbe_write(1, WIDTH as u16);
        vbe_write(2, HEIGHT as u16);
        vbe_write(3, 32);
        vbe_write(4, 0xc1); // enabled + linear framebuffer, preserve VGA memory
        if vbe_read(1) != WIDTH as u16 || vbe_read(2) != HEIGHT as u16
            || vbe_read(3) != 32 {
            vbe_write(4, 0);
            text_mode.restore();
            return None;
        }

        compositor.present(bar as *mut u32);
        Some(Self { compositor, text_mode, legacy_vram,
                    framebuffer: bar as *mut u8 })
    }

    pub fn open_test_window(&mut self) {
        self.compositor.open_test_window();
        self.compositor.present(self.framebuffer as *mut u32);
    }

    pub fn command(&mut self, line: &str) -> (bool, Vec<Event>) {
        if line.starts_with('W') {
            let (accepted, events) = self.compositor.client_command(line);
            if !line.starts_with("WB|") && !line.starts_with("WP|") {
                self.compositor.present(self.framebuffer as *mut u32);
            }
            (accepted, events)
        } else {
            self.compositor.command(line);
            self.compositor.present(self.framebuffer as *mut u32);
            (true, Vec::new())
        }
    }

    pub fn key_scancode(&self, scan: u8, text: Option<u8>) -> Vec<Event> {
        self.compositor.key_scancode(scan, text)
    }

    pub fn has_client_windows(&self) -> bool { self.compositor.has_client_windows() }

    pub fn pointer_packet(&mut self, packet: u32) -> (Option<String>, Vec<Event>) {
        let output = self.compositor.pointer_packet(packet);
        self.compositor.present(self.framebuffer as *mut u32);
        output
    }
}

impl Drop for Desktop {
    fn drop(&mut self) {
        vbe_write(4, 0);
        for (offset, byte) in self.legacy_vram.iter().enumerate() {
            unsafe { self.framebuffer.add(offset).write_volatile(*byte); }
        }
        self.text_mode.restore();
    }
}
