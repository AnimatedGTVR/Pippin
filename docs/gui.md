# GUI — the retro desktop

The endgame is a Macintosh-shaped desktop: fixed menu bar, overlapping
windows, click-to-focus, owner-drawn controls, one primary mouse button. This
document records the intended shape so early kernel decisions (memory zones,
event model, handle-based objects) don't paint us into a corner.

## Milestone 4 — Display Manager ("QuickDraw-lite")

- Framebuffer from the bootloader (Limine) at Milestone 1+.
- A tiny 2-D raster engine: `set_pixel`, `fill_rect`, `blit`, clipping through
  dirty rectangles. All integers; the FPU policy from [kernel.md](kernel.md)
  holds.
- Double-buffered per-window or full-screen back buffer; compositing in the
  kernel for now, in a trusted server later.
- Text: a built-in bitmap font (the Macintosh System font energy, not the
  copyrights). This is the future "Resource Manager" font resource.

## The "Toolbox shell" (Milestone 4–5)

Managers that make Pippin feel like a Mac:

- **Window Manager** — window list, z-order, front window, drag/resize by
  frame handles, region invalidation, close/zoom boxes.
- **Menu Manager** — the always-present menu bar; drop-down menus; keyboard
  equivalents (⌘Q etc.).
- **Control Manager** — buttons, checkboxes, scrollbars, progress bars, all
  owner-drawn and hit-testable.
- **Event Manager** — the application event loop:
  `kEventWindowActivate`, `kEventMenuSelect`, `kEventMouseDown/Up`,
  `kEventKey`, `kEventIdle`. Events are queued IPC messages (see
  [architecture.md](architecture.md) §8), so drivers feed the loop through the
  same path as timers.

## Application model

- **Cooperative, event-driven apps**: a document/window draws itself on
  demand (`kEventWindowActivate`, dirty-region blends) and never blocks the
  machine. This is the classic Mac formula and it makes single-GUI-user
  scheduling trivially safe.
- **Handle-based resources** (the Resource Manager) so apps and the shell can
  be swapped without pointer invalidation.
- **C++ apps** at Milestone 5 using a small Toolbox client library
  (`apps/cpp/` becomes a real tree here with `PIpinToolbox.hh`-style headers).
- **C# desktop (x86-64 only, Milestone 6):** an AOT-managed runtime hosts the
  shell and higher-level apps. C# is intentionally an x86-64-desktop-only
  concern — it does not affect the 68k port or the kernel.

## Aesthetic notes

- Greys/beige, dark chrome, and the Mac's confident use of 1 px rules and
  drop shadows. Fonts bitmap-basic at first, then a proper rasterizer.
- The desktop busies itself with a menu bar, a "Finder"-like list of apps and
  draggable windows — not compositor eye-candy. Retrofuturist, not skeuomorph.

## Premise for the kernel

The GUI works because of three early kernel decisions, not because of clever
window code:

1. **Handles, not pointers** — resources survive load/unload without
   dangling memory.
2. **Typed event queues** — every input path funnels into the Event Manager.
3. **Zone memory with ownership** — the Window Manager, Menu Manager, etc.
   each own a zone, so ownership of a pixel's worth of memory is always
   defined.