//! M4 Rust compositor: wallpaper, window stack, mouse focus, and back buffer.

use alloc::vec;
use alloc::vec::Vec;
use alloc::string::{String, ToString};
use crate::window_server::{Event, WindowServer};
use crate::ffi;

pub const WIDTH: usize = 1024;
pub const HEIGHT: usize = 768;
const MAX_WINDOWS: usize = 16;

// Pippin chrome: compact GNOME-like header bars, Skift-inspired soft surfaces,
// and the deliberately simple geometry of Redox/Orbital.
const CHROME_INK: u32 = 0x00252b31;
const CHROME_SURFACE: u32 = 0x00f7f7f5;
const CHROME_HEADER: u32 = 0x00e9ecef;
const CHROME_HEADER_INACTIVE: u32 = 0x00dfe3e6;
const CHROME_BORDER: u32 = 0x009aa2a8;
const CHROME_ACCENT: u32 = 0x003d78a8;
const CHROME_CLOSE: u32 = 0x00d95d55;
const CHROME_SHADOW: u32 = 0x00151b22;
const SHELL_DARK: u32 = 0x0014191f;
const SHELL_SURFACE: u32 = 0x00222931;
const SHELL_SURFACE_HOVER: u32 = 0x002d3540;
const SHELL_BORDER: u32 = 0x00414b57;
const SHELL_TEXT: u32 = 0x00f3f5f7;
const SHELL_MUTED: u32 = 0x00aeb8c2;
const HEADER_HEIGHT: i32 = 44;

