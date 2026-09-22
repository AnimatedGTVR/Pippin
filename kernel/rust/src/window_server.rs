//! Generic client windows. No application widgets or content are defined here.

use alloc::string::{String, ToString};
use alloc::vec;
use alloc::vec::Vec;

use crate::compositor::{glyph, HEIGHT, WIDTH};

const MAX_WINDOWS: usize = 8;
const MAX_WIDTH: i32 = 600;
const MAX_HEIGHT: i32 = 440;
const TITLE_HEIGHT: i32 = 32;
const BORDER: i32 = 2;

#[derive(Clone, Copy)]
pub struct Event {
    pub id: u32,
    pub kind: &'static str,
    pub a: i32,
    pub b: i32,
}

impl Event {
    fn new(id: u32, kind: &'static str, a: i32, b: i32) -> Self {
        Self { id, kind, a, b }
    }
}

struct Window {
    id: u32,
    title: String,
    x: i32,
    y: i32,
    width: i32,
    height: i32,
    visible: bool,
    surface: Vec<u32>,
    updating: bool,
    cursor: u8,
}

impl Window {
    fn frame_width(&self) -> i32 { self.width + BORDER * 2 }
    fn frame_height(&self) -> i32 { self.height + TITLE_HEIGHT + BORDER }
    fn contains(&self, x: i32, y: i32) -> bool {
        self.visible && x >= self.x && y >= self.y
            && x < self.x + self.frame_width() && y < self.y + self.frame_height()
    }
    fn local(&self, x: i32, y: i32) -> (i32, i32) {
        (x - self.x - BORDER, y - self.y - TITLE_HEIGHT)
    }
}

#[derive(Clone, Copy)]
enum Grab { Move(u32, i32, i32), Resize(u32, i32, i32, i32, i32) }

pub struct WindowServer {
    windows: Vec<Window>, // back to front
    focused: Option<u32>,
    grab: Option<Grab>,
    pressed: Option<u32>,
}

impl WindowServer {
    pub fn new() -> Self {
        Self { windows: Vec::new(), focused: None, grab: None, pressed: None }
    }

    pub fn is_active(&self) -> bool { self.windows.iter().any(|window| window.visible) }

    pub fn cursor_at(&self, x: i32, y: i32) -> u8 {
        self.windows.iter().rfind(|window| window.contains(x, y))
            .map_or(0, |window| window.cursor)
    }

    /// Validate the complete command before mutating a window or surface.
    pub fn validate(&self, line: &str) -> bool {
        let mut fields = line.split('|');
        let (Some(op), Some(id)) = (fields.next(), fields.next().and_then(|s| s.parse::<u32>().ok()))
            else { return false; };
        if id == 0 { return false; }
        if op == "WC" {
            let (Some(_x), Some(_y), Some(width), Some(height), Some(title)) =
                (parse_i32(fields.next()), parse_i32(fields.next()),
                 parse_i32(fields.next()), parse_i32(fields.next()), fields.next()) else { return false; };
            return self.windows.len() < MAX_WINDOWS && !self.windows.iter().any(|w| w.id == id)
                && valid_size(width, height) && title.len() <= 48 && fields.next().is_none();
        }
        let Some(window) = self.windows.iter().find(|w| w.id == id) else { return false; };
        match op {
            "WD" | "WS" | "WH" | "WI" | "WB" | "WE" => fields.next().is_none(),
            "WT" => fields.next().is_some_and(|s| s.len() <= 48) && fields.next().is_none(),
            "WM" => parse_i32(fields.next()).is_some() && parse_i32(fields.next()).is_some()
                && fields.next().is_none(),
            "WZ" => match (parse_i32(fields.next()), parse_i32(fields.next())) {
                (Some(w), Some(h)) => valid_size(w, h) && fields.next().is_none(),
                _ => false,
            },
            "WK" => fields.next().is_some_and(|s| matches!(s, "default" | "hand" | "text"))
                && fields.next().is_none(),
            "WP" => {
                if !window.updating { return false; }
                let Some(mut at) = fields.next().and_then(|s| s.parse::<usize>().ok()) else { return false; };
                let Some(runs) = fields.next() else { return false; };
                if runs.is_empty() || fields.next().is_some() { return false; }
                for run in runs.split(';') {
                    let Some((count, color)) = run.split_once(',') else { return false; };
                    let (Ok(count), Ok(_)) = (count.parse::<usize>(), u32::from_str_radix(color, 16))
                        else { return false; };
                    let Some(end) = at.checked_add(count) else { return false; };
                    if count == 0 || end > window.surface.len() { return false; }
                    at = end;
                }
                true
            }
            _ => false,
        }
    }

