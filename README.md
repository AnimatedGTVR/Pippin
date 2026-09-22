# Pippin

A retro-styled, Macintosh-inspired desktop operating system. Single-user, GUI-first,
built with a mix of **Assembly**, **Rust**, **C++** and **C** in the kernel, **C++**
drivers, a **C++ and Rust** desktop, optional **C#** applications later, and
**shell scripts** for build and boot glue.

These are the current choices, not a limit on Pippin's languages. Other
languages can join when a subsystem or application benefits from them.

> **Milestone 0 is complete; Milestone 1 is in progress.** The kernel builds and
> boots under QEMU, with memory management and timer interrupts underway. See
> [docs/milestones.md](docs/milestones.md).

## What is here

| Path                             | Contents                                             |
|----------------------------------|------------------------------------------------------|
| `docs/`                          | Architecture and design documents (start here)       |
| `kernel/asm/`                    | Assembly: multiboot entry, long-mode bring-up        |
| `kernel/rust/`                   | Rust kernel core (cargo staticlib, `no_std`)         |
| `kernel/cpp/`                    | C++ kernel runtime / entry                           |
| `kernel/c/`                      | C glue: libc-style stubs used by C++ and Rust        |
| `drivers/cpp/`                   | C++ driver layer (PCI stub today)                    |
| `boot/`                          | Bootloader config (Limine, for the later migration)  |
| `apps/`                          | Future application layer (C++ and Rust; optional C#) |
| `scripts/`                       | Shell glue for QEMU / ISO workflows                  |

## Build

Requirements: `cmake >= 3.20`, a freestanding-capable `gcc`/`g++` (>= 13), `rustc`/`cargo`
with a host target, `make`, and optional `qemu-system-x86_64` to run it.

```sh
make                # configure + build build/kernel.elf
make run            # boot it in QEMU (serial console)
make run-gdb        # boot under QEMU with a GDB stub on :1234
make clean
```

See [docs/build.md](docs/build.md) for the full toolchain story, why the Rust crate is
currently a host-target `no_std` staticlib, and how to move to a dedicated
`x86_64-unknown-none` target.

## Design in one paragraph

Pippin is organized like the classic Macintosh Toolbox: a collection of cooperating
"Managers" (Memory, Processor, Event, File, Driver, Display, Window, Menu, Control,
Resource) wrapped around a small preemptive kernel core. Assembly owns boot and the CPU
trampolines, Rust owns the safety-critical core (memory, interrupts, scheduling, IPC),
C++ owns subsystems and drivers, C provides ABI-shim glue, and the future desktop
shell uses C++ and Rust. Optional C# apps can use the same Toolbox API. The full
picture lives in [docs/architecture.md](docs/architecture.md). Shell scripts
connect build, image, and emulator steps on the development host.

## Roadmap

- **M0 — Skeleton (complete):** multi-language build wired end-to-end, boots in QEMU.
- **M1 — Core:** higher-half paging, interrupts/GDT/IDT, real frame + heap allocators.
- **M2 — Processes:** scheduler, syscall ABI, IPC ports.
- **M3 — Drivers:** ACPI/PCI, PS/2, VESA framebuffer, disk.
- **M4 — GUI:** compositor, window/menu/control managers, retro desktop.
- **M5 — Apps:** C++ and Rust applications using the Toolbox API.
- **M6 — Optional C# apps:** managed apps using the Toolbox API on x86-64.
- **M7 — 68k:** Motorola 68000 feasibility port.
