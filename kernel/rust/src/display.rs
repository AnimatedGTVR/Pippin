//! First Display Manager step: a Limine-provided 32-bit RGB framebuffer.

use core::fmt;

use crate::cpu;

const VGA_WIDTH: usize = 80;
const VGA_HEIGHT: usize = 25;
const VGA_MEMORY: *mut u16 = 0xb8000 as *mut u16;

pub struct TextConsole {
    row: usize,
    column: usize,
}

impl TextConsole {
    pub fn new() -> Self {
        let mut console = Self { row: 0, column: 0 };
        console.clear();
        console
    }

    pub fn clear(&mut self) {
        for cell in 0..VGA_WIDTH * VGA_HEIGHT {
            unsafe { VGA_MEMORY.add(cell).write_volatile(0x1f20); }
        }
        self.row = 0;
        self.column = 0;
        self.update_cursor();
    }

    fn put(&mut self, byte: u8) {
        match byte {
            b'\n' => { self.row += 1; self.column = 0; }
            b'\r' => self.column = 0,
            8 => {
                if self.column > 0 {
                    self.column -= 1;
                    unsafe { VGA_MEMORY.add(self.row * VGA_WIDTH + self.column)
                        .write_volatile(0x1f20); }
                }
            }
            0x20..=0x7e => {
                if self.column == VGA_WIDTH { self.row += 1; self.column = 0; }
                if self.row == VGA_HEIGHT { self.scroll(); }
                unsafe { VGA_MEMORY.add(self.row * VGA_WIDTH + self.column)
                    .write_volatile(0x1f00 | byte as u16); }
                self.column += 1;
            }
            _ => {}
        }
        if self.row == VGA_HEIGHT { self.scroll(); }
        self.update_cursor();
    }

    fn update_cursor(&self) {
        let position = (self.row * VGA_WIDTH + self.column.min(VGA_WIDTH - 1)) as u16;
        unsafe {
            cpu::outb(0x3d4, 0x0f);
            cpu::outb(0x3d5, position as u8);
            cpu::outb(0x3d4, 0x0e);
            cpu::outb(0x3d5, (position >> 8) as u8);
        }
    }

    fn scroll(&mut self) {
        for row in 1..VGA_HEIGHT {
            for column in 0..VGA_WIDTH {
                let from = row * VGA_WIDTH + column;
                let to = from - VGA_WIDTH;
                unsafe { VGA_MEMORY.add(to).write_volatile(VGA_MEMORY.add(from).read_volatile()); }
            }
        }
        for column in 0..VGA_WIDTH {
            unsafe { VGA_MEMORY.add((VGA_HEIGHT - 1) * VGA_WIDTH + column)
                .write_volatile(0x1f20); }
        }
        self.row = VGA_HEIGHT - 1;
    }
}

impl fmt::Write for TextConsole {
    fn write_str(&mut self, text: &str) -> fmt::Result {
        for byte in text.bytes() { self.put(byte); }
        Ok(())
    }
}

extern "C" {
    fn pippin_limine_framebuffer(address: *mut u64, width: *mut u64,
        height: *mut u64, pitch: *mut u64, bpp: *mut u16) -> bool;
}

pub struct Framebuffer {
    address: *mut u8,
    pub width: u64,
    pub height: u64,
    pitch: u64,
}

pub fn discover() -> Option<Framebuffer> {
    let (mut address, mut width, mut height, mut pitch, mut bpp) = (0, 0, 0, 0, 0);
    if !unsafe { pippin_limine_framebuffer(&mut address, &mut width, &mut height,
                                           &mut pitch, &mut bpp) } { return None; }
    if address == 0 || width == 0 || height == 0 || width > 8192 || height > 8192
        || bpp != 32 || pitch < width * 4 || pitch > 65536 { return None; }
    Some(Framebuffer { address: address as *mut u8, width, height, pitch })
}

impl Framebuffer {
    pub fn clear(&self) {
        for y in 0..self.height as usize {
            for x in 0..self.width as usize {
                let pixel = unsafe { self.address.add(y * self.pitch as usize + x * 4) as *mut u32 };
                unsafe { pixel.write_volatile(0x00202b45); }
            }
        }
    }
}