    /// V2 commands are always addressed to a numeric window ID. The host
    /// broker owns ID allocation and enforces client ownership.
    pub fn command(&mut self, line: &str) -> Vec<Event> {
        let mut events = Vec::new();
        let mut fields = line.split('|');
        let Some(op) = fields.next() else { return events; };
        let Some(id) = fields.next().and_then(|field| field.parse::<u32>().ok()) else { return events; };
        if id == 0 { return events; }
        if op == "WC" {
            if self.windows.len() >= MAX_WINDOWS || self.windows.iter().any(|window| window.id == id) { return events; }
            let (Some(x), Some(y), Some(width), Some(height), Some(title)) =
                (parse_i32(fields.next()), parse_i32(fields.next()),
                 parse_i32(fields.next()), parse_i32(fields.next()), fields.next()) else { return events; };
            if !valid_size(width, height) || title.len() > 48 { return events; }
            let x = x.clamp(0, WIDTH as i32 - 40);
            let y = y.clamp(0, HEIGHT as i32 - TITLE_HEIGHT);
            self.windows.push(Window { id, title: title.to_string(), x, y, width, height,
                visible: false, surface: vec![0x0019232d; (width * height) as usize],
                updating: false, cursor: 0 });
            events.push(Event::new(id, "WindowCreated", x, y));
            events.push(Event::new(id, "Redraw", width, height));
            return events;
        }
        let Some(index) = self.windows.iter().position(|window| window.id == id) else { return events; };
        match op {
            "WD" => {
                self.windows.remove(index);
                if self.focused == Some(id) {
                    self.focused = None;
                    if let Some(next) = self.windows.iter().rfind(|window| window.visible) {
                        self.focused = Some(next.id);
                        events.push(Event::new(next.id, "FocusGained", 0, 0));
                    }
                }
                self.grab = None;
                self.pressed = None;
            }
            "WS" => {
                self.windows[index].visible = true;
                self.focus(id, &mut events);
            }
            "WH" => {
                self.windows[index].visible = false;
                if self.focused == Some(id) {
                    self.focused = None;
                    events.push(Event::new(id, "FocusLost", 0, 0));
                }
            }
            "WT" => {
                if let Some(title) = fields.next() {
                    if title.len() <= 48 { self.windows[index].title = title.to_string(); }
                }
            }
            "WM" => {
                if let (Some(x), Some(y)) = (parse_i32(fields.next()), parse_i32(fields.next())) {
                    self.windows[index].x = x.clamp(0, WIDTH as i32 - 40);
                    self.windows[index].y = y.clamp(0, HEIGHT as i32 - TITLE_HEIGHT);
                    events.push(Event::new(id, "Move", self.windows[index].x, self.windows[index].y));
                }
            }
            "WZ" => {
                if let (Some(width), Some(height)) = (parse_i32(fields.next()), parse_i32(fields.next())) {
                    self.resize(index, width, height, &mut events);
                }
            }
            "WK" => {
                if let Some(cursor) = fields.next() {
                    self.windows[index].cursor = match cursor { "hand" => 1, "text" => 2, _ => 0 };
                }
            }
            "WI" => events.push(Event::new(id, "Redraw", self.windows[index].width, self.windows[index].height)),
            "WB" => self.windows[index].updating = true,
            "WP" => {
                if self.windows[index].updating {
                    if let Some(offset) = fields.next().and_then(|s| s.parse::<usize>().ok()) {
                        let mut at = offset;
                        for run in fields.next().unwrap_or("").split(';') {
                            let Some((count, color)) = run.split_once(',') else { break; };
                            let (Ok(count), Ok(color)) = (count.parse::<usize>(), u32::from_str_radix(color, 16)) else { break; };
                            let Some(end) = at.checked_add(count) else { break; };
                            if count == 0 || end > self.windows[index].surface.len() { break; }
                            self.windows[index].surface[at..end].fill(color & 0x00ffffff);
                            at = end;
                        }
                    }
                }
            }
            "WE" => self.windows[index].updating = false,
            _ => {}
        }
        events
    }

