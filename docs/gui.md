# GUI architecture (M4)

The reusable multi-window API and toolkit are specified in
[windowing.md](windowing.md). Run `make run-ui` to see two independent C# app
processes create overlapping windows through the Rust compositor. The apps
currently run on the host through a broker; Pippin still needs a guest
managed runtime and loader.

Pippin's desktop is split into three layers:

| Layer | Language | Responsibility |
| --- | --- | --- |
| Drivers and display access | C++ | PCI discovery, VGA BAR access, PS/2 input |
| Compositor and window manager | Rust | Framebuffer, wallpaper fallback, clipping, window stack, focus, dragging, close and pointer |
| Shell and app windows | C# | Panel, dock, launcher, settings, files, notifications, wallpaper selection and individual app content |

This aims for a GTK/GNOME-style separation: the compositor owns placement and
input; independent clients own their controls and content. The initial UI
library in `apps/csharp/Pippin.UI/` is deliberately small and declarative. A
`Surface` has a role, bounds and a widget tree (`UiNode`). C# shell components
in `apps/csharp/Pippin.Shell/` each describe their own surface. The M4.5 bridge
flattens those widget trees into a small text-and-button protocol. The Rust
compositor draws them and returns button actions.

## What runs today

`make run` builds and starts the C# shell on the host and boots QEMU with a
socket-backed second serial port. Pippin automatically enters the 1024×768
graphics desktop when the shell connects. The panel, dock, Terminal and
notification surfaces appear at startup. `make run-shell` is an alias for
this same full-desktop boot path.

The standalone `desktop` command remains a Rust-compositor-only fallback. It
does not start host C# applications by itself, because the guest cannot spawn
the host .NET process.


Click **Pippin** to open the launcher, **Files** to open its window, or
**Settings** to change the wallpaper palette. Window title bars drag, minimize to the dock, maximize/restore, and close. The
shell bridge also preserves window geometry/state when refreshing content. The C# process owns surface descriptions and
button actions; the Rust compositor owns pixels, window position and input.

Run `dotnet run --project apps/csharp/Pippin.Shell/Pippin.Shell.csproj` without
the bridge to print the shell descriptions as JSON. Pippin still cannot load
or execute .NET code inside the guest. The host bridge is an interactive
development step; running C# inside Pippin requires a managed runtime, app
loader and guest IPC.

## M4.5 bridge protocol

COM1 remains the boot log and CLI. COM2 carries newline-terminated ASCII
messages to the C# host process. `H|1` / `R|1` is the version handshake.
`S|id|role|x|y|width|height|title|kind:text@action;...` creates or refreshes a
surface; `R|id` restores a minimized surface and `X|id` removes one. Pippin acknowledges each command with `A`.
Button presses return `E|action`. The protocol is bounded to 384 bytes per
line and 16 simultaneous surfaces; it supports labels, headings, buttons, toggles and search-field presentation. Background (`B`), panel (`P`), dock (`D`), launcher (`L`),
notification (`N`) and app window (`W`) roles are defined.

## Next implementation steps

1. Add keyboard-driven shell controls, richer text input, and damage tracking.
2. Add a guest managed runtime and loader so C# clients can connect through
   guest IPC rather than the host serial bridge.
3. Let C# submit wallpaper pixels and app content, while keeping a Rust boot
   fallback.

The bootstrap CLI remains available over VGA text and serial. Its commands
include `help`, `fetch`, `desktop`, `console`, `clear`, `uname`, `uptime`, `mem`,
`pci`, `ls`, `cat hello.pipb`, and `echo TEXT`.
