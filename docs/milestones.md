# Milestones

Each milestone leaves the system running in QEMU. "Done" means the checkboxes
in `make run` output and hold steady.

## M0 — Skeleton (current) ✅

- [x] Multi-language build: Assembly + Rust + C++ + C + driver C++ in one ELF
- [x] Multiboot header with explicit addresses; boots under QEMU (`-kernel`)
- [x] 32→64 bit bring-up (PAE, paging, GDT, SSE-enabled CR4)
- [x] C++ boot runtime runs global constructors and hands off to Rust
- [x] Rust core logs boot banner over COM1 serial
- [x] Reverse FFI: Rust calls C++ (`pippin_cpp_version`) and the driver
      registry (`pippin_driver_count`)
- [x] C glue provides `memset`/`memcpy`/`memmove`/`memcmp`/`strlen`
- [x] 68k: long-range plan only (no code)

## M1 — Core

- [ ] Higher-half remap (`0xFFFF800000000000+`), real 4 KiB page tables
- [ ] Physical frame allocator (bitmap over the bootloader memory map)
- [ ] Zone heap: `#[global_allocator]` backing `Box`/`Vec` in the Rust core
- [ ] GDT/TSS + full IDT; Rust interrupt dispatcher; serial keeps working
- [ ] PIT/APIC timer + basic `sleep`/ticks
- [ ] Migrate boot protocol to Limine (`boot/limine.conf`,
      `scripts/make-iso.sh`); `make iso` produces a bootable image
- [ ] Switch Rust to a freestanding target (`x86_64-unknown-none` via rustup,
      or custom JSON + `-Zbuild-std`); update `.cargo/config.toml` + CMake
      `PIPPIN_RUST_TARGET_SUBDIR` in the same change

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
- [ ] Retro desktop: menu bar, click-to-focus windows, desktop background

## M5 — C++ Apps

- [ ] `apps/cpp/` becomes a real application tree; `PIPPIN_BUILD_APPS=ON`
- [ ] Toolbox client library for native apps (windows, menus, controls)
- [ ] Sample: a bitmap/text editor that can load/save via the File Manager

## M6 — C# desktop (x86-64 only)

- [ ] AOT-managed runtime in `apps/csharp/`; shell apps in C#
- [ ] C# via `P/Invoke`-style extern "C" thunks into the Toolbox ABI
- [ ] Desktop shell (menu bar, app switcher) in C#, native tools in C++

## M7 — 68k study

- [ ] Feasibility: kernel core (Rust) + C++ subsystem split on 68k
- [ ] Decide: 68k as an emulated target (Retro68/QEMU) vs physical
- [ ] C# explicitly excluded from the 68k platform

## Skipping notes

- C# is *not* added before M6 and does not follow to 68k.
- The kernel core stays `no_std`, dependency-free, and float-free forever —
  that discipline is what makes M7 plausible.