    fn resize(&mut self, index: usize, width: i32, height: i32, events: &mut Vec<Event>) {
        if !valid_size(width, height) { return; }
        let window = &mut self.windows[index];
        if window.width == width && window.height == height { return; }
        window.width = width;
        window.height = height;
        window.surface = vec![0x0019232d; (width * height) as usize];
        events.push(Event::new(window.id, "Resize", width, height));
        events.push(Event::new(window.id, "Redraw", width, height));
    }

    fn focus(&mut self, id: u32, events: &mut Vec<Event>) {
        if self.focused != Some(id) {
            if let Some(old) = self.focused { events.push(Event::new(old, "FocusLost", 0, 0)); }
            self.focused = Some(id);
            events.push(Event::new(id, "FocusGained", 0, 0));
        }
        if let Some(index) = self.windows.iter().position(|window| window.id == id) {
            let window = self.windows.remove(index);
            self.windows.push(window);
        }
    }

    /// Returns true when a client window owns this pointer packet.
    pub fn pointer(&mut self, x: i32, y: i32, left: bool, was_left: bool) -> (bool, Vec<Event>) {
        let mut events = Vec::new();
        if left {
            if let Some(grab) = self.grab {
                match grab {
                    Grab::Move(id, ox, oy) => if let Some(window) = self.windows.iter_mut().find(|window| window.id == id) {
                        let nx = (x - ox).clamp(0, WIDTH as i32 - 40);
                        let ny = (y - oy).clamp(0, HEIGHT as i32 - TITLE_HEIGHT);
                        if (window.x, window.y) != (nx, ny) {
                            window.x = nx; window.y = ny;
                            events.push(Event::new(id, "Move", nx, ny));
                        }
                    },
                    Grab::Resize(id, sx, sy, sw, sh) => if let Some(index) = self.windows.iter().position(|window| window.id == id) {
                        self.resize(index, (sw + x - sx).clamp(80, MAX_WIDTH),
                            (sh + y - sy).clamp(50, MAX_HEIGHT), &mut events);
                    },
                }
                return (true, events);
            }
        } else { self.grab = None; }

        let hit = self.windows.iter().rposition(|window| window.contains(x, y));
        if left && !was_left {
            let Some(index) = hit else { return (false, events); };
            let id = self.windows[index].id;
            self.focus(id, &mut events);
            let window = self.windows.last().unwrap();
            let local_x = x - window.x;
            let local_y = y - window.y;
            if local_y < TITLE_HEIGHT {
                if local_x >= window.frame_width() - 28 {
                    events.push(Event::new(id, "CloseRequested", 0, 0));
                } else { self.grab = Some(Grab::Move(id, local_x, local_y)); }
            } else if local_x >= window.frame_width() - 14 && local_y >= window.frame_height() - 14 {
                self.grab = Some(Grab::Resize(id, x, y, window.width, window.height));
            } else {
                let (lx, ly) = window.local(x, y);
                if lx >= 0 && ly >= 0 && lx < window.width && ly < window.height {
                    self.pressed = Some(id);
                    events.push(Event::new(id, "MouseDown", lx, ly));
                }
            }
            return (true, events);
        }
        if !left && was_left {
            if let Some(id) = self.pressed.take() {
                if let Some(window) = self.windows.iter().find(|window| window.id == id) {
                    let (lx, ly) = window.local(x, y);
                    events.push(Event::new(id, "MouseUp", lx, ly));
                }
                return (true, events);
            }
        }
        if let Some(index) = hit {
            let window = &self.windows[index];
            let (lx, ly) = window.local(x, y);
            if lx >= 0 && ly >= 0 && lx < window.width && ly < window.height {
                events.push(Event::new(window.id, "MouseMove", lx, ly));
            }
            return (true, events);
        }
        (false, events)
    }

