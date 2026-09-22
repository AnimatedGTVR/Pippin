//! M4 Rust compositor: wallpaper, window stack, mouse focus, and back buffer.

use alloc::vec;
use alloc::vec::Vec;
use alloc::string::{String, ToString};
use crate::window_server::{Event, WindowServer};

pub const WIDTH: usize = 800;
pub const HEIGHT: usize = 600;
const MAX_WINDOWS: usize = 16;

#[derive(Clone)]
struct Row { text: String, action: String }

#[derive(Clone)]
struct Window {
    id: String,
    role: u8,
    x: i32,
    y: i32,
    width: i32,
    height: i32,
    title: String,
    rows: Vec<Row>,
    native: bool,
}

pub struct Compositor {
    pixels: Vec<u32>,
    clients: WindowServer,
    windows: Vec<Window>, // back to front
    next_id: u32,
    wallpaper_base: u32,
    cursor_x: i32,
    cursor_y: i32,
    left_down: bool,
    drag: Option<(String, i32, i32)>, // window ID and pointer offset
}

impl Compositor {
    pub fn new() -> Self {
        let mut compositor = Self {
            pixels: vec![0; WIDTH * HEIGHT], clients: WindowServer::new(),
            windows: Vec::new(), next_id: 1,
            wallpaper_base: 0x00142b42,
            cursor_x: (WIDTH / 2) as i32, cursor_y: (HEIGHT / 2) as i32,
            left_down: false, drag: None,
        };
        compositor.render();
        compositor
    }

    /// Native test window. Future C# clients will request windows via IPC.
    pub fn open_test_window(&mut self) {
        if self.windows.len() == MAX_WINDOWS { return; }
        let offset = self.windows.len() as i32 * 28;
        self.windows.push(Window {
            id: alloc::format!("native{}", self.next_id), role: b'W',
            x: 104 + offset, y: 88 + offset,
            width: 440, height: 300, title: "WINDOW TEST".to_string(),
            rows: vec![Row { text: "RUST COMPOSITOR".to_string(), action: String::new() },
                       Row { text: "DRAG TITLE BAR".to_string(), action: String::new() }],
            native: true,
        });
        self.next_id = self.next_id.wrapping_add(1).max(1);
        self.render();
    }

    /// A bounded host command: S|id|role|x|y|w|h|title|text@action;...
    pub fn command(&mut self, line: &str) {
        if let Some(id) = line.strip_prefix("X|") {
            self.windows.retain(|window| window.id != id);
            if id == "wallpaper" { self.wallpaper_base = 0x00142b42; }
            self.render();
            return;
        }
        let Some(payload) = line.strip_prefix("S|") else { return; };
        let mut fields = payload.splitn(8, '|');
        let (Some(id), Some(role), Some(x), Some(y), Some(width), Some(height),
             Some(title), Some(rows)) = (fields.next(), fields.next(), fields.next(),
                fields.next(), fields.next(), fields.next(), fields.next(), fields.next())
            else { return; };
        if id.is_empty() || id.len() > 32 || title.len() > 40 || rows.len() > 220 { return; }
        if role == "B" {
            if let Some(color) = title.strip_prefix('#').and_then(|hex| u32::from_str_radix(hex, 16).ok()) {
                self.wallpaper_base = color & 0x00ffffff;
                self.render();
            }
            return;
        }
        let Some(role) = role.as_bytes().first().copied() else { return; };
        if !matches!(role, b'P' | b'D' | b'L' | b'N' | b'W') { return; }
        let (Ok(x), Ok(y), Ok(width), Ok(height)) =
            (x.parse::<i32>(), y.parse::<i32>(), width.parse::<i32>(), height.parse::<i32>())
            else { return; };
        if width < 40 || height < 30 || width > WIDTH as i32 || height > HEIGHT as i32 { return; }
        let mut parsed_rows = Vec::new();
        for part in rows.split(';').take(8) {
            if part.is_empty() { continue; }
            let (text, action) = part.split_once('@').unwrap_or((part, ""));
            if text.len() > 32 || action.len() > 64 { return; }
            parsed_rows.push(Row { text: text.to_string(), action: action.to_string() });
        }
        self.windows.retain(|window| window.id != id);
        if self.windows.len() >= MAX_WINDOWS { return; }
        self.windows.push(Window { id: id.to_string(), role, x, y, width, height,
                                   title: title.to_string(), rows: parsed_rows, native: false });
        self.render();
    }

