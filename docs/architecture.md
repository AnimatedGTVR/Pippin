# Pippin Architecture

A retro-styled, Macintosh-inspired desktop operating system. Single-user,
GUI-first, designed to look and feel like a classic Macintosh while running on
modest modern hardware.

This document is the system's north star. Everything else in `docs/` is a
deeper dive into one slice: [build](build.md), [boot](boot.md),
[kernel](kernel.md), [drivers](drivers.md), [gui](gui.md),
[milestones](milestones.md).

---

## 1. Design goals

1. **The classic Macintosh experience.** Desktop metaphor: overlapping
   windows, a fixed menu bar, click-to-focus, drag-select, single primary
   mouse button. The system is built around a *Toolbox* of cooperating
   service managers and an event loop, not around POSIX-style processes.
2. **Retro where it makes sense, modern where it has to.** A preemptive
   scheduler and memory protection underneath, cooperative, event-driven
   applications on top. We borrow the Mac's *cooperative multitasking +
   message-passing* application model and combine it with a real, protected
   kernel core.
3. **Language honesty.** Each language is used for what it is genuinely good
   at, with a clean, documented ABI boundary between them (see §3, §10).
4. **Bootable reality check.** The skeleton must actually build and boot, not
   just look good on a whiteboard. Every milestone leaves the system running
   in QEMU.

## 2. What Pippin is *not*

- Not a Unix clone. No `/proc`, no fork/exec pipeline, no POSIX ABI.
- Not a Linux distribution or a microkernel research box — a single-user,
  appliance-style OS with one graphical user at a time.
- Not a faithful reimplementation of classic Mac OS. We borrow the *shape*,
   not the carbon API. (The name "Pippin" is a deliberate nod, not a license.)

## 3. Language map

| Layer            | Language | Responsibility |
|------------------|----------|----------------|
| Boot & CPU       | Assembly | Multiboot entry, long-mode bring-up, GDT/IDT/TSS tracks, syscall trampolines, context-switch stubs, spinlocks where rustc can't do it |
| Kernel core      | Rust     | Memory manager, interrupt dispatch, scheduler, IPC, syscall table, Toolbox service plumbing, safety-critical data structures |
| Subsystems       | C++      | ACPI/PCI, disks, filesystem, graphics/compositor plumbing, the C++ half of the Toolbox, driver framework |
| Glue / ABI       | C        | libc-style stubs (`memset`, `memcpy`, `strlen`, …), a stable shim layer for anything that wants a plain C ABI |
| Drivers          | C++      | Concrete device drivers implementing the `pippin::drv::Driver` interface |
| Desktop shell (later) | C++ and Rust | Native shell, desktop services, and Toolbox clients |
| Applications (later) | C++ and Rust; optional C# | Native Toolbox apps first; optional managed C# apps use Toolbox bindings on x86-64 |
| Build and boot tooling | Shell | Host-side scripts connect build, image creation, and QEMU commands |

This map describes current code and planned roles; it is not a fixed language
limit. Additional languages, such as Ada/SPARK or Haskell, are welcome when
they fit a concrete subsystem or application. Each addition should have a
supported target toolchain, a clear build path, and an explicit boundary to
the rest of Pippin (usually the Toolbox C ABI). Mentioning a language here
does not commit it to a milestone.

Future note — **68k**: the planned Motorola 68k port keeps Assembly + Rust + C++
(kernel) and C++ (drivers), with a C++ and Rust desktop. Optional C# app support
does not follow to 68k.

## 4. Layered architecture

```
┌───────────────────────────────────────────────────────────────┐
│  APPS (later): C++/Rust shell and apps; optional C# apps      │
│  → event loop, Window/Menu/Control managers, Resource Mgr    │
├───────────────────────────────────────────────────────────────┤
│  TOOLBOX API  (the "Macintosh-style" service surface)        │
│  → externally stable syscall ABI, handle-based                │
├───────────────────────────────────────────────────────────────┤
│  KERNEL SERVICES (Rust core + C++ subsystems)                │
│  → Memory Mgr, Processor/Scheduler, IPC/Event Mgr, File Mgr, │
│    Driver Mgr, Display Mgr                                    │
├───────────────────────────────────────────────────────────────┤
│  HAL (Rust + C++ + a little C)                               │
│  → interrupts, MMU, clock/timer, port I/O, ACPI/PCI          │
├───────────────────────────────────────────────────────────────┤
│  BRING-UP (Assembly)                                          │
│  → boot.S: Multiboot → long mode → kernel_entry               │
├───────────────────────────────────────────────────────────────┤
│  HARDWARE — QEMU q35 today, real x86-64 later, 68k later     │
└───────────────────────────────────────────────────────────────┘
```

## 5. The Toolbox model

Classic Mac OS organized itself as a set of "Managers", each owning one piece
of the machine. Pippin keeps that concept as named kernel services with the
calls routed through the syscall table. Several are familiar; a few are blends:

| Pippin Manager        | Classic Mac ancestor   | In charge of |
|-----------------------|------------------------|--------------|
| Boot Manager          | Init/Start Manager     | bring-up, boot args, resets |
| Memory Manager (MemMan) | Memory Manager       | physical frames, virtual address zones, kernel heap ("The Zone") |
| Processor Manager (ProcMan) | Process Manager   | threads, preemptive scheduler, run queues |
| Event Manager         | Event Manager          | queued events (input, timers, application messages) |
| IPC Manager           | (AppleEvents, ports)   | message ports, handle tracks between processes |
| File Manager          | File Manager           | VFS, media, the future resource fork |
| Driver Manager        | (Drivers)              | device tree, probe/init lifecycle, "Gestalt" registry |
| Display Manager       | QuickDraw              | framebuffer, 2-D primitives, double-buffering |
| Window Manager        | Window Manager         | window list, dirty regions, hit-testing |
| Menu Mgr / Control Mgr | Menu/Control Managers | fixed menu bar, buttons/scrollbars/… |
| Resource Manager      | Resource Manager       | typed blobs (`.rsrc`-style), theme, app resources |

