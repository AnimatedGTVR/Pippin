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
const SHELL_SURFACE_PRESSED: u32 = 0x00384452;
const SHELL_DISABLED: u32 = 0x0020272e;
const CONTROL_LIGHT_HOVER: u32 = 0x00eef4f8;
const CONTROL_LIGHT_PRESSED: u32 = 0x00dce8f2;
const CONTROL_LIGHT_DISABLED: u32 = 0x00eef0f1;
const SHELL_BORDER: u32 = 0x00414b57;
const SHELL_TEXT: u32 = 0x00f3f5f7;
const SHELL_MUTED: u32 = 0x00aeb8c2;
const CONTROL_STYLE_SUBTLE: u8 = 1;
const CONTROL_STYLE_ACCENT: u8 = 3;
const CONTROL_STYLE_SEARCH: u8 = 4;
const CONTROL_STYLE_STATUS: u8 = 5;
const CONTROL_FLAG_FOCUSABLE: u8 = 1 << 0;
const CONTROL_FLAG_DISABLED: u8 = 1 << 1;
const HEADER_HEIGHT: i32 = 44;

#[derive(Clone)]
struct Row {
    text: String,
    action: String,
    kind: u8,
    style: u8,
    flags: u8,
    x: i32,
    y: i32,
    width: i32,
    height: i32,
}

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

#[derive(Clone, PartialEq, Eq)]
enum FocusScope {
    Desktop,
    Window(String),
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
    hovered_control: Option<(String, usize)>,
    pressed_control: Option<(String, usize)>,
    focused_control: Option<(String, usize)>,
    focus_scope: FocusScope,
    drag: Option<(String, i32, i32)>, // window ID and pointer offset
}