    pub fn client_command(&mut self, line: &str) -> (bool, Vec<Event>) {
        if !self.clients.validate(line) { return (false, Vec::new()); }
        let events = self.clients.command(line);
        if !line.starts_with("WB|") && !line.starts_with("WP|") { self.render(); }
        (true, events)
    }

    pub fn key_scancode(&self, scan: u8, text: Option<u8>) -> Vec<Event> {
        self.clients.key(scan, text)
    }

    pub fn has_client_windows(&self) -> bool { self.clients.is_active() }

    /// PS/2 packet: sign-extended relative movement and the primary button.
    pub fn pointer_packet(&mut self, packet: u32) -> (Option<String>, Vec<Event>) {
        let buttons = packet as u8;
        let dx = ((packet >> 8) as u8 as i8) as i32;
        let dy = ((packet >> 16) as u8 as i8) as i32;
        self.cursor_x = (self.cursor_x + dx).clamp(0, WIDTH as i32 - 1);
        self.cursor_y = (self.cursor_y - dy).clamp(0, HEIGHT as i32 - 1);
        let left = buttons & 1 != 0;
        let (handled, events) = self.clients.pointer(self.cursor_x, self.cursor_y, left, self.left_down);
        let action = if !handled && left && !self.left_down { self.press() } else { None };
        if left && !handled {
            if let Some((ref id, offset_x, offset_y)) = self.drag {
                if let Some(window) = self.windows.iter_mut().find(|window| window.id == *id) {
                    window.x = (self.cursor_x - offset_x).clamp(0, WIDTH as i32 - 40);
                    window.y = (self.cursor_y - offset_y).clamp(0, HEIGHT as i32 - 36);
                }
            }
        } else {
            self.drag = None;
        }
        self.left_down = left;
        self.render();
        (action, events)
    }

    fn press(&mut self) -> Option<String> {
        let Some(index) = self.windows.iter().rposition(|window| {
            self.cursor_x >= window.x && self.cursor_x < window.x + window.width
                && self.cursor_y >= window.y && self.cursor_y < window.y + window.height
        }) else { return None; };
        let window = self.windows.remove(index);
        if matches!(window.role, b'W' | b'L')
            && self.cursor_x >= window.x + 14 && self.cursor_x < window.x + 35
            && self.cursor_y >= window.y + 10 && self.cursor_y < window.y + 31 {
            return if window.native { None } else { Some(alloc::format!("{}.close", window.id)) };
        }
        let row_top = if matches!(window.role, b'P' | b'D') { 7 } else { 62 };
        let row_step = if matches!(window.role, b'P' | b'D') { 118 } else { 42 };
        let row = if matches!(window.role, b'P' | b'D') {
            ((self.cursor_x - window.x - 16) / row_step) as usize
        } else { ((self.cursor_y - window.y - row_top) / row_step) as usize };
        let in_rows = if matches!(window.role, b'P' | b'D') {
            self.cursor_y >= window.y + row_top && self.cursor_y < window.y + window.height - 4
        } else { self.cursor_y >= window.y + row_top };
        let action = if in_rows { window.rows.get(row).map(|row| row.action.clone()) } else { None };
        if matches!(window.role, b'W' | b'L') && self.cursor_y < window.y + 42 {
            self.drag = Some((window.id.clone(), self.cursor_x - window.x, self.cursor_y - window.y));
        }
        self.windows.push(window); // focused window becomes frontmost
        action.filter(|action| !action.is_empty())
    }

    pub fn present(&self, framebuffer: *mut u32) {
        for (offset, pixel) in self.pixels.iter().enumerate() {
            unsafe { framebuffer.add(offset).write_volatile(*pixel); }
        }
    }

