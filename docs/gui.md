# GUI architecture (M4)

Pippin's desktop is now a native guest-side stack:

| Layer | Language | Responsibility |
| --- | --- | --- |
| Drivers and display access | C++ | PCI discovery, VGA BAR access, PS/2 input |
| Shell model | C++ | Panel, dock, launcher, Files, Settings, Terminal surface definitions |
| ABI glue | C | Stable structs/version/validation between C++ and Rust |
| Compositor/window manager | Rust | Framebuffer, ownership, clipping, focus, dragging, minimize/restore and pointer routing |

The shell model lives in `kernel/cpp/src/shell.cc`. It exports a small array of
surface descriptors through `kernel/c/include/pippin/shell.h`. Rust imports
those descriptors through `kernel/rust/src/ffi.rs`, copies them into compositor
owned state, and renders them.

This is intentionally an interim bootstrap architecture. Pippin does not yet
have a general native executable loader for C++ ring-3 applications. Keeping the
shell model in C++ now means the UI language and ABI are already correct; once
the loader exists, the same model/toolkit can move into a user process instead
of being rewritten.

## What runs today

`make run` boots directly into the 1024x768 native desktop. No host .NET process
is required. The top panel and dock appear immediately. Launcher, Files,
Settings, and Terminal are native C++-defined surfaces that start hidden and are
restored by Pippin itself.

Press Esc to return to the text command shell. The `desktop` command reopens the
graphics desktop.

The previous C# code under `apps/csharp/` is retained only as a UI prototype
and API reference; it is no longer part of the default boot path.

## Native shell ABI

The C ABI exposes:

- ABI version
- surface count
- indexed surface descriptors
- role, geometry, title, initial visibility
- bounded row/item descriptors with text, action and kind
- C-side validation before Rust accepts a descriptor

Rust owns the copied strings and window state after import. C++ does not receive
raw framebuffer ownership from Rust.

## Next implementation steps

1. Build a reusable C++ widget/layout library for the native shell.
2. Give Terminal a native command/input model instead of the current static
   bootstrap surface.
3. Add a general ELF/native executable loader and per-process address spaces.
4. Move the C++ shell and applications out of the kernel image into ring-3
   processes using the same ABI concepts.
5. Add richer text rendering, icons, damage tracking, and input focus.

The bootstrap CLI remains available over VGA text and serial. Its commands
include `help`, `fetch`, `desktop`, `console`, `clear`, `uname`, `uptime`,
`mem`, `pci`, `ls`, `cat hello.pipb`, and `echo TEXT`.