#[derive(Clone)]
struct Row { text: String, action: String, kind: u8 }

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
    maximized: bool,
    minimized: bool,
    restore: Option<(i32, i32, i32, i32)>,
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
            wallpaper_base: 0x002f80ed,
            cursor_x: (WIDTH / 2) as i32, cursor_y: (HEIGHT / 2) as i32,
            left_down: false, drag: None,
        };

        // The visible shell model now comes from C++ through the tiny C ABI.
        // Rust remains responsible for validation, ownership and rendering.
        if ffi::shell_abi_version() == 1 {
            for index in 0..ffi::shell_surface_count().min(MAX_WINDOWS) {
                let Some(surface) = ffi::shell_surface(index) else { continue; };
                if !matches!(surface.role, b'P' | b'D' | b'L' | b'N' | b'W') {
                    continue;
                }
                if surface.width < 40 || surface.height < 30
                    || surface.width > WIDTH as i32 || surface.height > HEIGHT as i32 {
                    continue;
                }

                let mut rows = Vec::new();
                for raw in surface.items.iter().take(8) {
                    let item = ffi::shell_item(raw);
                    rows.push(Row {
                        text: item.text.to_string(),
                        action: item.action.to_string(),
                        kind: item.kind,
                    });
                }

                compositor.windows.push(Window {
                    id: surface.id.to_string(),
                    role: surface.role,
                    x: surface.x,
                    y: surface.y,
                    width: surface.width,
                    height: surface.height,
                    title: surface.title.to_string(),
                    rows,
                    native: false,
                    maximized: false,
                    minimized: false,
                    restore: None,
                });
            }
        }

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
            rows: vec![Row { text: "RUST COMPOSITOR".to_string(), action: String::new(), kind: b'l' },
                       Row { text: "DRAG TITLE BAR".to_string(), action: String::new(), kind: b'l' }],
            native: true, maximized: false, minimized: false, restore: None,
        });
        self.next_id = self.next_id.wrapping_add(1).max(1);
        self.render();
    }

    /// A bounded host command: S|id|role|x|y|w|h|title|text@action;...
    /// Returns false when the payload is rejected so the bridge can report a
    /// protocol error instead of silently acknowledging a surface that was
    /// never created.
    pub fn command(&mut self, line: &str) -> bool {
        if let Some(id) = line.strip_prefix("R|") {
            if let Some(index) = self.windows.iter().position(|window| window.id == id) {
                let mut window = self.windows.remove(index);
                window.minimized = false;
                self.windows.push(window);
                self.render();
            }
            return true;
        }
        if let Some(id) = line.strip_prefix("X|") {
            self.windows.retain(|window| window.id != id);
            if id == "wallpaper" { self.wallpaper_base = 0x002f80ed; }
            self.render();
            return true;
        }
        let Some(payload) = line.strip_prefix("S|") else { return false; };
        let mut fields = payload.splitn(8, '|');
        let (Some(id), Some(role), Some(x), Some(y), Some(width), Some(height),
             Some(title), Some(rows)) = (fields.next(), fields.next(), fields.next(),
                fields.next(), fields.next(), fields.next(), fields.next(), fields.next())
            else { return false; };

        // Keep the whole command safely under the COM2 bridge's 384-byte line
        // limit, but allow useful terminal/status text instead of the old
        // 32-character row ceiling.
        if id.is_empty() || id.len() > 32 || title.len() > 40 || rows.len() > 320 { return false; }

        if role == "B" {
            let Some(color) = title.strip_prefix('#')
                .and_then(|hex| u32::from_str_radix(hex, 16).ok()) else { return false; };
            self.wallpaper_base = color & 0x00ffffff;
            self.render();
            return true;
        }

        let Some(role) = role.as_bytes().first().copied() else { return false; };
        if !matches!(role, b'P' | b'D' | b'L' | b'N' | b'W') { return false; }
        let (Ok(x), Ok(y), Ok(width), Ok(height)) =
            (x.parse::<i32>(), y.parse::<i32>(), width.parse::<i32>(), height.parse::<i32>())
            else { return false; };
        if width < 40 || height < 30 || width > WIDTH as i32 || height > HEIGHT as i32 { return false; }

        let mut parsed_rows = Vec::new();
        for part in rows.split(';').take(8) {
            if part.is_empty() { continue; }
            let (text, action) = part.split_once('@').unwrap_or((part, ""));
            if text.len() > 96 || action.len() > 64 { return false; }
            let (kind, text) = text.split_once(':').map(|(kind, value)|
                (kind.as_bytes().first().copied().unwrap_or(b'l'), value)).unwrap_or((b'l', text));
            parsed_rows.push(Row { text: text.to_string(), action: action.to_string(), kind });
        }

        if let Some(index) = self.windows.iter().position(|window| window.id == id) {
            let mut window = self.windows.remove(index);
            window.role = role;
            window.x = x;
            window.y = y;
            window.width = width;
            window.height = height;
            window.title = title.to_string();
            window.rows = parsed_rows;
            window.minimized = false;
            self.windows.push(window);
        } else {
            if self.windows.len() >= MAX_WINDOWS { return false; }
            self.windows.push(Window { id: id.to_string(), role, x, y, width, height,
                                       title: title.to_string(), rows: parsed_rows, native: false,
                                       maximized: false, minimized: false, restore: None });
        }
        self.render();
        true
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
            !window.minimized && self.cursor_x >= window.x && self.cursor_x < window.x + window.width
                && self.cursor_y >= window.y && self.cursor_y < window.y + window.height
        }) else { return None; };
        let mut window = self.windows.remove(index);
        if matches!(window.role, b'W' | b'L') && self.cursor_y >= window.y + 14 && self.cursor_y < window.y + 32 {
            let control = self.cursor_x - (window.x + 15);
            if (0..18).contains(&control) {
                return if window.native { None } else { Some(alloc::format!("{}.close", window.id)) };
            }
            if (29..47).contains(&control) {
                window.minimized = true;
                self.windows.push(window);
                return None;
            }
            if (58..76).contains(&control) {
                if window.maximized {
                    if let Some((x, y, width, height)) = window.restore.take() {
                        window.x = x; window.y = y; window.width = width; window.height = height;
                    }
                    window.maximized = false;
                } else {
                    window.restore = Some((window.x, window.y, window.width, window.height));
                    window.x = 8; window.y = 58;
                    window.width = WIDTH as i32 - 16; window.height = HEIGHT as i32 - 116;
                    window.maximized = true;
                }
                self.windows.push(window);
                return None;
            }
        }
        let (row, in_rows) = if window.role == b'D' {
            let local_x = self.cursor_x - window.x;
            let local_y = self.cursor_y - window.y;
            let row = if (12..=116).contains(&local_x) { 0 }
                else if (132..=236).contains(&local_x) { 1 }
                else if (252..=356).contains(&local_x) { 2 }
                else { usize::MAX };
            (row, (8..60).contains(&local_y))
        } else if window.role == b'P' {
            let local_x = self.cursor_x - window.x;
            let local_y = self.cursor_y - window.y;
            let row = if (12..=210).contains(&local_x) { 0 }
                else if (420..=604).contains(&local_x) { 1 }
                else if (774..=1012).contains(&local_x) { 2 }
                else { usize::MAX };
            (row, (6..42).contains(&local_y))
        } else {
            let row_top = 62;
            (((self.cursor_y - window.y - row_top) / 42) as usize,
             self.cursor_y >= window.y + row_top)
        };
        let action = if in_rows { window.rows.get(row).map(|row| row.action.clone()) } else { None };
        if matches!(window.role, b'W' | b'L') && !window.maximized
            && self.cursor_y < window.y + HEADER_HEIGHT {
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
        let focused = self.windows.iter().rposition(|window| !window.minimized);
        for index in 0..self.windows.len() {
            if !self.windows[index].minimized {
                self.window(self.windows[index].clone(), focused == Some(index));
            }
        }
        self.clients.paint(&mut self.pixels);
        self.cursor();
    }

    fn wallpaper(&mut self) {
        // The desktop background is intentionally simple: one clean Pippin blue.
        // Shell surfaces, windows, controls, dock, launcher and notifications provide
        // the visual structure instead of baking decoration into the wallpaper.
        self.pixels.fill(self.wallpaper_base);
    }

    fn window(&mut self, window: Window, focused: bool) {
        if window.role == b'P' {
            // Hideo-inspired taskbar: one quiet dark strip with three clear
            // zones instead of a row of debug buttons.
            self.fill_rect(window.x, window.y, window.width, window.height, SHELL_DARK);
            self.fill_rect(window.x, window.y + window.height - 1, window.width, 1, SHELL_BORDER);

            // Left search pill.
            self.rounded_rect(window.x + 12, window.y + 7, 198, 34, SHELL_SURFACE, SHELL_BORDER);
            self.fill_rect(window.x + 28, window.y + 18, 10, 10, CHROME_ACCENT);
            if let Some(row) = window.rows.get(0) {
                self.text(window.x + 48, window.y + 17, &row.text, 1, SHELL_TEXT);
            }

            // Center date/time/identity pill.
            self.rounded_rect(window.x + 420, window.y + 7, 184, 34, SHELL_SURFACE, SHELL_BORDER);
            if let Some(row) = window.rows.get(1) {
                let text_width = row.text.len() as i32 * 6;
                self.text(window.x + 512 - text_width / 2, window.y + 17, &row.text, 1, SHELL_TEXT);
            }

            // Right system status pill.
            self.rounded_rect(window.x + 774, window.y + 7, 238, 34, SHELL_SURFACE, SHELL_BORDER);
            self.fill_rect(window.x + 790, window.y + 18, 10, 8, 0x005bc0eb);
            self.fill_rect(window.x + 808, window.y + 16, 6, 12, 0x00d5dae0);
            self.fill_rect(window.x + 822, window.y + 17, 16, 10, 0x0089d185);
            if let Some(row) = window.rows.get(2) {
                self.text(window.x + 850, window.y + 17, &row.text, 1, SHELL_MUTED);
            }
            return;
        }

        if window.role == b'D' {
            // Floating launcher dock. The stepped rounded rectangles are the
            // bootstrap rasterizer's approximation of Hideo's soft radii.
            self.rounded_rect(window.x + 5, window.y + 7, window.width, window.height,
                              0x0010151a, 0x0010151a);
            self.rounded_rect(window.x, window.y, window.width, window.height,
                              SHELL_DARK, SHELL_BORDER);

            for (index, row) in window.rows.iter().take(3).enumerate() {
                let tile_x = window.x + 12 + index as i32 * 120;
                let tile_y = window.y + 8;
                self.rounded_rect(tile_x, tile_y, 104, 52, SHELL_SURFACE, SHELL_BORDER);

                // App-icon tile.
                let icon_x = tile_x + 10;
                let icon_y = tile_y + 11;
                let icon_color = match index {
                    0 => 0x004f8fdc,
                    1 => 0x0057a773,
                    _ => 0x00886bd8,
                };
                self.rounded_rect(icon_x, icon_y, 30, 30, icon_color, icon_color);
                let glyph = match index { 0 => "A", 1 => "F", _ => "S" };
                self.text(icon_x + 9, icon_y + 9, glyph, 1, 0x00ffffff);

                self.text(tile_x + 49, tile_y + 20, &row.text, 1, SHELL_TEXT);
            }
            return;
        }

        if window.role == b'N' {
            self.rounded_rect(window.x + 4, window.y + 5, window.width, window.height,
                              CHROME_SHADOW, CHROME_SHADOW);
            self.rounded_rect(window.x, window.y, window.width, window.height,
                              SHELL_DARK, SHELL_BORDER);
            for (index, row) in window.rows.iter().enumerate() {
                self.text(window.x + 18, window.y + 18 + index as i32 * 18,
                          &row.text, 1, SHELL_TEXT);
            }
            return;
        }

        // Layered shadow gives depth without the old heavy black frame.
        self.fill_rect(window.x + 8, window.y + 10, window.width, window.height, CHROME_SHADOW);
        self.fill_rect(window.x + 4, window.y + 5, window.width, window.height, 0x00323a42);
        self.fill_rect(window.x, window.y, window.width, window.height, CHROME_SURFACE);
        self.border(window.x, window.y, window.width, window.height, CHROME_BORDER);

        let header = if focused { CHROME_HEADER } else { CHROME_HEADER_INACTIVE };
        self.fill_rect(window.x + 1, window.y + 1, window.width - 2, HEADER_HEIGHT, header);
        self.fill_rect(window.x + 1, window.y + HEADER_HEIGHT, window.width - 2, 1, 0x00c7cdd1);

        // Compact circular-ish controls. With the bootstrap rasterizer these are stepped,
        // which gives Pippin its own identity instead of cloning any source desktop.
        let close_x = window.x + 15;
        let control_y = window.y + 14;
        self.fill_rect(close_x + 3, control_y, 12, 18, CHROME_CLOSE);
        self.fill_rect(close_x, control_y + 3, 18, 12, CHROME_CLOSE);
        self.fill_rect(close_x + 5, control_y + 5, 8, 8, 0x00f7d8d5);

        let min_x = close_x + 29;
        self.fill_rect(min_x + 3, control_y, 12, 18, 0x00c6ccd0);
        self.fill_rect(min_x, control_y + 3, 18, 12, 0x00c6ccd0);
        self.fill_rect(min_x + 5, control_y + 8, 8, 2, 0x00656d73);

        let max_x = min_x + 29;
        self.fill_rect(max_x + 3, control_y, 12, 18, 0x00c6ccd0);
        self.fill_rect(max_x, control_y + 3, 18, 12, 0x00c6ccd0);
        self.border(max_x + 5, control_y + 5, 8, 8, 0x00656d73);

        // GNOME-like centered title treatment while retaining Pippin's tiny built-in font.
        let title_width = window.title.len() as i32 * 12;
        let title_x = window.x + ((window.width - title_width) / 2).max(92);
        self.text(title_x, window.y + 15, &window.title, 2,
                  if focused { CHROME_INK } else { 0x006f777d });

        for (index, row) in window.rows.iter().enumerate() {
            let y = window.y + 68 + index as i32 * 42;
            if y + 22 > window.y + window.height { break; }
            match row.kind {
                b'h' => {
                    self.text(window.x + 28, y, &row.text, 2, CHROME_INK);
                    self.fill_rect(window.x + 28, y + 20, window.width - 56, 1, 0x00d5d9dc);
                }
                b's' => {
                    self.fill_rect(window.x + 24, y - 8, window.width - 48, 34, 0x00ffffff);
                    self.border(window.x + 24, y - 8, window.width - 48, 34,
                                if focused { CHROME_ACCENT } else { 0x00b7bec3 });
                    self.text(window.x + 38, y, &row.text, 2, 0x00777f85);
                }
                b't' => {
                    self.text(window.x + 38, y, &row.text, 2, CHROME_INK);
                    let tx = window.x + window.width - 78;
                    self.fill_rect(tx, y - 5, 38, 20, 0x00c8cdd1);
                    self.fill_rect(tx + 3, y - 2, 14, 14, 0x00ffffff);
                }
                _ => {
                    if !row.action.is_empty() {
                        self.fill_rect(window.x + 24, y - 8, window.width - 48, 34, 0x00ffffff);
                        self.border(window.x + 24, y - 8, window.width - 48, 34,
                                    if focused { CHROME_ACCENT } else { 0x00c2c8cc });
                    }
                    self.text(window.x + 38, y, &row.text, 2, CHROME_INK);
                }
            }
        }
    }

    fn cursor(&mut self) {
        let x = self.cursor_x;
        let y = self.cursor_y;
        match self.clients.cursor_at(x, y) {
            1 => {
                // Hand pointer: larger and outlined so it stays readable on both
                // the blue desktop and light application surfaces.
                self.fill_rect(x + 5, y + 2, 4, 16, 0x0019232d);
                self.fill_rect(x + 9, y + 8, 11, 11, 0x0019232d);
                self.fill_rect(x + 7, y + 4, 2, 12, 0x00ffffff);
                self.fill_rect(x + 10, y + 10, 8, 7, 0x00ffffff);
                return;
            }
            2 => {
                // Text caret cursor.
                self.fill_rect(x + 5, y, 3, 22, 0x0019232d);
                self.fill_rect(x, y, 13, 3, 0x0019232d);
                self.fill_rect(x, y + 19, 13, 3, 0x0019232d);
                self.fill_rect(x + 6, y + 2, 1, 18, 0x00ffffff);
                return;
            }
            _ => {}
        }

        // Pippin arrow cursor: a proper 24px white pointer with a dark outline
        // and small shadow, replacing the old skinny triangular cursor.
        const CURSOR: [&str; 24] = [
            "##......................",
            "#W#.....................",
            "#WW#....................",
            "#WWW#...................",
            "#WWWW#..................",
            "#WWWWW#.................",
            "#WWWWWW#................",
            "#WWWWWWW#...............",
            "#WWWWWWWW#..............",
            "#WWWWWWWWW#.............",
            "#WWWWWWWWWW#............",
            "#WWWWWWWWWWW#...........",
            "#WWWWWW#######..........",
            "#WWW#WW#.................",
            "#WW#.#WW#................",
            "#W#..#WW#................",
            "##....#WW#...............",
            "......#WW#...............",
            ".......#WW#..............",
            ".......#WW#..............",
            "........##...............",
            "........................",
            "........................",
            "........................",
        ];
        for (dy, row) in CURSOR.iter().enumerate() {
            for (dx, pixel) in row.bytes().enumerate() {
                match pixel {
                    b'#' => self.set_pixel(x + dx as i32, y + dy as i32, 0x0019232d),
                    b'W' => self.set_pixel(x + dx as i32, y + dy as i32, 0x00ffffff),
                    _ => {}
                }
            }
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

    fn rounded_rect(&mut self, x: i32, y: i32, width: i32, height: i32,
                    fill: u32, border: u32) {
        if width < 8 || height < 8 {
            self.fill_rect(x, y, width, height, fill);
            self.border(x, y, width, height, border);
            return;
        }

        // 4 px stepped radius: deliberately simple enough for the bootstrap
        // software rasterizer while reading much softer than a hard box.
        self.fill_rect(x + 4, y, width - 8, 1, border);
        self.fill_rect(x + 2, y + 1, width - 4, 1, border);
        self.fill_rect(x + 1, y + 2, width - 2, 1, border);
        self.fill_rect(x, y + 3, width, height - 6, border);
        self.fill_rect(x + 1, y + height - 3, width - 2, 1, border);
        self.fill_rect(x + 2, y + height - 2, width - 4, 1, border);
        self.fill_rect(x + 4, y + height - 1, width - 8, 1, border);

        self.fill_rect(x + 4, y + 1, width - 8, height - 2, fill);
        self.fill_rect(x + 2, y + 2, width - 4, height - 4, fill);
        self.fill_rect(x + 1, y + 3, width - 2, height - 6, fill);
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
