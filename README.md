# Pippin

A retro-styled, Macintosh-inspired desktop operating system. Single-user, GUI-first,
built with a mix of **Assembly**, **Rust**, **C++** and **C** in the kernel, **C++**
drivers, a **Rust** compositor and **C++** desktop shell, with a small **C** ABI layer, and
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
| `apps/`                          | Native apps plus the archived C# shell prototype      |
| `scripts/`                       | Shell glue for QEMU / ISO workflows                  |

## Build

Requirements: `cmake >= 3.20`, a freestanding-capable `gcc`/`g++` (>= 13), `rustc`/`cargo`
with the `x86_64-unknown-none` target, `make`, and optional `qemu-system-x86_64`
to run it. `make iso` also needs Limine and `xorriso`.

```sh
make specs          # inspect host specs and calculate safe Pippin limits
make                # preflight + configure + build build/kernel.elf
make run            # boot full desktop using QEMU's normal frontend
make run-shell      # alias for make run
make run-ui         # alias for the native desktop run path
make run-headless   # serial console only
make run-disk       # QEMU window with the FAT32 Hello bundle disk
make run-gdb        # normal QEMU frontend + GDB stub on :1234
make iso            # build the Limine BIOS/UEFI ISO
make clean
```

Every normal build runs `scripts/preflight.sh` first. It reads the host's
logical CPU count, total RAM, and free project-disk space, then writes
`build/pippin-limits.env`. That file selects a resource profile, caps compiler
parallelism, and chooses **256-768 MiB** of QEMU RAM. The QEMU cap intentionally
stays below Pippin's current 1 GiB early direct-map limit. Run `make specs` to
print the calculation without booting Pippin.

While a graphical desktop session is running, the launching terminal becomes
the diagnostic console. Guest serial output is tagged `[guest]`, QEMU stderr is tagged `[qemu]`, and QEMU
guest-error diagnostics are tagged `[qemu-debug]`. The same output is saved in
`build/logs/runtime.log`, with dedicated `shell.log` and `qemu-debug.log`
files for post-crash inspection.

The Makefile defaults GUI runs to `QEMU_MODE=--defaultqemu`, which leaves display
frontend selection to QEMU instead of forcing Pippin's stripped GTK frontend.
Override it with `make run QEMU_MODE=` if you want the custom GTK mode instead.
Because Pippin currently uses a relative PS/2 mouse, QEMU may still grab pointer
input while interacting with the guest; a truly grab-free absolute pointer will
require a USB/virtio tablet input driver in Pippin.

See [docs/build.md](docs/build.md) for the full toolchain and ISO setup.

After `make run`, Pippin enters the graphical desktop directly from the guest.
The shell model is compiled into the kernel-side C++ runtime and exported through
a small C ABI to the Rust compositor. No host .NET process is required.
`make run-shell` and `make run-ui` are aliases for this same native desktop
boot path. Use `make run-headless` for the kernel/serial-only workflow and
`make run-disk` to make `hello.pipb` available to `ls` and `cat`.

## Design in one paragraph

Pippin is organized like the classic Macintosh Toolbox: a collection of cooperating
"Managers" (Memory, Processor, Event, File, Driver, Display, Window, Menu, Control,
Resource) wrapped around a small preemptive kernel core. Assembly owns boot and the CPU
trampolines, Rust owns the safety-critical core (memory, interrupts, scheduling, IPC),
C++ owns subsystems, drivers, and the desktop shell model; C provides the stable ABI shim between C++ and Rust; Rust owns the compositor, window management, input, and safety-critical core. The old C# shell remains only as a design prototype and is no longer part of the default boot path. The full
picture lives in [docs/architecture.md](docs/architecture.md). Shell scripts
connect build, image, and emulator steps on the development host.

## Roadmap

- **M0 — Skeleton (complete):** multi-language build wired end-to-end, boots in QEMU.
- **M1 — Core (complete):** higher-half paging, GDT/TSS/IDT, frame and heap allocators, APIC timer, Limine ISO.
- **M2 — Processes (complete):** preemptive scheduler, ring-3 syscall ABI,
  IPC event ports, owned zones and handles.
- **M3 — Drivers (core complete):** ACPI/PCI discovery, PS/2 events, Limine
  framebuffer, AHCI reads, FAT32 Hello bundle loading.
- **M4 — GUI:** Rust compositor/window manager with a native C++ desktop shell.
- **M4.5 — Native shell ABI:** C++ shell surfaces exported through a small C ABI to Rust.
- **Window API foundation:** separate clients, owned surfaces and input events.
- **M5 — Apps:** C++ and Rust applications using the Toolbox API.
- **M6 — Runtime expansion:** native executable loading, packaging, and services on x86-64.
- **M7 — 68k:** Motorola 68000 feasibility port.
