# Milestones

Each milestone leaves the system running in QEMU. M0 and the Multiboot M1 path
use `make run`; the Limine M1 path uses `make iso` and boots the ISO in QEMU.

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

## M2 — Processes & IPC

- [ ] `syscall`/`sysret` trampolines; numbered table (`pippin::kabi::Syscall`)
- [ ] Preemptive scheduler: run queues, quantum, per-task FP/XMM save/restore
- [ ] Threads + the "process slot" model (classic ProcMenu energy)
- [ ] Message ports; Event Manager feeds the loop over IPC
- [ ] Memory zones with ownership; handles (not pointers) for user objects

## M3 — Drivers & File

- [ ] ACPI/PCI discovery; real device tree; driver midlifecycle (`probe`/`init`)
- [ ] PS/2 keyboard/mouse → `kEventKey`/`kEventMouse`
- [ ] VESA framebuffer via Limine (budget: the Display Manager goes live)
- [ ] AHCI disk + FAT (then ISO9660)
- [ ] First user of the File Manager: load an app bundle from disk

## M4 — GUI

- [ ] Display Manager: framebuffer compositor + 2-D raster engine
- [ ] Window Manager: z-order, drag/resize, dirty regions
- [ ] Menu Manager (fixed menu bar), Control Manager (owner-drawn widgets)
- [ ] Resource Manager: typed blobs, fonts, themes
- [ ] C++ and Rust desktop shell: menu bar, click-to-focus windows, desktop background

## M5 — Native Apps

- [ ] `apps/cpp/` and `apps/rust/` become application trees; `PIPPIN_BUILD_APPS=ON`
- [ ] Toolbox client library for native apps (windows, menus, controls)
- [ ] Rust bindings to the same stable Toolbox C ABI
- [ ] Sample: a bitmap/text editor that can load/save via the File Manager

## M6 — Optional C# Apps (x86-64 only)

- [ ] Evaluate a managed runtime for optional apps in `apps/csharp/`
- [ ] C# bindings to the Toolbox ABI for windows, controls, and events
- [ ] Sample C# GUI app alongside native C++ and Rust apps

## M7 — 68k study

- [ ] Feasibility: kernel core (Rust) + C++ subsystem split on 68k
- [ ] Decide: 68k as an emulated target (Retro68/QEMU) vs physical
- [ ] Keep optional C# application support outside the 68k platform

## Skipping notes

- C# app support is optional, begins no earlier than M6, and does not follow to 68k.
- The kernel core stays `no_std`, dependency-free, and float-free forever —
  that discipline is what makes M7 plausible.