    fn render(&mut self) {
        self.wallpaper();
        for index in 0..self.windows.len() {
            self.window(self.windows[index].clone());
        }
        self.clients.paint(&mut self.pixels);
        self.cursor();
    }

    fn wallpaper(&mut self) {
        for y in 0..HEIGHT {
            let t = y as u32;
            let red = ((self.wallpaper_base >> 16) & 255) + t * 22 / HEIGHT as u32;
            let green = ((self.wallpaper_base >> 8) & 255) + t * 63 / HEIGHT as u32;
            let blue = (self.wallpaper_base & 255) + t * 59 / HEIGHT as u32;
            let color = (red.min(255) << 16) | (green.min(255) << 8) | blue.min(255);
            self.pixels[y * WIDTH..(y + 1) * WIDTH].fill(color);
        }
        // Quiet geometric forms keep the wallpaper useful behind app windows.
        for y in 0..HEIGHT as i32 {
            for x in 0..WIDTH as i32 {
                if y > 390 + (x - 300).abs() / 5 {
                    self.set_pixel(x, y, 0x002f6871);
                }
                if y > 490 + (x - 610).abs() / 7 {
                    self.set_pixel(x, y, 0x003b7a7a);
                }
            }
        }
        self.fill_rect(0, HEIGHT as i32 - 12, WIDTH as i32, 12, 0x00224252);
    }

    fn window(&mut self, window: Window) {
        const INK: u32 = 0x00263743;
        const PAPER: u32 = 0x00f5f3ec;
        if matches!(window.role, b'P' | b'D' | b'N') {
            self.fill_rect(window.x + 3, window.y + 4, window.width, window.height, 0x001b3545);
            self.fill_rect(window.x, window.y, window.width, window.height, 0x00e3e9e6);
            self.border(window.x, window.y, window.width, window.height, INK);
            for (index, row) in window.rows.iter().enumerate() {
                let x = window.x + 16 + index as i32 * 118;
                if x + 90 > window.x + window.width { break; }
                if !row.action.is_empty() { self.fill_rect(x - 5, window.y + 6, 105, window.height - 12, 0x00f8f8f0); }
                self.text(x, window.y + 12, &row.text, 2, INK);
            }
            return;
        }
        self.fill_rect(window.x + 9, window.y + 11, window.width, window.height, 0x001b3545);
        self.fill_rect(window.x, window.y, window.width, window.height, PAPER);
        self.border(window.x, window.y, window.width, window.height, INK);
        self.fill_rect(window.x + 2, window.y + 2, window.width - 4, 40, 0x00dce7e8);
        self.fill_rect(window.x + 2, window.y + 41, window.width - 4, 2, INK);
        self.border(window.x + 14, window.y + 10, 21, 21, INK);
        self.text(window.x + 52, window.y + 13, &window.title, 2, INK);
        for (index, row) in window.rows.iter().enumerate() {
            let y = window.y + 64 + index as i32 * 42;
            if y + 22 > window.y + window.height { break; }
            if !row.action.is_empty() {
                self.fill_rect(window.x + 28, y - 7, window.width - 56, 32, 0x00dae8e6);
                self.border(window.x + 28, y - 7, window.width - 56, 32, 0x0084999d);
            }
            self.text(window.x + 40, y, &row.text, 2, INK);
        }
    }

    fn cursor(&mut self) {
        let x = self.cursor_x;
        let y = self.cursor_y;
        match self.clients.cursor_at(x, y) {
            1 => {
                self.fill_rect(x, y + 3, 3, 12, 0x00ffffff);
                self.fill_rect(x + 3, y + 7, 10, 8, 0x00ffffff);
                self.border(x, y + 3, 13, 12, 0x001b2b34);
                return;
            }
            2 => {
                self.fill_rect(x + 4, y, 2, 16, 0x00ffffff);
                self.fill_rect(x, y, 10, 2, 0x00ffffff);
                self.fill_rect(x, y + 14, 10, 2, 0x00ffffff);
                return;
            }
            _ => {}
        }
        for row in 0..18 {
            for column in 0..=row / 2 {
                self.set_pixel(x + column, y + row, 0x00ffffff);
            }
            self.set_pixel(x, y + row, 0x001b2b34);
            self.set_pixel(x + row / 2, y + row, 0x001b2b34);
        }
    }

