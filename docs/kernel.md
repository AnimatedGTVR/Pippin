# Kernel

The kernel core is written in Rust (`kernel/rust/`), wrapped by a thin C++
runtime and linked into both boot ELFs. Read [architecture.md](architecture.md) for
the whole picture; this file is the Rust deep-dive.

## Entry and ownership

```
kernel_entry(mb_info) or limine_start()     C++
   ├─ run global constructors
   └─ pippin_core_main(_limine)             Rust (lib.rs) — never returns
         ├─ serial::init(0x3F8)    + writeln! bootstrap banner
         ├─ memory map → frame bitmap → page tables / heap
         ├─ ffi::cpp_version()      ← reverse bridge into C++
         ├─ ffi::driver_count()     ← reverse bridge into drivers
         ├─ GDT/TSS → IDT → PIT → APIC timer
         ├─ scheduler → IPC/Event Manager → syscall entry
         └─ loop { cpu::halt() }
```

The Rust crate is `no_std`, dependency-free (`Cargo.toml` `[dependencies]` is
intentionally empty) and compiled with `panic=abort`. It exposes a single C
entry for each boot path plus whatever the C++/driver layer needs to call
back into (`kernel/rust/src/ffi.rs`). Nothing enters Rust but through
`extern "C"` — this keeps the language boundary trivial to audit.

## Modules today

| Module        | Contents |
|---------------|----------|
| `cpu.rs`      | `outb`/`inb` port I/O, `halt`, `cli`/`sti` (inline `asm!`) |
| `serial.rs`   | 16550 COM1 driver; `stdout()` -> `core::fmt::Write`; `is_ready()` |
| `mem.rs`      | Multiboot/Limine memory maps and physical frame bitmap |
| `mm.rs`       | 4 KiB page tables for the Multiboot path and MMIO mapping |
| `heap.rs`     | zone allocator backing `Box` and `Vec` |
| `gdt.rs`      | GDT, TSS, and double-fault stack |
| `idt.rs`/`interrupts.rs` | 256-vector IDT, PIC/PIT and interrupt dispatch |
| `apic.rs`     | calibrated local APIC timer |
| `acpi.rs`/`ahci.rs` | ACPI root discovery and polling SATA reads |
| `display.rs`/`ps2.rs` | Limine RGB framebuffer, VGA text, and polled input events |
| `desktop.rs`  | QEMU VGA mode switching and framebuffer presentation |
| `compositor.rs` | Rust wallpaper fallback, window stack, pointer, drawing |
| `cli.rs`      | M4 bootstrap command shell over VGA text and serial |
| `file.rs`     | Limine module and bounded read-only FAT32 bundle loading |
| `sched.rs`    | preemptive run queue, task stacks, process slots, FP state |
| `syscall.rs`  | numbered ring-3 syscall table and demo process setup |
| `ipc.rs`/`event.rs` | owned message ports and Event Manager delivery |
| `zones.rs`    | owned zones and generation-checked object handles |
| `ffi.rs`      | `extern "C"` bridge: `pippin_cpp_version`, `pippin_driver_count` |
| `lib.rs`      | `pippin_core_main`, `#[panic_handler]` |

## Naming and the "Manager" convention

Subsystem modules follow the classic Macintosh names — `mem`, `event`, `ipc`,
and later `file`, `drv`, `disp`, `win`, `menu`, `ctrl`, `res`. Current managers
use module-level state on one CPU; the remaining Toolbox managers arrive in
later milestones.

## Milestone progression

- **Milestone 1 — memory & interrupts (complete):**
  - physical frame bitmap over the bootloader memory map,
  - higher-half 4 KiB tables on Multiboot, Limine page tables on the ISO path,
  - GDT/TSS, 256-vector IDT, Rust dispatcher, and PIT→APIC timer handoff,
  - zone allocator backing `Box`/`Vec` through `#[global_allocator]`.
- **Milestone 2 — processes & syscalls (complete):**
  - `syscall`/`sysret` trampolines (asm), numbered table with names from
    `pippin::kabi::Syscall` (`kernel/cpp/include/pippin/kernel.hh`),
  - scheduler: preemptive, per-task FP/XMM save (fxsave/fxrstor), quantum
    timers,
  - IPC: owned message ports carrying typed `Event`s into the event loop,
  - owned zones with opaque generation-checked handles.

The M2 demo runs one ring-3 task in the shared boot address space. The task
maps a page, logs, sends an IPC event and exits. General executable loading
and private process page tables are later work.
- **Milestone 3 — file & drivers (core complete):** ACPI root validation,
  PCI bridge tree and C++ driver lifecycle, PS/2 events, Limine framebuffer,
  polling AHCI SATA reads, and a bounded FAT32 Hello bundle loader.
  ISO9660 and broader file operations remain future work.
- **Milestone 4 — in progress:** an interactive command shell consumes PS/2
  key events and serial input; the Rust framebuffer compositor is an M4
  prototype and C# guest clients remain future work.

## Interrupt discipline

Until the IDT exists, interrupts are off and the world is single-threaded.
`cpu::cli`/`cpu::sti` control boot interrupt state. Shared scheduler, heap,
IPC and zone mutations mask local interrupts on this single-CPU kernel. The
serial writer is byte-blocking and non-recursive, including in panic output.

## FP/XMM policy

- Code in the **Rust core and C++ kernel subsystems must not use float math**.
  This keeps kernel-internal paths free of FP state save/restore and of #NM/#
  XM surprises.
- SSE is nonetheless initialized at boot (CR4.OSFXSR) because compilers emit
  integer SIMD (see [boot.md](boot.md) for the #UD story).
- The scheduler saves/restores the full FPU/XMM context
  (`fxsave64`/`fxrstor64`) per task, so strictly-float drivers are possible
  later without poisoning the core.

## Adding something new

1. Pick the right layer: CPU/asm → `kernel/asm`, core logic → `kernel/rust`,
   subsystem/driver → `kernel/cpp` or `drivers/cpp`, ABI shim → `kernel/c`.
2. Cross a language boundary only through a new `extern "C"` pair (declare in
   `ffi.rs` / a `kernel/*/include` header), and add the source file to the
   matching `CMakeLists.txt` (for Rust, also to the `DEPENDS` list of the
   `rust-kernel` custom command in the root `CMakeLists.txt`).
3. Rebuild `make`, boot `make run`, watch the serial console.
