# Pippin

A retro-styled, Macintosh-inspired desktop operating system. Single-user, GUI-first,
built with a mix of **Assembly**, **Rust**, **C++** and **C** in the kernel, **C++**
drivers, a **Rust** compositor and **C#** desktop shell, and
**shell scripts** for build and boot glue.

These are the current choices, not a limit on Pippin's languages. Other
languages can join when a subsystem or application benefits from them.

> **Milestones 0 and 1 are complete.** The kernel boots under QEMU through
> Multiboot or a Limine ISO, with memory management and timer interrupts. See
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
| `boot/`                          | Limine bootloader config                              |
| `apps/`                          | C# shell model and future native applications         |
| `scripts/`                       | Shell glue for QEMU / ISO workflows                  |

## Build

Requirements: `cmake >= 3.20`, a freestanding-capable `gcc`/`g++` (>= 13), `rustc`/`cargo`
with the `x86_64-unknown-none` target, `make`, and optional `qemu-system-x86_64`
to run it. `make iso` also needs Limine and `xorriso`.

```sh
make                # configure + build build/kernel.elf
make run            # boot full desktop using QEMU's normal frontend
make run-shell      # alias for make run
make run-ui         # two independent C# apps using the new window API
make run-headless   # serial console only
make run-disk       # QEMU window with the FAT32 Hello bundle disk
make run-gdb        # normal QEMU frontend + GDB stub on :1234
make iso            # build the Limine BIOS/UEFI ISO
make clean
```

The Makefile defaults GUI runs to `QEMU_MODE=--defaultqemu`, which leaves display
frontend selection to QEMU instead of forcing Pippin's stripped GTK frontend.
Override it with `make run QEMU_MODE=` if you want the custom GTK mode instead.
Because Pippin currently uses a relative PS/2 mouse, QEMU may still grab pointer
input while interacting with the guest; a truly grab-free absolute pointer will
require a USB/virtio tablet input driver in Pippin.

See [docs/build.md](docs/build.md) for the full toolchain and ISO setup.

After `make run`, the host .NET shell connects over QEMU's second serial port
and Pippin enters the full graphical desktop automatically. The panel, dock,
notifications, and Terminal are created at startup; **Alt+T** brings Terminal
to the front. `make run-shell` is kept as an alias for this same full-desktop
boot path. Use `make run-headless` for the kernel/serial-only workflow and
`make run-disk` to make `hello.pipb` available to `ls` and `cat`.
`make run-ui` starts a window broker and two separate C# processes. About and
Task Manager each paint a private surface through the reusable window API;
both appear in QEMU. See [docs/windowing.md](docs/windowing.md) for the API and
the remaining guest-runtime boundary.

## Design in one paragraph

Pippin is organized like the classic Macintosh Toolbox: a collection of cooperating
"Managers" (Memory, Processor, Event, File, Driver, Display, Window, Menu, Control,
Resource) wrapped around a small preemptive kernel core. Assembly owns boot and the CPU
trampolines, Rust owns the safety-critical core (memory, interrupts, scheduling, IPC),
C++ owns subsystems and drivers, C provides ABI-shim glue, Rust owns the
compositor and window management, and C# defines the panel, dock, launcher,
settings, files, notifications, wallpaper and app windows. The C# projects
build on the host; guest execution still needs a managed runtime and IPC. The full
picture lives in [docs/architecture.md](docs/architecture.md). Shell scripts
connect build, image, and emulator steps on the development host.

## Roadmap

- **M0 — Skeleton (complete):** multi-language build wired end-to-end, boots in QEMU.
- **M1 — Core (complete):** higher-half paging, GDT/TSS/IDT, frame and heap allocators, APIC timer, Limine ISO.
- **M2 — Processes (complete):** preemptive scheduler, ring-3 syscall ABI,
  IPC event ports, owned zones and handles.
- **M3 — Drivers (core complete):** ACPI/PCI discovery, PS/2 events, Limine
  framebuffer, AHCI reads, FAT32 Hello bundle loading.
- **M4 — GUI:** Rust compositor/window manager and C# desktop shell.
- **M4.5 — C# bridge:** interactive host C# shell surfaces displayed in QEMU.
- **Window API foundation:** separate clients, owned surfaces and input events.
- **M5 — Apps:** C++ and Rust applications using the Toolbox API.
- **M6 — Runtime expansion:** managed app packaging and services on x86-64.
- **M7 — 68k:** Motorola 68000 feasibility port.