    pub fn key(&self, scan: u8, text: Option<u8>) -> Vec<Event> {
        let mut events = Vec::new();
        if let Some(id) = self.focused {
            let down = scan & 0x80 == 0;
            events.push(Event::new(id, if down { "KeyDown" } else { "KeyUp" }, (scan & 0x7f) as i32, 0));
            if let Some(byte) = text { events.push(Event::new(id, "TextInput", byte as i32, 0)); }
        }
        events
    }

    pub fn paint(&self, pixels: &mut [u32]) {
        for window in &self.windows {
            if !window.visible { continue; }
            let focused = self.focused == Some(window.id);
            fill(pixels, window.x + 7, window.y + 9, window.frame_width(), window.frame_height(), 0x0009141e);
            fill(pixels, window.x, window.y, window.frame_width(), window.frame_height(), 0x00384250);
            fill(pixels, window.x + 1, window.y + 1, window.frame_width() - 2, TITLE_HEIGHT - 1,
                if focused { 0x002a3948 } else { 0x0023303c });
            text(pixels, window.x + 11, window.y + 11, &window.title, 0x00e5edf3,
                ((window.frame_width() - 52) / 7).max(0) as usize);
            let close_x = window.x + window.frame_width() - 25;
            fill(pixels, close_x, window.y + 8, 16, 16, 0x00675661);
            text(pixels, close_x + 4, window.y + 9, "X", 0x00ffffff, 1);
            fill(pixels, window.x + BORDER, window.y + TITLE_HEIGHT,
                window.width, window.height, 0x0019232d);
            for sy in 0..window.height {
                let dy = window.y + TITLE_HEIGHT + sy;
                if dy < 0 || dy >= HEIGHT as i32 { continue; }
                for sx in 0..window.width {
                    let dx = window.x + BORDER + sx;
                    if dx >= 0 && dx < WIDTH as i32 {
                        pixels[dy as usize * WIDTH + dx as usize] =
                            window.surface[sy as usize * window.width as usize + sx as usize];
                    }
                }
            }
            fill(pixels, window.x + window.frame_width() - 12,
                window.y + window.frame_height() - 12, 8, 8, 0x007c8f9a);
        }
    }
}

fn parse_i32(value: Option<&str>) -> Option<i32> { value?.parse().ok() }
fn valid_size(width: i32, height: i32) -> bool {
    width >= 80 && height >= 50 && width <= MAX_WIDTH && height <= MAX_HEIGHT
}

fn fill(pixels: &mut [u32], x: i32, y: i32, w: i32, h: i32, color: u32) {
    for row in y.max(0)..(y + h).min(HEIGHT as i32) {
        for col in x.max(0)..(x + w).min(WIDTH as i32) {
            pixels[row as usize * WIDTH + col as usize] = color;
        }
    }
}

fn text(pixels: &mut [u32], x: i32, y: i32, label: &str, color: u32, max_chars: usize) {
    for (index, byte) in label.bytes().take(max_chars).enumerate() {
        for (row, bits) in glyph(byte.to_ascii_uppercase()).iter().enumerate() {
            for col in 0..5 {
                if bits & (1 << (4 - col)) != 0 {
                    fill(pixels, x + index as i32 * 7 + col, y + row as i32, 1, 1, color);
                }
            }
        }
    }
}