The boundary between "kernel" and "Toolbox server" is deliberately fuzzy at
first: the Managers start as kernel services and grow user-space server
processes as protection arrives.

## 6. Kernel core design

- **Small preemptive core, cooperative apps.** The kernel is preemptively
  scheduled and interrupt-driven. Applications are *cooperative documents* in
  the Mac sense: one visible app, an owner-drawn interface, an event loop that
  pumps `kEvent*` messages. Preemption protects the system; cooperation makes
  the retro UI model honest.
- **Everything is a handle, not a pointer.** Following the classic Resource
  Manager, user-visible objects are handles into kernel-owned zones rather
  than raw addresses. This is the foundation of the protected Toolbox ABI.
- **Interrupt discipline.** Early boot is single-threaded and interrupt-free
  until the IDT/PIC timer live (Milestone 2). All `writeln!` console output
  is byte-flushed and blocking to keep the skeleton deterministic.
- **No FP in the core.** Kernel code avoids floating point so the scheduler
  never has to save/restore XMM state for kernel-internal work. SSE is still
  *enabled* at boot (compilers emit integer SIMD), so the scheduler saves
  FP/XMM state per task at Milestone 2 anyway (see [kernel.md](kernel.md)).

## 7. Memory model

- **Early:** flat identity map of the first 1 GiB (2 MiB pages) set up in
  `boot.S`. Kernel lives at physical 1 MiB; virtual == physical.
- **Milestone 1:** higher-half remap — kernel mapped at
  `0xFFFF800000000000+`, user land below, proper 4 KiB page tables, physical
  frame bitmap + zone heap allocators.
- **Zones:** the classic "NewPtr/NewHandle" model becomes typed memory zones,
  each owned by a Manager or process; handles are zone+kinds guarded objects.
- Layout constants live in `kernel/rust/src/mem.rs`.

## 8. Processes, IPC and syscalls

- **Syscall ABI:** `syscall` instruction, C parameter convention, numbered
  table. Draft numbers already reserved in `kernel/cpp/include/pippin/kernel.hh`
  (`SYSCALL_EXIT`, `SYSCALL_LOG`, `SYSCALL_MMAP`, `SYSCALL_IPC_SEND/RECV`).
- **IPC:** message ports in the Event Manager spirit — typed events flowing
  from hardware/driver layer up to the app event loop.
- **Processes:** start as kernel threads with a per-thread "process slot"
  (classic ProcMenu-style), gain address-space isolation at Milestone 2+.

## 9. Driver model

Drivers are C++ classes implementing `pippin::drv::Driver`
(`drivers/cpp/include/pippin/drivers.hh`). They register with the Driver
Manager, which owns probing, initialization order and the device tree. Cross-
language calls go through C thunks — see [drivers.md](drivers.md).

## 10. FFI and linking strategy

The four languages share one ELF and a few simple rules:

1. **The boundary is `extern "C"`.** No mangled C++ symbols cross languages.
   Rust declares C functions in `kernel/rust/src/ffi.rs`; C++ exports C names
   from `entry.cc` and the driver stubs.
2. **One linker pass.** CMake builds C/C++/ASM objects plus the Rust
   `staticlib`; `ld` links them with a linker group (`--start-group/--end-
   group`) because the archives reference each other in both directions.
3. **Common codegen rules** (root `CMakeLists.txt`): freestanding, no PIC/PIE,
   no red zone, no stack protector. Rust builds against the host target today
   but `#![no_std]` — the doc-versioned directive is a dedicated freestanding
   target (see [build.md](build.md)).
4. **One linker script** (`kernel/linker.ld`) owns section order, the BSS
   region, `__init_array` handling and the Multiboot address fields.
5. **Host-side shell glue** (`scripts/`) runs image and emulator commands around
   the build. CMake and the linker still own compilation and the final ELF link;
   shell scripts are not part of the kernel image.

## 11. Directory map

```
Pippin/
├── boot/limine.conf        # reserved for the Milestone-1 Limine migration
├── docs/                   # this documentation
├── kernel/
│   ├── asm/boot.S          # Multiboot → long-mode bring-up (only the ELF entry)
│   ├── c/                  # C glue: libc stubs, shims
│   ├── cpp/                # C++ runtime: entry, "Managers" scaffolding
│   ├── rust/               # Rust kernel core: serial, cpu, mem, ffi
│   └── linker.ld
├── drivers/cpp/            # C++ driver layer
├── apps/                   # future C++/Rust apps; optional C# bindings
└── scripts/                # run-qemu.sh, make-iso.sh
```

## 12. Boot path (summary)

QEMU/GRUB → Multiboot v1 header (address fields set, so 64-bit works) →
`boot.S` zeroes BSS, builds an identity map, enables PAE/LME/paging/SSE →
loads a 64-bit GDT → jumps to `kernel_entry` (C++) → constructors →
`pippin_core_main` (Rust) → idle loop. Full details in [boot.md](boot.md).

## 13. Roadmap

See [milestones.md](milestones.md). Short version: M0 skeleton (this), M1 core
memory+interrupts+Limine, M2 processes+syscalls+IPC, M3 drivers, M4 GUI, M5 C++
apps, M6 optional C# apps on x86-64, M7 68k study.
