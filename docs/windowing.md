# Pippin window system: audit and implementation path

## Current system (before the new window API)

- `kernel/rust/src/compositor.rs` owns an 800×600 back buffer and a single
  vector of windows. It has z-order, focus by raising, drag and close hit
  testing, clipping, and a pointer. Window content is drawn from text rows
  inside the compositor. There are no client pixel buffers or resize events.
- `kernel/rust/src/desktop.rs` switches QEMU standard VGA to 800×600×32 and
  copies the composed frame to PCI BAR0. C++ PCI code discovers that BAR.
- `kernel/rust/src/ps2.rs` polls keyboard and mouse and posts to the Event
  Manager. The bootstrap shell dispatches those events to the desktop.
- `kernel/rust/src/ipc.rs` has fixed-capacity in-kernel message ports. The
  scheduler has process slots but only one fixed ring-3 demo program in a
  shared address space. It cannot load arbitrary applications or .NET yet.
- `kernel/rust/src/bridge.rs` uses COM2 for one host C# shell process. Its
  current protocol is a row description and click action, not a window API.
- `apps/csharp/Pippin.UI` contains declarative `Surface`/`UiNode` records.
  `Pippin.Shell` runs as one host process and sends all shell surfaces.

## Smallest clean boundary

```text
Independent application processes (host bridge now; guest later)
    → Pippin.UI toolkit (layout, painting, local widget input)
    → Pippin.Window client API (window lifecycle and events)
    → versioned window protocol via broker and COM2
    → Rust compositor (ownership, window stack, focus, geometry, clipping)
    → QEMU VGA framebuffer
```

The broker only multiplexes clients and assigns ownership. Applications
never access the compositor's window vector or framebuffer. Each window has
an ID, owner, visibility state, title, position, size, focus state and its own
pixel buffer. The compositor paints window decorations and composites the
client buffer inside the frame. Window commands target an ID and return a
status or event. Input is reported in window-local coordinates.

The protocol must support create/destroy, show/hide, title/size/position,
surface submission/invalidation, and cursor selection. Events include create,
close request, resize/move, focus changes, pointer motion and buttons,
keyboard make/break and text input. Client ownership is enforced by the
broker for the host transport and later by guest IPC credentials. Messages
are bounded; pixel updates are chunked and acknowledged so the UART FIFO
cannot overrun. The transport is replaceable without changing the API.

## Increments

1. **Multi-window core:** generic client windows in Rust, lifecycle and
   geometry commands, focus/z-order, drag/resize/close request, event routing.
   A host broker runs two separate application processes and routes events.
2. **Client surfaces and events:** each app allocates and paints a pixel
   buffer; the protocol submits bounded, compressed updates. Rust composites
   buffers and sends pointer/keyboard/window events. The client redraws after
   resize or invalidation.
3. **Toolkit:** `Window`, panel/stack layout, `Label` and `Button` built on
   the low-level API. A button is hit-tested by the client using local input,
   not by the compositor. Other widgets and visual polish follow later.

These increments make C# applications visible and interactive in QEMU while
the .NET processes run on the host. Actual guest C# processes require a
managed runtime, an executable loader, private address spaces, and a guest
IPC transport. The window API and protocol should not depend on the COM2
bridge so they can be reused when that work lands.

## Implemented version 2 protocol

`H|2` / `R|2` negotiates the COM2 protocol. The host broker accepts any
number of local C# clients and serializes their commands. It assigns window
ownership by the client's connection and forwards only that window's events
back to it. On disconnect it destroys the client's windows. Commands use
newline-terminated ASCII and are acknowledged with `A` or rejected with `N`
by Pippin. The broker replies `OK` or `ERR` to each client. IDs are numeric and globally
unique while connected.

| Command | Operation |
| --- | --- |
| `WC|id|x|y|width|height|title` | Create a hidden window |
| `WD|id`, `WS|id`, `WH|id` | Destroy, show, hide |
| `WT|id|title`, `WZ|id|w|h`, `WM|id|x|y` | Change title, size, position |
| `WI|id`, `WK|id|cursor` | Request redraw, select pointer shape |
| `WB|id`, `WP|id|offset|length,color;...`, `WE|id` | Begin, stream RGB runs, end a surface update |

`EV|id|name|a|b` carries `WindowCreated`, `CloseRequested`, `Resize`, `Move`,
`FocusGained`, `FocusLost`, `MouseMove`, `MouseDown`, `MouseUp`, `KeyDown`,
`KeyUp`, `TextInput`, and `Redraw`. Pointer coordinates are relative to the
client content area. `TextInput` sends an ASCII byte in `a` for the current
US keyboard mapping. The compositor handles decorations and never interprets
widget clicks; the C# toolkit does that inside its window.

The current protocol is intentionally bounded to 384 bytes per line, eight
client windows, and 600×440 RGB surfaces. Updates are run-length encoded and
acknowledged chunk by chunk to fit COM2. The broker validates ownership;
the guest validates IDs, sizes and buffer offsets. This transport is a
development bridge, not the final guest IPC ABI.

## Remaining gaps

- The two demo applications are separate host .NET processes, not guest
  Pippin processes. Pippin's scheduler has one fixed ring-3 demo program and
  no general executable loader or managed runtime.
- The older M4.5 shell still uses the first text-row protocol. It can run
  separately with `make run-shell`; it has not migrated to `Pippin.Window`.
- The toolkit currently provides vertical layout, bitmap text, labels and
  buttons. Grid, images, text fields, scrolling, menus, accessibility and
  polished font rendering are future work.
- The window frame has a close box and resize grip. Minimize/maximize and
  saved window placement are not implemented yet.
