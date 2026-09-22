# Milestones

Each milestone leaves the system running in QEMU. The Multiboot path uses
`make run`; the Limine path uses `make iso` and boots the ISO in QEMU.

## M0 — Skeleton (complete) ✅

- [x] Multi-language build: Assembly + Rust + C++ + C + driver C++ in one ELF
- [x] Multiboot header with explicit addresses; boots under QEMU (`-kernel`)
- [x] 32→64 bit bring-up (PAE, paging, GDT, SSE-enabled CR4)
- [x] C++ boot runtime runs global constructors and hands off to Rust
- [x] Rust core logs boot banner over COM1 serial
- [x] Reverse FFI: Rust calls C++ (`pippin_cpp_version`) and the driver
      registry (`pippin_driver_count`)
- [x] C glue provides `memset`/`memcpy`/`memmove`/`memcmp`/`strlen`
- [x] 68k: long-range plan only (no code)

## M1 — Core (complete) ✅

- [x] Higher-half remap (`0xFFFF800000000000+`), real 4 KiB page tables
- [x] Physical frame allocator (bitmap over the bootloader memory map)
- [x] Zone heap: `#[global_allocator]` backing `Box`/`Vec` in the Rust core
- [x] Full IDT and Rust interrupt dispatcher; serial keeps working
- [x] GDT/TSS for later task and privilege transitions
- [x] PIT timer and basic tick-based sleep
- [x] APIC timer support
- [x] Add a Limine protocol boot path (`boot/limine.conf`,
      `scripts/make-iso.sh`); `make iso` produces a bootable image
- [x] Switch Rust to the freestanding `x86_64-unknown-none` target and update
      `.cargo/config.toml` + CMake `PIPPIN_RUST_TARGET_SUBDIR` together

## M2 — Processes & IPC (complete) ✅

- [x] `syscall`/`sysret` trampolines; numbered table (`pippin::kabi::Syscall`)
- [x] Preemptive scheduler: run queue, quantum, per-task FP/XMM save/restore
- [x] Threads + the "process slot" model (classic ProcMenu energy)
- [x] Message ports; Event Manager feeds the loop over IPC
- [x] Memory zones with ownership; handles (not pointers) for user objects

M2 demonstrates one ring-3 process in the shared boot address space. Private
address spaces, general executable loading, and a user Toolbox ABI remain
future work.

## M3 — Drivers & File (core complete) ✅

- [x] ACPI root table validation, PCI discovery and bridge parent tree;
      C++ driver `probe`/`init` lifecycle
- [x] PS/2 keyboard/mouse packets → Event Manager IPC events
- [x] RGB framebuffer via Limine (first Display Manager path)
- [x] AHCI SATA sector reads and bounded read-only FAT32 file access
- [x] File Manager loads a versioned Hello app bundle from FAT32 disk;
      Limine module path also works from ISO

M3's file path reads one small root-directory bundle; it does not execute the
bundle as a user program. The AHCI reader is polling and single-port. ISO9660,
FAT writes, ACPI namespace evaluation and broader hardware support follow in
later storage and driver work.

## M4 — GUI

- [x] Interactive bootstrap CLI on VGA text and serial (`help`, `fetch`, `clear`,
      `uname`, `uptime`, `mem`, `pci`, `ls`, `cat`, `echo`)
- [x] Rust framebuffer compositor with wallpaper fallback and clipped 2-D drawing
- [x] Rust window stack with focus, dragging, close box, mouse pointer, and Esc return
- [x] C# UI and shell projects define panel, dock, launcher, settings, files,
      notifications, wallpaper, and individual app surfaces on the host
- [ ] Window resizing and damage tracking
- [ ] Managed guest runtime, app loader, and IPC connection for C# surfaces

## M4.5 — C# shell in QEMU

- [x] QEMU COM2 socket bridge with versioned surface and click protocol
- [x] C# host shell displays its panel, dock, notification, launcher, Settings,
      Files and wallpaper palette as separate QEMU surfaces
- [x] Rust compositor returns button actions to C#; C# opens and closes windows
- [ ] Managed runtime and loader to move the C# process inside Pippin

## Window API foundation (host clients in QEMU)

- [x] Generic Rust client windows with independent pixel buffers, z-order,
      focus, moving, resizing, close requests, clipping and client events
- [x] Version 2 window protocol and broker for multiple C# processes
- [x] Low-level C# window API for lifecycle, geometry, invalidation, cursors,
      surfaces and input events
- [x] Two independent C# processes create overlapping About and Task Manager
      windows in QEMU; each paints and receives events through its own surface
- [x] First reusable toolkit widgets: Window, Panel, StackPanel, Label, Button
- [ ] Guest process loader and managed runtime replace the host bridge

## M5 — Native Apps

- [ ] `apps/cpp/` and `apps/rust/` become application trees; `PIPPIN_BUILD_APPS=ON`
- [ ] Toolbox client library for native apps (windows, menus, controls)
- [ ] Rust bindings to the same stable Toolbox C ABI
- [ ] Sample: a bitmap/text editor that can load/save via the File Manager

## M6 — Application runtime expansion

- [ ] Broaden managed app packaging, lifecycle, and services after the M4 C#
      shell runtime is working
- [ ] Sample third-party C# GUI app alongside native C++ and Rust apps

## M7 — 68k study

- [ ] Feasibility: kernel core (Rust) + C++ subsystem split on 68k
- [ ] Decide: 68k as an emulated target (Retro68/QEMU) vs physical
- [ ] Decide whether a managed shell is feasible on 68k or needs a native alternative

## Skipping notes

- C# shell support is an M4 goal on x86-64; a managed guest runtime is not yet
  available. Any 68k port needs its own shell/runtime decision.
- The kernel core stays `no_std`, dependency-free, and float-free forever —
  that discipline is what makes M7 plausible.