    fn set_pixel(&mut self, x: i32, y: i32, color: u32) {
        if x >= 0 && y >= 0 && (x as usize) < WIDTH && (y as usize) < HEIGHT {
            self.pixels[y as usize * WIDTH + x as usize] = color;
        }
    }

    fn fill_rect(&mut self, x: i32, y: i32, width: i32, height: i32, color: u32) {
        if width <= 0 || height <= 0 { return; }
        let left = x.max(0).min(WIDTH as i32) as usize;
        let top = y.max(0).min(HEIGHT as i32) as usize;
        let right = x.saturating_add(width).max(0).min(WIDTH as i32) as usize;
        let bottom = y.saturating_add(height).max(0).min(HEIGHT as i32) as usize;
        for row in top..bottom {
            self.pixels[row * WIDTH + left..row * WIDTH + right].fill(color);
        }
    }

    fn border(&mut self, x: i32, y: i32, width: i32, height: i32, color: u32) {
        self.fill_rect(x, y, width, 2, color);
        self.fill_rect(x, y + height - 2, width, 2, color);
        self.fill_rect(x, y, 2, height, color);
        self.fill_rect(x + width - 2, y, 2, height, color);
    }

    fn text(&mut self, x: i32, y: i32, label: &str, scale: i32, color: u32) {
        for (index, byte) in label.bytes().enumerate() {
            let glyph = glyph(byte.to_ascii_uppercase());
            for (row, bits) in glyph.iter().enumerate() {
                for column in 0..5 {
                    if bits & (1 << (4 - column)) != 0 {
                        self.fill_rect(x + index as i32 * 6 * scale + column * scale,
                                       y + row as i32 * scale, scale, scale, color);
                    }
                }
            }
        }
    }
}

/// Built-in capitals for the native bootstrap window. C# clients own UI text.
pub(crate) fn glyph(byte: u8) -> [u8; 7] {
    match byte {
        b'A' => [14, 17, 17, 31, 17, 17, 17],
        b'B' => [30, 17, 17, 30, 17, 17, 30],
        b'C' => [14, 17, 16, 16, 16, 17, 14],
        b'D' => [30, 17, 17, 17, 17, 17, 30],
        b'E' => [31, 16, 16, 30, 16, 16, 31],
        b'F' => [31, 16, 16, 30, 16, 16, 16],
        b'G' => [14, 17, 16, 23, 17, 17, 14],
        b'H' => [17, 17, 17, 31, 17, 17, 17],
        b'I' => [31, 4, 4, 4, 4, 4, 31],
        b'J' => [7, 2, 2, 2, 18, 18, 12],
        b'K' => [17, 18, 20, 24, 20, 18, 17],
        b'L' => [16, 16, 16, 16, 16, 16, 31],
        b'M' => [17, 27, 21, 21, 17, 17, 17],
        b'N' => [17, 25, 21, 19, 17, 17, 17],
        b'O' => [14, 17, 17, 17, 17, 17, 14],
        b'P' => [30, 17, 17, 30, 16, 16, 16],
        b'Q' => [14, 17, 17, 17, 21, 18, 13],
        b'R' => [30, 17, 17, 30, 20, 18, 17],
        b'S' => [15, 16, 16, 14, 1, 1, 30],
        b'T' => [31, 4, 4, 4, 4, 4, 4],
        b'U' => [17, 17, 17, 17, 17, 17, 14],
        b'V' => [17, 17, 17, 17, 17, 10, 4],
        b'W' => [17, 17, 17, 21, 21, 21, 10],
        b'X' => [17, 17, 10, 4, 10, 17, 17],
        b'Y' => [17, 17, 10, 4, 4, 4, 4],
        b'Z' => [31, 1, 2, 4, 8, 16, 31],
        _ => [0; 7],
    }
}