impl Compositor {
    pub fn new() -> Self {
        let mut compositor = Self {
            pixels: vec![0; WIDTH * HEIGHT], clients: WindowServer::new(),
            windows: Vec::new(), next_id: 1,
            wallpaper_base: 0x002f80ed,
            cursor_x: (WIDTH / 2) as i32, cursor_y: (HEIGHT / 2) as i32,
            left_down: false,
            hovered_control: None,
            pressed_control: None,
            focused_control: None,
            focus_scope: FocusScope::Desktop,
            drag: None,
        };

        // The visible shell model now comes from C++ through the tiny C ABI.
        // Rust remains responsible for validation, ownership and rendering.
        if ffi::shell_abi_version() == 3 {
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
                        style: item.style,
                        flags: item.flags,
                        x: item.x,
                        y: item.y,
                        width: item.width,
                        height: item.height,
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
                    minimized: !surface.visible,
                    restore: None,
                });
            }
        }

        compositor.render();
        compositor
    }

    /// Native compositor test window. Real apps request windows through the window protocol.
    pub fn open_test_window(&mut self) {
        if self.windows.len() == MAX_WINDOWS { return; }
        let offset = self.windows.len() as i32 * 28;
        self.windows.push(Window {
            id: alloc::format!("native{}", self.next_id), role: b'W',
            x: 104 + offset, y: 88 + offset,
            width: 440, height: 300, title: "WINDOW TEST".to_string(),
            rows: vec![Row {
                text: "RUST COMPOSITOR".to_string(), action: String::new(), kind: b'l',
                style: 0, flags: 0, x: 0, y: 0, width: 0, height: 0,
            },
            Row {
                text: "DRAG TITLE BAR".to_string(), action: String::new(), kind: b'l',
                style: 0, flags: 0, x: 0, y: 0, width: 0, height: 0,
            }],
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
                let scope = if matches!(window.role, b'W' | b'L') {
                    FocusScope::Window(window.id.clone())
                } else {
                    FocusScope::Desktop
                };
                self.windows.push(window);
                self.set_focus_scope(scope);
                self.render();
            }
            return true;
        }
        if let Some(id) = line.strip_prefix("M|") {
            if let Some(index) = self.windows.iter().position(|window| window.id == id) {
                self.windows[index].minimized = true;
                self.repair_focus_scope();
                self.render();
            }
            return true;
        }
        if let Some(id) = line.strip_prefix("X|") {
            self.windows.retain(|window| window.id != id);
            if id == "wallpaper" { self.wallpaper_base = 0x002f80ed; }
            self.repair_focus_scope();
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
            parsed_rows.push(Row {
                text: text.to_string(), action: action.to_string(), kind,
                style: 0, flags: 0, x: 0, y: 0, width: 0, height: 0,
            });
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

    pub fn key_scancode(&mut self, scan: u8, text: Option<u8>, reverse_focus: bool)
        -> (Option<String>, Vec<Event>) {
        // App/client windows own keyboard traversal while they are active.
        if self.clients.is_active() {
            return (None, self.clients.key(scan, text));
        }

        if scan & 0x80 == 0 {
            // Tab / Shift+Tab moves focus across native managed controls.
            if scan == 0x0f {
                self.focus_next(reverse_focus);
                self.render();
                return (None, Vec::new());
            }

            // Enter or Space activates the focused control.
            if matches!(scan, 0x1c | 0x39) {
                if let Some(action) = self.focused_action() {
                    self.render();
                    return (Some(action), Vec::new());
                }
            }
        }

        (None, self.clients.key(scan, text))
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

        self.hovered_control = if handled {
            None
        } else {
            self.control_at(self.cursor_x, self.cursor_y)
        };

        let mut action = None;
        if !handled && left && !self.left_down {
            self.pressed_control = self.hovered_control.clone();
            // Window chrome reacts on press; managed controls only focus here.
            // Their action fires on release if the pointer is still over the
            // same control.
            action = self.pointer_down();
        } else if !left && self.left_down {
            if !handled {
                action = self.release_control();
            }
            self.pressed_control = None;
        }

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

    fn control_at(&self, x: i32, y: i32) -> Option<(String, usize)> {
        for window in self.windows.iter().rev() {
            if window.minimized { continue; }
            let local_x = x - window.x;
            let local_y = y - window.y;
            for (index, row) in window.rows.iter().enumerate() {
                if row.width <= 0 || row.height <= 0
                    || row.action.is_empty()
                    || row.flags & CONTROL_FLAG_DISABLED != 0 {
                    continue;
                }
                if local_x >= row.x && local_x < row.x + row.width
                    && local_y >= row.y && local_y < row.y + row.height {
                    return Some((window.id.clone(), index));
                }
            }
        }
        None
    }

    fn focused_action(&self) -> Option<String> {
        let (id, index) = self.focused_control.as_ref()?;
        let window = self.windows.iter().find(|window| !window.minimized && &window.id == id)?;
        let row = window.rows.get(*index)?;
        if row.flags & CONTROL_FLAG_DISABLED != 0 || row.flags & CONTROL_FLAG_FOCUSABLE == 0
            || row.action.is_empty() {
            return None;
        }
        Some(row.action.clone())
    }

    fn focus_next(&mut self, reverse: bool) {
        let mut controls: Vec<(String, usize)> = Vec::new();
        for window in &self.windows {
            if window.minimized { continue; }
            for (index, row) in window.rows.iter().enumerate() {
                if row.width > 0 && row.height > 0
                    && row.flags & CONTROL_FLAG_FOCUSABLE != 0
                    && row.flags & CONTROL_FLAG_DISABLED == 0
                    && !row.action.is_empty() {
                    controls.push((window.id.clone(), index));
                }
            }
        }

        if controls.is_empty() {
            self.focused_control = None;
            return;
        }

        let current = self.focused_control.as_ref()
            .and_then(|focused| controls.iter().position(|candidate| candidate == focused));

        let next = if reverse {
            current.map(|index| if index == 0 { controls.len() - 1 } else { index - 1 })
                .unwrap_or(controls.len() - 1)
        } else {
            current.map(|index| (index + 1) % controls.len()).unwrap_or(0)
        };

        self.focused_control = Some(controls[next].clone());
    }

    fn control_state(&self, window_id: &str, index: usize)
        -> (bool, bool, bool) {
        let matches = |state: &Option<(String, usize)>| {
            state.as_ref().is_some_and(|(id, row)| id == window_id && *row == index)
        };
        (
            matches(&self.hovered_control),
            matches(&self.pressed_control),
            matches(&self.focused_control),
        )
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
                if window.native {
                    // Native compositor test windows really close.
                    return None;
                }
                let action = alloc::format!("{}.close", window.id);
                // Built-in C++ shell surfaces stay resident so they can be
                // restored later without a host process recreating them.
                window.minimized = true;
                self.windows.push(window);
                self.render();
                return Some(action);
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
        let local_x = self.cursor_x - window.x;
        let local_y = self.cursor_y - window.y;
        let laid_out = window.rows.iter().any(|row| row.width > 0 && row.height > 0);

        let (row, in_rows) = if laid_out {
            // Every migrated surface uses Control Manager rectangles. The
            // compositor no longer needs to know whether this is a Dock,
            // Panel, Launcher, Files, Settings, or Terminal layout.
            let row = window.rows.iter().position(|row| {
                row.width > 0 && row.height > 0
                    && local_x >= row.x && local_x < row.x + row.width
                    && local_y >= row.y && local_y < row.y + row.height
            }).unwrap_or(usize::MAX);
            (row, row != usize::MAX)
        } else if window.role == b'D' {
            // Legacy bridge compatibility.
            let row = if (12..=116).contains(&local_x) { 0 }
                else if (132..=236).contains(&local_x) { 1 }
                else if (252..=356).contains(&local_x) { 2 }
                else { usize::MAX };
            (row, (8..60).contains(&local_y))
        } else if window.role == b'P' {
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
        let action = if in_rows {
            window.rows.get(row).and_then(|control| {
                if control.flags & CONTROL_FLAG_DISABLED != 0 {
                    None
                } else {
                    if control.flags & CONTROL_FLAG_FOCUSABLE != 0 {
                        self.focused_control = Some((window.id.clone(), row));
                    }
                    Some(control.action.clone())
                }
            })
        } else { None };
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
            // Hideo-inspired taskbar. Geometry and focusability come from the
            // C++ Control Manager; Rust only paints the current interaction state.
            self.fill_rect(window.x, window.y, window.width, window.height, SHELL_DARK);
            self.fill_rect(window.x, window.y + window.height - 1, window.width, 1, SHELL_BORDER);

            let managed = window.rows.iter().any(|row| row.width > 0 && row.height > 0);
            if managed {
                for (index, row) in window.rows.iter().take(8).enumerate() {
                    if row.width <= 0 || row.height <= 0 { continue; }

                    let (hovered, pressed, control_focused) = self.control_state(&window.id, index);
                    let disabled = row.flags & CONTROL_FLAG_DISABLED != 0;
                    let x = window.x + row.x;
                    let y = window.y + row.y;

                    let base = match row.style {
                        CONTROL_STYLE_ACCENT => CHROME_ACCENT,
                        _ => SHELL_SURFACE,
                    };
                    let fill = if disabled {
                        SHELL_DISABLED
                    } else if pressed {
                        SHELL_SURFACE_PRESSED
                    } else if hovered {
                        SHELL_SURFACE_HOVER
                    } else {
                        base
                    };
                    let border = if control_focused { CHROME_ACCENT } else { SHELL_BORDER };
                    let text_color = if disabled { 0x006f7983 } else { SHELL_TEXT };

                    self.rounded_rect(x, y, row.width, row.height, fill, border);

                    match row.style {
                        CONTROL_STYLE_SEARCH => {
                            self.fill_rect(x + 16, y + 11, 10, 10,
                                if disabled { 0x006f7983 } else { CHROME_ACCENT });
                            self.text(x + 36, y + 10, &row.text, 1, text_color);
                        }
                        CONTROL_STYLE_STATUS => {
                            let icon = if disabled { 0x006f7983 } else { 0x00d5dae0 };
                            self.fill_rect(x + 16, y + 11, 10, 8, icon);
                            self.fill_rect(x + 34, y + 9, 6, 12, icon);
                            self.fill_rect(x + 48, y + 10, 16, 10, icon);
                            self.text(x + 76, y + 10, &row.text, 1,
                                      if disabled { 0x006f7983 } else { SHELL_MUTED });
                        }
                        CONTROL_STYLE_SUBTLE => {
                            let text_width = row.text.len() as i32 * 6;
                            self.text(x + (row.width - text_width) / 2, y + 10,
                                      &row.text, 1, text_color);
                        }
                        _ => {
                            self.text(x + 16, y + 10, &row.text, 1, text_color);
                        }
                    }
                }
            } else {
                // Compatibility path for older bridge-created panel surfaces.
                self.rounded_rect(window.x + 12, window.y + 7, 198, 34, SHELL_SURFACE, SHELL_BORDER);
                if let Some(row) = window.rows.get(0) {
                    self.text(window.x + 48, window.y + 17, &row.text, 1, SHELL_TEXT);
                }
                self.rounded_rect(window.x + 420, window.y + 7, 184, 34, SHELL_SURFACE, SHELL_BORDER);
                if let Some(row) = window.rows.get(1) {
                    let text_width = row.text.len() as i32 * 6;
                    self.text(window.x + 512 - text_width / 2, window.y + 17, &row.text, 1, SHELL_TEXT);
                }
                self.rounded_rect(window.x + 774, window.y + 7, 238, 34, SHELL_SURFACE, SHELL_BORDER);
                if let Some(row) = window.rows.get(2) {
                    self.text(window.x + 850, window.y + 17, &row.text, 1, SHELL_MUTED);
                }
            }
            return;
        }

        if window.role == b'D' {
            self.rounded_rect(window.x + 5, window.y + 7, window.width, window.height,
                              0x0010151a, 0x0010151a);
            self.rounded_rect(window.x, window.y, window.width, window.height,
                              SHELL_DARK, SHELL_BORDER);

            for (index, row) in window.rows.iter().take(8).enumerate() {
                let managed = row.width > 0 && row.height > 0;
                let tile_x = window.x + if managed { row.x } else { 12 + index as i32 * 120 };
                let tile_y = window.y + if managed { row.y } else { 8 };
                let tile_w = if managed { row.width } else { 104 };
                let tile_h = if managed { row.height } else { 52 };

                let (hovered, pressed, control_focused) = self.control_state(&window.id, index);
                let disabled = row.flags & CONTROL_FLAG_DISABLED != 0;
                let base = match row.style {
                    CONTROL_STYLE_ACCENT => CHROME_ACCENT,
                    _ => SHELL_SURFACE,
                };
                let tile_fill = if disabled {
                    SHELL_DISABLED
                } else if pressed {
                    SHELL_SURFACE_PRESSED
                } else if hovered {
                    SHELL_SURFACE_HOVER
                } else {
                    base
                };
                let tile_border = if control_focused { CHROME_ACCENT } else { SHELL_BORDER };
                self.rounded_rect(tile_x, tile_y, tile_w, tile_h, tile_fill, tile_border);

                let icon_x = tile_x + 10;
                let icon_y = tile_y + ((tile_h - 30) / 2).max(0);
                let icon_color = if disabled {
                    0x00525b64
                } else {
                    match index {
                        0 => 0x004f8fdc,
                        1 => 0x0057a773,
                        _ => 0x00886bd8,
                    }
                };
                self.rounded_rect(icon_x, icon_y, 30, 30, icon_color, icon_color);
                let glyph = match index { 0 => "A", 1 => "F", _ => "S" };
                self.text(icon_x + 9, icon_y + 9, glyph, 1,
                          if disabled { 0x00929aa2 } else { 0x00ffffff });

                self.text(tile_x + 49, tile_y + ((tile_h - 7) / 2).max(0),
                          &row.text, 1, if disabled { 0x00717a83 } else { SHELL_TEXT });
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
            let managed = row.width > 0 && row.height > 0;
            let x = window.x + if managed { row.x } else { 24 };
            let y = window.y + if managed { row.y } else { 60 + index as i32 * 42 };
            let width = if managed { row.width } else { window.width - 48 };
            let height = if managed { row.height } else { 34 };

            if y >= window.y + window.height { break; }

            let (hovered, pressed, control_focused) = self.control_state(&window.id, index);
            let disabled = row.flags & CONTROL_FLAG_DISABLED != 0;
            let surface = if disabled {
                CONTROL_LIGHT_DISABLED
            } else if pressed {
                CONTROL_LIGHT_PRESSED
            } else if hovered {
                CONTROL_LIGHT_HOVER
            } else {
                0x00ffffff
            };
            let border = if control_focused {
                CHROME_ACCENT
            } else if focused {
                0x00b7bec3
            } else {
                0x00c2c8cc
            };
            let ink = if disabled { 0x00828a90 } else { CHROME_INK };

            match row.kind {
                b'h' => {
                    self.text(x + 4, y + ((height - 14) / 2).max(0),
                              &row.text, 2, ink);
                    self.fill_rect(x, y + height - 2, width, 1, 0x00d5d9dc);
                }
                b's' => {
                    self.rounded_rect(x, y, width, height, surface, border);
                    self.text(x + 14, y + ((height - 14) / 2).max(0),
                              &row.text, 2, if disabled { 0x0090999f } else { 0x00777f85 });
                }
                b't' => {
                    if !row.action.is_empty() {
                        self.rounded_rect(x, y, width, height, surface, border);
                    }
                    self.text(x + 14, y + ((height - 14) / 2).max(0),
                              &row.text, 2, ink);
                    let track_w = 38;
                    let track_h = 20;
                    let tx = x + width - track_w - 14;
                    let ty = y + (height - track_h) / 2;
                    self.rounded_rect(tx, ty, track_w, track_h,
                                      if disabled { 0x00cfd4d7 } else { 0x00c8cdd1 },
                                      0x00adb5ba);
                    self.rounded_rect(tx + 3, ty + 3, 14, 14,
                                      0x00ffffff, 0x00ffffff);
                }
                _ => {
                    if !row.action.is_empty() {
                        self.rounded_rect(x, y, width, height, surface, border);
                    }
                    self.text(x + 14, y + ((height - 14) / 2).max(0),
                              &row.text, 2, ink);
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

/// Built-in capitals for the native bootstrap shell and compositor windows.
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
